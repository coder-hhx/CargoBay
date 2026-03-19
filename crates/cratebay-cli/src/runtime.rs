fn detect_docker_socket() -> Option<String> {
    // Unix socket detection (macOS / Linux)
    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates = [
            cratebay_core::runtime::host_docker_socket_path()
                .to_string_lossy()
                .into_owned(),
            format!("{}/.colima/default/docker.sock", home),
            format!("{}/.orbstack/run/docker.sock", home),
            "/var/run/docker.sock".to_string(),
            format!("{}/.docker/run/docker.sock", home),
        ];
        if let Some(sock) = candidates.into_iter().find(|p| Path::new(p).exists()) {
            return Some(sock);
        }
    }

    // Windows named pipe detection
    #[cfg(windows)]
    {
        // Docker Desktop for Windows uses named pipes
        let candidates = [
            r"//./pipe/docker_engine",
            r"//./pipe/dockerDesktopLinuxEngine",
        ];
        for pipe in &candidates {
            if Path::new(pipe).exists() {
                return Some(pipe.to_string());
            }
        }
        // WSL2 Docker socket
        let userprofile = std::env::var("USERPROFILE").unwrap_or_default();
        let wsl_sock = format!(r"{}\\.docker\run\docker.sock", userprofile);
        if Path::new(&wsl_sock).exists() {
            return Some(wsl_sock);
        }
    }

    None
}

#[cfg(unix)]
fn docker_ping_unix_socket(sock: &Path) -> Result<(), String> {
    use std::io::{Read, Write as _};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut stream =
        UnixStream::connect(sock).map_err(|e| format!("connect {}: {}", sock.display(), e))?;

    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));

    stream
        .write_all(
            b"GET /_ping HTTP/1.1\r\nHost: docker\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        )
        .map_err(|e| format!("write _ping: {}", e))?;

    // Read more than a single packet: Docker may include enough headers that
    // the "OK" body isn't within the first 256 bytes.
    let mut out: Vec<u8> = Vec::with_capacity(2048);
    let mut buf = [0u8; 1024];
    for _ in 0..16 {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return Err(format!("read _ping: {}", e)),
        };
        out.extend_from_slice(&buf[..n]);
        if out.len() >= 8192 {
            break;
        }
        // Fast-path: once we see the end of headers, we likely have the body.
        if out.windows(4).any(|w| w == b"\r\n\r\n") && out.windows(2).any(|w| w == b"OK") {
            break;
        }
    }
    let resp = String::from_utf8_lossy(&out);

    if resp.contains("200 OK") && resp.contains("\r\n\r\nOK") {
        return Ok(());
    }
    if resp.contains("\r\n\r\nOK") || resp.trim_end() == "OK" {
        return Ok(());
    }

    Err(format!(
        "unexpected /_ping response: {}",
        resp.lines().next().unwrap_or_default()
    ))
}

#[cfg(target_os = "macos")]
fn wait_for_docker_socket_ready(sock: &Path, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if sock.exists() && docker_ping_unix_socket(sock).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    Err(format!(
        "Docker socket was not ready within {}s: {}",
        timeout.as_secs(),
        sock.display()
    ))
}

#[cfg(target_os = "macos")]
static MACOS_RUNTIME_CONNECT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(target_os = "macos")]
fn macos_runtime_connect_lock() -> &'static Mutex<()> {
    MACOS_RUNTIME_CONNECT_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(target_os = "macos")]
fn prime_macos_runtime_assets_env() {
    if std::env::var_os("CRATEBAY_RUNTIME_ASSETS_DIR").is_none() {
        if let Some(dir) = cratebay_core::runtime::bundled_runtime_assets_dir() {
            std::env::set_var("CRATEBAY_RUNTIME_ASSETS_DIR", dir);
        }
    }
}

#[cfg(target_os = "macos")]
fn connect_cratebay_runtime_docker() -> Result<Docker, String> {
    let _guard = macos_runtime_connect_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if std::env::var_os("CRATEBAY_RUNTIME_ASSETS_DIR").is_none() {
        if let Some(dir) = cratebay_core::runtime::bundled_runtime_assets_dir() {
            std::env::set_var("CRATEBAY_RUNTIME_ASSETS_DIR", dir);
        }
    }

    let cratebay_sock = cratebay_core::runtime::host_docker_socket_path().to_path_buf();
    if cratebay_sock.exists() && docker_ping_unix_socket(&cratebay_sock).is_ok() {
        return Docker::connect_with_socket(
            cratebay_sock
                .to_str()
                .ok_or_else(|| "Docker socket path is not valid UTF-8".to_string())?,
            120,
            bollard::API_DEFAULT_VERSION,
        )
        .map_err(|e| {
            format!(
                "Failed to connect to CrateBay Runtime at {}: {}",
                cratebay_sock.display(),
                e
            )
        });
    }

    let hv = cratebay_core::create_hypervisor();
    let start_attempt = |label: &str| -> Result<String, String> {
        let vm_id = cratebay_core::runtime::ensure_runtime_vm_running(hv.as_ref())
            .map_err(|e| format!("{} failed to start CrateBay Runtime VM: {}", label, e))?;
        wait_for_docker_socket_ready(&cratebay_sock, Duration::from_secs(45)).map_err(|e| {
            format!(
                "{} failed waiting for Docker socket (vm: {}, sock: {}): {}",
                label,
                vm_id,
                cratebay_sock.display(),
                e
            )
        })?;
        Ok(vm_id)
    };

    let start_runtime = || -> Result<String, String> {
        if let Ok(vm_id) = start_attempt("start") {
            return Ok(vm_id);
        }

        let _ = cratebay_core::runtime::stop_runtime_vm_if_exists(hv.as_ref());
        let _ = std::fs::remove_file(&cratebay_sock);
        if let Ok(vm_id) = start_attempt("restart") {
            return Ok(vm_id);
        }

        let _ = cratebay_core::runtime::stop_runtime_vm_if_exists(hv.as_ref());
        let _ = std::fs::remove_file(&cratebay_sock);
        cratebay_core::runtime::reset_runtime_vm(hv.as_ref())
            .map_err(|e| format!("reset failed to recreate CrateBay Runtime VM: {}", e))?;
        start_attempt("reset")
    };

    let _vm_id = start_runtime()?;
    Docker::connect_with_socket(
        cratebay_sock
            .to_str()
            .ok_or_else(|| "Docker socket path is not valid UTF-8".to_string())?,
        120,
        bollard::API_DEFAULT_VERSION,
    )
    .map_err(|e| {
        format!(
            "Failed to connect to CrateBay Runtime at {}: {}",
            cratebay_sock.display(),
            e
        )
    })
}

#[cfg(target_os = "linux")]
fn connect_cratebay_runtime_docker() -> Result<Docker, String> {
    let host = cratebay_core::runtime::ensure_runtime_linux_running()
        .map_err(|e| format!("Failed to start CrateBay Runtime (Linux/QEMU): {}", e))?;
    std::env::set_var("DOCKER_HOST", &host);

    Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)
        .map_err(|e| format!("Failed to connect to CrateBay Runtime at {}: {}", host, e))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DockerHostTarget {
    Tcp(String),
    Unix(String),
    NamedPipe(String),
    Unsupported(String),
}

fn parse_docker_host_target(host: &str) -> DockerHostTarget {
    let trimmed = host.trim();

    if trimmed.starts_with("tcp://") || trimmed.starts_with("http://") {
        return DockerHostTarget::Tcp(trimmed.to_string());
    }

    if let Some(path) = trimmed.strip_prefix("unix://") {
        return DockerHostTarget::Unix(path.to_string());
    }

    if let Some(path) = trimmed.strip_prefix("npipe://") {
        return DockerHostTarget::NamedPipe(path.to_string());
    }

    if trimmed.starts_with("//./pipe/") || trimmed.starts_with(r"\\.\pipe\") {
        return DockerHostTarget::NamedPipe(trimmed.to_string());
    }

    if trimmed.starts_with('/') {
        return DockerHostTarget::Unix(trimmed.to_string());
    }

    DockerHostTarget::Unsupported(trimmed.to_string())
}

#[cfg(windows)]
fn parse_tcp_docker_host_endpoint(host: &str) -> Option<(String, u16)> {
    let endpoint = host.strip_prefix("tcp://")?;

    if endpoint.starts_with('[') {
        let end = endpoint.find(']')?;
        let host_part = endpoint.get(1..end)?.to_string();
        let port = endpoint.get(end + 1..)?.strip_prefix(':')?.parse().ok()?;
        return Some((host_part, port));
    }

    let (host_part, port_part) = endpoint.rsplit_once(':')?;
    let port = port_part.parse().ok()?;
    if host_part.trim().is_empty() {
        return None;
    }
    Some((host_part.to_string(), port))
}

#[cfg(windows)]
async fn wait_for_docker_http_ready(host: &str, timeout: Duration) -> Result<(), String> {
    let request_timeout = timeout
        .min(Duration::from_secs(2))
        .max(Duration::from_secs(1));
    let docker = Docker::connect_with_http(
        host,
        request_timeout.as_secs(),
        bollard::API_DEFAULT_VERSION,
    )
    .map_err(|e| format!("Failed to connect to Docker at {}: {}", host, e))?;
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let last_error = match tokio::time::timeout(request_timeout, docker.version()).await {
            Ok(Ok(_)) => return Ok(()),
            Ok(Err(error)) => error.to_string(),
            Err(_) => format!(
                "timed out waiting {} seconds for Docker at {}",
                request_timeout.as_secs(),
                host
            ),
        };

        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "Docker runtime at {} did not become ready within {} seconds: {}",
                host,
                timeout.as_secs(),
                last_error
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(windows)]
async fn docker_http_engine_version(host: &str, timeout: Duration) -> Result<String, String> {
    let request_timeout = timeout.max(Duration::from_secs(1));
    let docker = Docker::connect_with_http(
        host,
        request_timeout.as_secs(),
        bollard::API_DEFAULT_VERSION,
    )
    .map_err(|e| format!("Failed to connect to Docker at {}: {}", host, e))?;

    match tokio::time::timeout(timeout, docker.version()).await {
        Ok(Ok(version)) => Ok(version.version.unwrap_or_else(|| "unknown".into())),
        Ok(Err(error)) => Err(format!("Docker engine check failed: {}", error)),
        Err(_) => Err(format!(
            "Docker engine check timed out after {} seconds at {}",
            timeout.as_secs(),
            host
        )),
    }
}

#[cfg(windows)]
async fn ensure_windows_runtime_terminal_host() -> Result<String, String> {
    let guest = cratebay_core::runtime::ensure_runtime_wsl_guest_host()
        .map_err(|e| format!("Failed to start CrateBay Runtime (WSL2): {}", e))?;
    let (guest_ip, port) = parse_tcp_docker_host_endpoint(&guest)
        .ok_or_else(|| format!("Invalid WSL Docker host '{}'", guest))?;

    if wait_for_docker_http_ready(&guest, Duration::from_secs(5))
        .await
        .is_ok()
    {
        return Ok(guest);
    }

    let _ = kill_runtime_proxy_process();
    let relay_port = pick_free_localhost_port()?;
    let relay_host = format!("tcp://127.0.0.1:{relay_port}");
    spawn_runtime_proxy_detached(&guest_ip, relay_port, port)?;

    if wait_for_docker_http_ready(&relay_host, Duration::from_secs(20))
        .await
        .is_ok()
    {
        return Ok(relay_host);
    }

    wait_for_docker_http_ready(&guest, Duration::from_secs(5))
        .await
        .map(|_| guest)
        .map_err(|relay_error| {
            format!(
                "Failed to reach CrateBay Runtime (WSL2) through the direct guest endpoint or detached local relay: {}",
                relay_error
            )
        })
}

#[cfg(windows)]
fn cratebay_runtime_linux_platform() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "linux/arm64"
    }

    #[cfg(target_arch = "x86_64")]
    {
        "linux/amd64"
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "linux/amd64"
    }
}

fn connect_docker_with_host(host: &str) -> Result<Docker, String> {
    match parse_docker_host_target(host) {
        DockerHostTarget::Tcp(endpoint) => {
            Docker::connect_with_http(&endpoint, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| format!("Failed to connect to Docker at {}: {}", endpoint, e))
        }
        DockerHostTarget::Unix(socket) => {
            Docker::connect_with_socket(&socket, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| format!("Failed to connect to Docker at {}: {}", socket, e))
        }
        DockerHostTarget::NamedPipe(pipe) => {
            #[cfg(windows)]
            {
                Docker::connect_with_named_pipe(&pipe, 120, bollard::API_DEFAULT_VERSION)
                    .map_err(|e| format!("Failed to connect to Docker at {}: {}", pipe, e))
            }

            #[cfg(not(windows))]
            {
                Err(format!(
                    "Windows named pipe DOCKER_HOST is only supported on Windows: {}",
                    pipe
                ))
            }
        }
        DockerHostTarget::Unsupported(value) => {
            Err(format!("Unsupported DOCKER_HOST value: {}", value))
        }
    }
}

fn connect_docker() -> Result<Docker, String> {
    // Check DOCKER_HOST env first
    if let Ok(host) = std::env::var("DOCKER_HOST") {
        return connect_docker_with_host(&host);
    }

    #[cfg(unix)]
    {
        if let Some(sock) = detect_docker_socket() {
            let sock_path = Path::new(&sock);
            if docker_ping_unix_socket(sock_path).is_ok() {
                return Docker::connect_with_socket(&sock, 120, bollard::API_DEFAULT_VERSION)
                    .map_err(|e| format!("Failed to connect to Docker at {}: {}", sock, e));
            }

            #[cfg(not(target_os = "macos"))]
            {
                return Err(format!(
                    "Docker socket exists but Docker engine is not responding: {}",
                    sock
                ));
            }
        }

        // No runtime detected; start the built-in runtime when available.
        #[cfg(target_os = "macos")]
        {
            connect_cratebay_runtime_docker()
        }

        #[cfg(target_os = "linux")]
        {
            connect_cratebay_runtime_docker()
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        Err("No Docker socket found. Set DOCKER_HOST or start a Docker-compatible runtime.".into())
    }

    #[cfg(windows)]
    {
        let candidates = [
            r"//./pipe/docker_engine",
            r"//./pipe/dockerDesktopLinuxEngine",
        ];
        for pipe in &candidates {
            if let Ok(d) = Docker::connect_with_named_pipe(pipe, 120, bollard::API_DEFAULT_VERSION)
            {
                return Ok(d);
            }
        }
        if let Ok(host) = cratebay_core::runtime::ensure_runtime_wsl_running() {
            std::env::set_var("DOCKER_HOST", &host);
            std::env::set_var(
                "CRATEBAY_DOCKER_PLATFORM",
                cratebay_runtime_linux_platform(),
            );
            return Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| format!("Failed to connect to CrateBay Runtime at {}: {}", host, e));
        }
        Err(
            "No Docker named pipe found. Set DOCKER_HOST or start a Docker-compatible runtime."
                .into(),
        )
    }

    #[cfg(not(any(unix, windows)))]
    {
        Docker::connect_with_local_defaults()
            .map_err(|e| format!("Failed to connect to Docker: {}", e))
    }
}

async fn run_cli(cli: Cli) {
    match cli.command {
        Commands::Vm { command } => handle_vm(command).await,
        Commands::Runtime { command } => handle_runtime(command).await,
        Commands::Docker { command } => {
            if let Err(e) = handle_docker(command).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Image { command } => {
            if let Err(e) = handle_image(command).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Mount { command } => handle_mount(command).await,
        Commands::Volume { command } => {
            if let Err(e) = handle_volume(command).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::K3s { command } => {
            if let Err(e) = handle_k3s(command).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Status => {
            println!("CrateBay v{}", env!("CARGO_PKG_VERSION"));
            println!("Platform: {}", cratebay_core::platform_info());
            let hv = cratebay_core::create_hypervisor();
            println!(
                "Rosetta x86_64: {}",
                if hv.rosetta_available() {
                    "available"
                } else {
                    "not available"
                }
            );
            match detect_docker_socket() {
                Some(sock) => println!("Docker: connected ({})", sock),
                None => println!("Docker: not found"),
            }

            let addr = grpc_addr();
            match connect_vm_service(&addr).await {
                Ok(_) => println!("Daemon gRPC: connected ({})", addr),
                Err(_) => println!("Daemon gRPC: not running ({})", addr),
            }
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "cratebay", &mut std::io::stdout());
        }
        Commands::InternalRuntimeProxy {
            guest_ip,
            listen_port,
            target_port,
        } => {
            #[cfg(windows)]
            {
                if let Err(error) = cratebay_core::runtime::run_wsl_host_relay_server(
                    listen_port,
                    &guest_ip,
                    target_port,
                ) {
                    eprintln!("Error: {}", error);
                    std::process::exit(1);
                }
            }

            #[cfg(not(windows))]
            {
                let _ = (guest_ip, listen_port, target_port);
                eprintln!("Error: internal runtime proxy is only available on Windows.");
                std::process::exit(1);
            }
        }
    }
}
