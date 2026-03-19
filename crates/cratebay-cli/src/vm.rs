fn grpc_addr() -> String {
    std::env::var("CRATEBAY_GRPC_ADDR").unwrap_or_else(|_| "127.0.0.1:50051".into())
}

fn grpc_endpoint(addr: &str) -> String {
    if addr.starts_with("http://") || addr.starts_with("https://") {
        addr.to_string()
    } else {
        format!("http://{}", addr)
    }
}

async fn connect_vm_service_timeout(
    addr: &str,
    timeout: Duration,
) -> Result<VmServiceClient<Channel>, String> {
    let endpoint = grpc_endpoint(addr);
    let connect_fut = VmServiceClient::connect(endpoint.clone());
    let client = tokio::time::timeout(timeout, connect_fut)
        .await
        .map_err(|_| format!("Timed out connecting to daemon at {}", endpoint))?
        .map_err(|e| format!("Failed to connect to daemon at {}: {}", endpoint, e))?;
    Ok(client)
}

async fn connect_vm_service(addr: &str) -> Result<VmServiceClient<Channel>, String> {
    connect_vm_service_timeout(addr, Duration::from_secs(1)).await
}

fn daemon_file_name() -> &'static str {
    #[cfg(windows)]
    {
        "cratebay-daemon.exe"
    }
    #[cfg(not(windows))]
    {
        "cratebay-daemon"
    }
}

fn daemon_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CRATEBAY_DAEMON_PATH") {
        return path.into();
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(daemon_file_name());
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    daemon_file_name().into()
}

fn spawn_daemon_detached() -> Result<u32, String> {
    use std::process::{Command as ProcessCommand, Stdio};

    let daemon = daemon_path();
    let mut cmd = ProcessCommand::new(&daemon);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", daemon.display(), e))?;
    Ok(child.id())
}

#[cfg(windows)]
fn runtime_proxy_pid_path() -> PathBuf {
    cratebay_core::data_dir()
        .join("run")
        .join("wsl-docker-proxy.pid")
}

#[cfg(windows)]
fn pick_free_localhost_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .map_err(|e| format!("Failed to reserve a local relay port: {}", e))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| format!("Failed to inspect local relay port: {}", e))
}

#[cfg(windows)]
fn kill_runtime_proxy_process() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command as ProcessCommand;
    use std::thread;
    use std::time::{Duration, Instant};

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const TASKKILL_TIMEOUT: Duration = Duration::from_secs(15);

    let pid_path = runtime_proxy_pid_path();
    let Some(pid) = std::fs::read_to_string(&pid_path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
    else {
        let _ = std::fs::remove_file(&pid_path);
        return Ok(());
    };

    let mut child = ProcessCommand::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to run taskkill for runtime proxy {}: {}", pid, e))?;

    let deadline = Instant::now() + TASKKILL_TIMEOUT;
    let mut timeout_error = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    timeout_error = Some(format!(
                        "taskkill exited with {} while stopping runtime proxy {}",
                        status, pid
                    ));
                }
                break;
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                timeout_error = Some(format!(
                    "Timed out waiting for taskkill to stop runtime proxy {}",
                    pid
                ));
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => {
                timeout_error = Some(format!(
                    "Failed to wait for taskkill to stop runtime proxy {}: {}",
                    pid, e
                ));
                break;
            }
        }
    }

    let _ = std::fs::remove_file(pid_path);
    if let Some(error) = timeout_error {
        return Err(error);
    }

    Ok(())
}

#[cfg(windows)]
fn spawn_runtime_proxy_detached(
    guest_ip: &str,
    listen_port: u16,
    target_port: u16,
) -> Result<u32, String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command as ProcessCommand, Stdio};

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let exe = std::env::current_exe()
        .map_err(|e| format!("Failed to resolve current executable: {}", e))?;
    let pid_path = runtime_proxy_pid_path();
    if let Some(parent) = pid_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create runtime proxy directory: {}", e))?;
    }

    let child = ProcessCommand::new(exe)
        .arg("internal-runtime-proxy")
        .arg("--guest-ip")
        .arg(guest_ip)
        .arg("--listen-port")
        .arg(listen_port.to_string())
        .arg("--target-port")
        .arg(target_port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to spawn detached runtime proxy: {}", e))?;

    std::fs::write(&pid_path, child.id().to_string())
        .map_err(|e| format!("Failed to record runtime proxy PID: {}", e))?;
    Ok(child.id())
}

async fn wait_for_vm_service(
    addr: &str,
    timeout: Duration,
) -> Result<VmServiceClient<Channel>, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(client) = connect_vm_service_timeout(addr, Duration::from_millis(200)).await {
            return Ok(client);
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out waiting for daemon to become ready at {}",
                grpc_endpoint(addr)
            ));
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn connect_vm_service_autostart(addr: &str) -> Option<VmServiceClient<Channel>> {
    if let Ok(client) = connect_vm_service(addr).await {
        return Some(client);
    }

    if spawn_daemon_detached().is_err() {
        return None;
    }

    wait_for_vm_service(addr, Duration::from_secs(5)).await.ok()
}

async fn resolve_vm_id_grpc(
    client: &mut VmServiceClient<Channel>,
    selector: &str,
) -> Result<String, String> {
    let resp = client
        .list_v_ms(proto::ListVMsRequest {})
        .await
        .map_err(|e| format!("Failed to list VMs: {}", e))?
        .into_inner();

    if resp.vms.iter().any(|vm| vm.vm_id == selector) {
        return Ok(selector.to_string());
    }
    if let Some(vm) = resp.vms.iter().find(|vm| vm.name == selector) {
        return Ok(vm.vm_id.clone());
    }
    Err(format!("VM not found: {}", selector))
}

fn resolve_vm_id_local(
    hv: &dyn cratebay_core::hypervisor::Hypervisor,
    selector: &str,
) -> Result<String, cratebay_core::hypervisor::HypervisorError> {
    let vms = hv.list_vms()?;
    if vms.iter().any(|vm| vm.id == selector) {
        return Ok(selector.to_string());
    }
    if let Some(vm) = vms.into_iter().find(|vm| vm.name == selector) {
        return Ok(vm.id);
    }
    Err(cratebay_core::hypervisor::HypervisorError::NotFound(
        selector.into(),
    ))
}

async fn handle_vm(cmd: VmCommands) {
    let addr = grpc_addr();
    let mut client = connect_vm_service_autostart(&addr).await;

    let hv = if client.is_none() {
        Some(cratebay_core::create_hypervisor())
    } else {
        None
    };

    match cmd {
        VmCommands::Create {
            name,
            cpus,
            memory,
            disk,
            rosetta,
            os_image,
        } => {
            if let Err(e) = validation::validate_vm_name(&name) {
                eprintln!("Error: invalid VM name '{}': {}", name, e);
                std::process::exit(1);
            }
            // Resolve image paths if an OS image was specified.
            let (kernel_path, initrd_path, disk_path) = if let Some(ref img_id) = os_image {
                if !cratebay_core::images::is_image_ready(img_id) {
                    eprintln!("Error: OS image '{}' is not downloaded yet. Run: cratebay image download-os {}", img_id, img_id);
                    std::process::exit(1);
                }
                let paths = cratebay_core::images::image_paths(img_id);
                (
                    Some(paths.kernel_path.to_string_lossy().into_owned()),
                    Some(paths.initrd_path.to_string_lossy().into_owned()),
                    Some(paths.rootfs_path.to_string_lossy().into_owned()),
                )
            } else {
                (None, None, None)
            };

            if let Some(client) = client.as_mut() {
                let resp = client
                    .create_vm(proto::CreateVmRequest {
                        name: name.clone(),
                        cpus,
                        memory_mb: memory,
                        disk_gb: disk,
                        rosetta,
                        shared_dirs: vec![],
                    })
                    .await;
                match resp {
                    Ok(r) => {
                        let id = r.into_inner().vm_id;
                        println!("Created VM '{}' (id: {})", name, id);
                        if rosetta {
                            println!("  Rosetta x86_64 translation: enabled");
                        }
                        if let Some(ref img) = os_image {
                            println!("  OS image: {}", img);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                use cratebay_core::hypervisor::VmConfig;
                let hv = hv.as_ref().unwrap();
                let config = VmConfig {
                    name: name.clone(),
                    cpus,
                    memory_mb: memory,
                    disk_gb: disk,
                    rosetta,
                    shared_dirs: vec![],
                    os_image: os_image.clone(),
                    kernel_path,
                    initrd_path,
                    disk_path,
                    port_forwards: vec![],
                };
                match hv.create_vm(config) {
                    Ok(id) => {
                        println!("Created VM '{}' (id: {})", name, id);
                        if rosetta {
                            println!("  Rosetta x86_64 translation: enabled");
                        }
                        if let Some(ref img) = os_image {
                            println!("  OS image: {}", img);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        VmCommands::Start { name } => {
            // Validate unless it looks like an internal ID (e.g. "stub-1").
            if !name.contains('-')
                || name
                    .split('-')
                    .next_back()
                    .and_then(|s| s.parse::<u64>().ok())
                    .is_none()
            {
                if let Err(e) = validation::validate_vm_name(&name) {
                    eprintln!("Error: invalid VM name '{}': {}", name, e);
                    std::process::exit(1);
                }
            }
            if let Some(client) = client.as_mut() {
                let id = match resolve_vm_id_grpc(client, &name).await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                if let Err(e) = client
                    .start_vm(proto::StartVmRequest { vm_id: id.clone() })
                    .await
                {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!("Started VM '{}'", name);
            } else {
                let hv = hv.as_ref().unwrap();
                let id = match resolve_vm_id_local(hv.as_ref(), &name) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                match hv.start_vm(&id) {
                    Ok(()) => println!("Started VM '{}'", name),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        VmCommands::Stop { name } => {
            if !name.contains('-')
                || name
                    .split('-')
                    .next_back()
                    .and_then(|s| s.parse::<u64>().ok())
                    .is_none()
            {
                if let Err(e) = validation::validate_vm_name(&name) {
                    eprintln!("Error: invalid VM name '{}': {}", name, e);
                    std::process::exit(1);
                }
            }
            if let Some(client) = client.as_mut() {
                let id = match resolve_vm_id_grpc(client, &name).await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                if let Err(e) = client
                    .stop_vm(proto::StopVmRequest { vm_id: id.clone() })
                    .await
                {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!("Stopped VM '{}'", name);
            } else {
                let hv = hv.as_ref().unwrap();
                let id = match resolve_vm_id_local(hv.as_ref(), &name) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                match hv.stop_vm(&id) {
                    Ok(()) => println!("Stopped VM '{}'", name),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        VmCommands::Delete { name } => {
            if !name.contains('-')
                || name
                    .split('-')
                    .next_back()
                    .and_then(|s| s.parse::<u64>().ok())
                    .is_none()
            {
                if let Err(e) = validation::validate_vm_name(&name) {
                    eprintln!("Error: invalid VM name '{}': {}", name, e);
                    std::process::exit(1);
                }
            }
            if let Some(client) = client.as_mut() {
                let id = match resolve_vm_id_grpc(client, &name).await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                if let Err(e) = client
                    .delete_vm(proto::DeleteVmRequest { vm_id: id.clone() })
                    .await
                {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!("Deleted VM '{}'", name);
            } else {
                let hv = hv.as_ref().unwrap();
                let id = match resolve_vm_id_local(hv.as_ref(), &name) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                match hv.delete_vm(&id) {
                    Ok(()) => println!("Deleted VM '{}'", name),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        VmCommands::List => {
            if let Some(client) = client.as_mut() {
                let resp = client.list_v_ms(proto::ListVMsRequest {}).await;
                match resp {
                    Ok(r) => {
                        let vms = r.into_inner().vms;
                        if vms.is_empty() {
                            println!("No VMs found.");
                            return;
                        }
                        println!(
                            "{:<12} {:<20} {:<10} {:<6} {:<8} {:<8} MOUNTS",
                            "ID", "NAME", "STATE", "CPUS", "MEMORY", "ROSETTA"
                        );
                        for vm in vms {
                            println!(
                                "{:<12} {:<20} {:<10} {:<6} {:<8} {:<8} {}",
                                vm.vm_id,
                                vm.name,
                                vm.status,
                                vm.cpus,
                                format!("{}MB", vm.memory_mb),
                                if vm.rosetta_enabled { "yes" } else { "no" },
                                vm.shared_dirs.len(),
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                let hv = hv.as_ref().unwrap();
                match hv.list_vms() {
                    Ok(vms) => {
                        if vms.is_empty() {
                            println!("No VMs found.");
                            return;
                        }
                        println!(
                            "{:<12} {:<20} {:<10} {:<6} {:<8} {:<8} MOUNTS",
                            "ID", "NAME", "STATE", "CPUS", "MEMORY", "ROSETTA"
                        );
                        for vm in vms {
                            println!(
                                "{:<12} {:<20} {:<10} {:<6} {:<8} {:<8} {}",
                                vm.id,
                                vm.name,
                                format!("{:?}", vm.state),
                                vm.cpus,
                                format!("{}MB", vm.memory_mb),
                                if vm.rosetta_enabled { "yes" } else { "no" },
                                vm.shared_dirs.len(),
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        VmCommands::LoginCmd {
            name,
            user,
            host,
            port,
        } => {
            let Some(port) = port else {
                eprintln!("Error: VM login is not available yet. Specify an SSH port via --port.");
                std::process::exit(1);
            };
            println!("ssh {}@{} -p {}", user, host, port);
            println!("# VM: {}", name);
        }
        VmCommands::Port { command } => {
            handle_port(command, client.as_mut(), hv.as_deref()).await;
        }
    }
}

async fn handle_port(
    cmd: PortCommands,
    client: Option<&mut VmServiceClient<Channel>>,
    hv: Option<&dyn cratebay_core::hypervisor::Hypervisor>,
) {
    match cmd {
        PortCommands::Add {
            vm,
            host,
            guest,
            protocol,
        } => {
            if let Some(client) = client {
                let vm_id = match resolve_vm_id_grpc(client, &vm).await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                if let Err(e) = client
                    .add_port_forward(proto::AddPortForwardRequest {
                        vm_id,
                        host_port: host as u32,
                        guest_port: guest as u32,
                        protocol: protocol.clone(),
                    })
                    .await
                {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!(
                    "Added port forward: host {} -> guest {} ({})",
                    host, guest, protocol
                );
            } else {
                let hv = hv.unwrap();
                let vm_id = match resolve_vm_id_local(hv, &vm) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                let pf = cratebay_core::hypervisor::PortForward {
                    host_port: host,
                    guest_port: guest,
                    protocol: protocol.clone(),
                };
                match hv.add_port_forward(&vm_id, &pf) {
                    Ok(()) => {
                        println!(
                            "Added port forward: host {} -> guest {} ({})",
                            host, guest, protocol
                        );
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        PortCommands::List { vm } => {
            if let Some(client) = client {
                let vm_id = match resolve_vm_id_grpc(client, &vm).await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                match client
                    .list_port_forwards(proto::ListPortForwardsRequest { vm_id })
                    .await
                {
                    Ok(resp) => {
                        let forwards = resp.into_inner().forwards;
                        if forwards.is_empty() {
                            println!("No port forwards configured.");
                            return;
                        }
                        println!("{:<12} {:<12} PROTOCOL", "HOST PORT", "GUEST PORT");
                        for pf in forwards {
                            println!("{:<12} {:<12} {}", pf.host_port, pf.guest_port, pf.protocol);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                let hv = hv.unwrap();
                let vm_id = match resolve_vm_id_local(hv, &vm) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                match hv.list_port_forwards(&vm_id) {
                    Ok(forwards) => {
                        if forwards.is_empty() {
                            println!("No port forwards configured.");
                            return;
                        }
                        println!("{:<12} {:<12} PROTOCOL", "HOST PORT", "GUEST PORT");
                        for pf in forwards {
                            println!("{:<12} {:<12} {}", pf.host_port, pf.guest_port, pf.protocol);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        PortCommands::Remove { vm, host } => {
            if let Some(client) = client {
                let vm_id = match resolve_vm_id_grpc(client, &vm).await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                if let Err(e) = client
                    .remove_port_forward(proto::RemovePortForwardRequest {
                        vm_id,
                        host_port: host as u32,
                    })
                    .await
                {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!("Removed port forward on host port {}", host);
            } else {
                let hv = hv.unwrap();
                let vm_id = match resolve_vm_id_local(hv, &vm) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                match hv.remove_port_forward(&vm_id, host) {
                    Ok(()) => println!("Removed port forward on host port {}", host),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}

async fn handle_runtime(cmd: RuntimeCommands) {
    #[cfg(target_os = "macos")]
    {
        let socket_path = cratebay_core::runtime::host_docker_socket_path();
        let socket_url = format!("unix://{}", socket_path.to_string_lossy());

        match cmd {
            RuntimeCommands::Env => {
                println!("export DOCKER_HOST={}", socket_url);
            }
            RuntimeCommands::Status => {
                let image_id = cratebay_core::runtime::runtime_os_image_id();
                println!(
                    "Runtime image: {} ({})",
                    image_id,
                    if cratebay_core::runtime::runtime_image_ready() {
                        "ready"
                    } else {
                        "not downloaded"
                    }
                );

                let hv = cratebay_core::create_hypervisor();
                let vms = match hv.list_vms() {
                    Ok(vms) => vms,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };

                if let Some(vm) = vms
                    .iter()
                    .find(|vm| vm.name == cratebay_core::runtime::runtime_vm_name())
                {
                    println!("Runtime VM: {} ({:?})", vm.id, vm.state);
                } else {
                    println!("Runtime VM: not created");
                }

                println!(
                    "Docker socket: {} ({})",
                    socket_path.display(),
                    if socket_path.exists() {
                        "present"
                    } else {
                        "missing"
                    }
                );

                if socket_path.exists() {
                    match Docker::connect_with_socket(
                        socket_path.to_str().unwrap_or_default(),
                        120,
                        bollard::API_DEFAULT_VERSION,
                    ) {
                        Ok(docker) => match docker.version().await {
                            Ok(v) => {
                                println!(
                                    "Docker engine: {}",
                                    v.version.unwrap_or_else(|| "unknown".into())
                                );
                            }
                            Err(e) => {
                                println!("Docker engine: not responding ({})", e);
                            }
                        },
                        Err(e) => {
                            println!("Docker engine: failed to connect ({})", e);
                        }
                    }
                }
            }
            RuntimeCommands::Start => {
                let hv = cratebay_core::create_hypervisor();
                if let Err(e) = cratebay_core::runtime::ensure_runtime_vm_exists(hv.as_ref()) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }

                if let Err(e) = connect_cratebay_runtime_docker() {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }

                let vm_id = match cratebay_core::runtime::ensure_runtime_vm_exists(hv.as_ref()) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };

                println!("CrateBay Runtime VM: {}", vm_id);
                println!("Docker socket: {}", socket_path.display());
                println!("DOCKER_HOST: {}", socket_url);

                match Docker::connect_with_socket(
                    socket_path.to_str().unwrap_or_default(),
                    120,
                    bollard::API_DEFAULT_VERSION,
                ) {
                    Ok(docker) => match docker.version().await {
                        Ok(v) => {
                            println!(
                                "Docker engine: {}",
                                v.version.unwrap_or_else(|| "unknown".into())
                            );
                        }
                        Err(e) => {
                            eprintln!("Docker engine check failed: {}", e);
                            std::process::exit(1);
                        }
                    },
                    Err(e) => {
                        eprintln!("Docker engine connect failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            RuntimeCommands::Stop => {
                let hv = cratebay_core::create_hypervisor();
                let vms = match hv.list_vms() {
                    Ok(vms) => vms,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };

                let Some(vm) = vms
                    .into_iter()
                    .find(|vm| vm.name == cratebay_core::runtime::runtime_vm_name())
                else {
                    println!("CrateBay Runtime VM not found.");
                    return;
                };

                if let Err(e) = hv.stop_vm(&vm.id) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!("Stopped CrateBay Runtime VM {}", vm.id);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let host = match &cmd {
            RuntimeCommands::Stop => String::new(),
            _ => match ensure_windows_runtime_terminal_host().await {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            },
        };

        match cmd {
            RuntimeCommands::Env => {
                let platform = cratebay_runtime_linux_platform();
                println!(
                    "PowerShell: $env:DOCKER_HOST=\"{}\"; $env:CRATEBAY_DOCKER_PLATFORM=\"{}\"",
                    host, platform
                );
                println!(
                    "CMD      : set DOCKER_HOST={} && set CRATEBAY_DOCKER_PLATFORM={}",
                    host, platform
                );
                println!(
                    "Bash     : export DOCKER_HOST={} CRATEBAY_DOCKER_PLATFORM={}",
                    host, platform
                );
            }
            RuntimeCommands::Status => {
                println!("Runtime: CrateBay Runtime (WSL2)");
                println!("DOCKER_HOST: {}", host);
                println!(
                    "CRATEBAY_DOCKER_PLATFORM: {}",
                    cratebay_runtime_linux_platform()
                );

                match docker_http_engine_version(&host, Duration::from_secs(15)).await {
                    Ok(version) => println!("Docker engine: {}", version),
                    Err(error) => println!("Docker engine: not responding ({})", error),
                }
            }
            RuntimeCommands::Start => {
                println!("Runtime: CrateBay Runtime (WSL2)");
                println!("DOCKER_HOST: {}", host);
                println!(
                    "CRATEBAY_DOCKER_PLATFORM: {}",
                    cratebay_runtime_linux_platform()
                );

                match docker_http_engine_version(&host, Duration::from_secs(15)).await {
                    Ok(version) => println!("Docker engine: {}", version),
                    Err(error) => {
                        eprintln!("{}", error);
                        std::process::exit(1);
                    }
                }
            }
            RuntimeCommands::Stop => {
                let _ = kill_runtime_proxy_process();
                if let Err(e) = cratebay_core::runtime::stop_runtime_wsl() {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!("Stopped CrateBay Runtime (WSL2).");
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let host = cratebay_core::runtime::runtime_linux_docker_host();
        let console_log = cratebay_core::runtime::runtime_linux_console_log_path();
        let running = cratebay_core::runtime::runtime_linux_is_running();
        let kvm_available = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kvm")
            .is_ok();

        match cmd {
            RuntimeCommands::Env => {
                println!("export DOCKER_HOST={}", host);
            }
            RuntimeCommands::Status => {
                let image_id = cratebay_core::runtime::runtime_os_image_id();
                println!(
                    "Runtime image: {} ({})",
                    image_id,
                    if cratebay_core::runtime::runtime_image_ready() {
                        "ready"
                    } else {
                        "not installed"
                    }
                );
                println!(
                    "Runtime: CrateBay Runtime (Linux/{})",
                    if kvm_available {
                        "QEMU+KVM"
                    } else {
                        "QEMU/TCG"
                    }
                );
                println!("State: {}", if running { "running" } else { "stopped" });
                println!("DOCKER_HOST: {}", host);
                println!("Console log: {}", console_log.display());

                if running {
                    match Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION) {
                        Ok(docker) => match docker.version().await {
                            Ok(v) => {
                                println!(
                                    "Docker engine: {}",
                                    v.version.unwrap_or_else(|| "unknown".into())
                                );
                            }
                            Err(e) => {
                                println!("Docker engine: not responding ({})", e);
                            }
                        },
                        Err(e) => {
                            println!("Docker engine: failed to connect ({})", e);
                        }
                    }
                }
            }
            RuntimeCommands::Start => {
                let host = match cratebay_core::runtime::ensure_runtime_linux_running() {
                    Ok(host) => host,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };

                println!(
                    "Runtime: CrateBay Runtime (Linux/{})",
                    if kvm_available {
                        "QEMU+KVM"
                    } else {
                        "QEMU/TCG"
                    }
                );
                println!("DOCKER_HOST: {}", host);
                println!("Console log: {}", console_log.display());

                match Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION) {
                    Ok(docker) => match docker.version().await {
                        Ok(v) => {
                            println!(
                                "Docker engine: {}",
                                v.version.unwrap_or_else(|| "unknown".into())
                            );
                        }
                        Err(e) => {
                            eprintln!("Docker engine check failed: {}", e);
                            std::process::exit(1);
                        }
                    },
                    Err(e) => {
                        eprintln!("Docker engine connect failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            RuntimeCommands::Stop => {
                if let Err(e) = cratebay_core::runtime::stop_runtime_linux() {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!("Stopped CrateBay Runtime (Linux/QEMU).");
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = cmd;
        eprintln!("CrateBay Runtime is not implemented on this platform yet.");
        std::process::exit(1);
    }
}

async fn handle_mount(cmd: MountCommands) {
    let addr = grpc_addr();
    let mut client = connect_vm_service_autostart(&addr).await;

    let hv = if client.is_none() {
        Some(cratebay_core::create_hypervisor())
    } else {
        None
    };

    match cmd {
        MountCommands::Add {
            vm,
            tag,
            host_path,
            guest_path,
            readonly,
        } => {
            if let Err(e) = validation::validate_mount_path(&host_path) {
                eprintln!("Error: invalid host path '{}': {}", host_path, e);
                std::process::exit(1);
            }
            if let Err(e) = validation::validate_mount_path(&guest_path) {
                eprintln!("Error: invalid guest path '{}': {}", guest_path, e);
                std::process::exit(1);
            }
            if let Some(client) = client.as_mut() {
                let vm_id = match resolve_vm_id_grpc(client, &vm).await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                let req = proto::MountVirtioFsRequest {
                    vm_id,
                    share: Some(proto::SharedDirectory {
                        tag: tag.clone(),
                        host_path: host_path.clone(),
                        guest_path: guest_path.clone(),
                        read_only: readonly,
                    }),
                };
                if let Err(e) = client.mount_virtio_fs(req).await {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!(
                    "Mounted '{}' → {} (tag: {}{})",
                    host_path,
                    guest_path,
                    tag,
                    if readonly { ", read-only" } else { "" }
                );
            } else {
                use cratebay_core::hypervisor::SharedDirectory;
                let hv = hv.as_ref().unwrap();
                let vm_id = match resolve_vm_id_local(hv.as_ref(), &vm) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                let share = SharedDirectory {
                    tag: tag.clone(),
                    host_path: host_path.clone(),
                    guest_path: guest_path.clone(),
                    read_only: readonly,
                };
                match hv.mount_virtiofs(&vm_id, &share) {
                    Ok(()) => {
                        println!(
                            "Mounted '{}' → {} (tag: {}{})",
                            host_path,
                            guest_path,
                            tag,
                            if readonly { ", read-only" } else { "" }
                        );
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        MountCommands::Remove { vm, tag } => {
            if let Some(client) = client.as_mut() {
                let vm_id = match resolve_vm_id_grpc(client, &vm).await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                if let Err(e) = client
                    .unmount_virtio_fs(proto::UnmountVirtioFsRequest {
                        vm_id,
                        tag: tag.clone(),
                    })
                    .await
                {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                println!("Unmounted tag '{}'", tag);
            } else {
                let hv = hv.as_ref().unwrap();
                let vm_id = match resolve_vm_id_local(hv.as_ref(), &vm) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                match hv.unmount_virtiofs(&vm_id, &tag) {
                    Ok(()) => println!("Unmounted tag '{}'", tag),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        MountCommands::List { vm } => {
            if let Some(client) = client.as_mut() {
                let vm_id = match resolve_vm_id_grpc(client, &vm).await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                let resp = client
                    .list_virtio_fs_mounts(proto::ListVirtioFsMountsRequest { vm_id })
                    .await;
                match resp {
                    Ok(r) => {
                        let mounts = r.into_inner().mounts;
                        if mounts.is_empty() {
                            println!("No VirtioFS mounts for VM '{}'.", vm);
                            return;
                        }
                        println!(
                            "{:<16} {:<30} {:<20} MODE",
                            "TAG", "HOST PATH", "GUEST PATH"
                        );
                        for m in mounts {
                            println!(
                                "{:<16} {:<30} {:<20} {}",
                                m.tag,
                                m.host_path,
                                m.guest_path,
                                if m.read_only { "ro" } else { "rw" }
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                let hv = hv.as_ref().unwrap();
                let vm_id = match resolve_vm_id_local(hv.as_ref(), &vm) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };
                match hv.list_virtiofs_mounts(&vm_id) {
                    Ok(mounts) => {
                        if mounts.is_empty() {
                            println!("No VirtioFS mounts for VM '{}'.", vm);
                            return;
                        }
                        println!(
                            "{:<16} {:<30} {:<20} MODE",
                            "TAG", "HOST PATH", "GUEST PATH"
                        );
                        for m in mounts {
                            println!(
                                "{:<16} {:<30} {:<20} {}",
                                m.tag,
                                m.host_path,
                                m.guest_path,
                                if m.read_only { "ro" } else { "rw" }
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct ImageSearchItem {
    source: &'static str,
    reference: String,
    description: String,
    stars: Option<u64>,
    pulls: Option<u64>,
    official: bool,
}

#[derive(Deserialize)]
struct DockerHubSearchResponse {
    results: Vec<DockerHubRepo>,
}

#[derive(Deserialize)]
struct DockerHubRepo {
    #[serde(alias = "repo_name")]
    name: String,
    #[serde(alias = "repo_owner")]
    namespace: Option<String>,
    #[serde(alias = "short_description")]
    description: Option<String>,
    star_count: Option<u64>,
    pull_count: Option<u64>,
    is_official: Option<bool>,
}

#[derive(Deserialize)]
struct DockerHubV1SearchResponse {
    results: Vec<DockerHubV1Repo>,
}

#[derive(Deserialize)]
struct DockerHubV1Repo {
    name: String,
    description: Option<String>,
    star_count: Option<u64>,
    is_official: Option<bool>,
}

#[derive(Deserialize)]
struct RegistryTagsResponse {
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RegistryTokenResponse {
    token: Option<String>,
    access_token: Option<String>,
}
