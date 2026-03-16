//! cratebay-vz — VM runner process using Apple Virtualization.framework.
//!
//! This binary is spawned by `MacOSHypervisor` (in cratebay-core) to run a
//! single Linux VM via the Virtualization.framework Swift bridge.
//!
//! On non-macOS platforms, it prints an error and exits.

#[cfg(target_os = "macos")]
mod ffi;

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("cratebay-vz is only supported on macOS");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
fn main() {
    cratebay_core::logging::init();

    let args = match Args::parse() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            eprintln!();
            eprintln!("{}", Args::usage());
            std::process::exit(2);
        }
    };

    if let Err(e) = run(args) {
        tracing::error!("{}", e);
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct Args {
    kernel: std::path::PathBuf,
    initrd: Option<std::path::PathBuf>,
    disk: std::path::PathBuf,
    cpus: u32,
    memory_mb: u64,
    cmdline: String,
    ready_file: Option<std::path::PathBuf>,
    console_log: Option<std::path::PathBuf>,
    rosetta: bool,
    /// Shared directories in "tag:host_path[:ro]" format.
    shared_dirs: Vec<String>,
    /// Vsock forwards in "guest_port:unix_socket_path" format.
    vsock_forwards: Vec<String>,
    /// TCP forwards in "guest_port:unix_socket_path" format (connects to guest IP).
    tcp_forwards: Vec<String>,
    /// Host TCP listeners forwarded to target host:port pairs.
    host_tcp_forwards: Vec<String>,
    /// Internal HTTP CONNECT proxies bound on the host bridge.
    http_connect_proxies: Vec<String>,
}

#[cfg(target_os = "macos")]
impl Args {
    fn usage() -> &'static str {
        "Usage:\n  cratebay-vz --kernel <path> --disk <path> --cpus <n> --memory-mb <n> \
         [--initrd <path>] [--cmdline <str>] [--ready-file <path>] \
         [--console-log <path>] [--rosetta] [--share tag:host_path[:ro]] \
         [--vsock-forward guest_port:unix_socket_path] \
         [--tcp-forward guest_port:unix_socket_path] \
         [--host-tcp-forward bind_host:bind_port=target_host:target_port] \
         [--http-connect-proxy bind_host:bind_port]\n"
    }

    fn parse() -> Result<Self, String> {
        let mut kernel: Option<std::path::PathBuf> = None;
        let mut initrd: Option<std::path::PathBuf> = None;
        let mut disk: Option<std::path::PathBuf> = None;
        let mut cpus: Option<u32> = None;
        let mut memory_mb: Option<u64> = None;
        let mut cmdline: Option<String> = None;
        let mut ready_file: Option<std::path::PathBuf> = None;
        let mut console_log: Option<std::path::PathBuf> = None;
        let mut rosetta = false;
        let mut shared_dirs: Vec<String> = Vec::new();
        let mut vsock_forwards: Vec<String> = Vec::new();
        let mut tcp_forwards: Vec<String> = Vec::new();
        let mut host_tcp_forwards: Vec<String> = Vec::new();
        let mut http_connect_proxies: Vec<String> = Vec::new();

        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    return Err(Self::usage().to_string());
                }
                "--kernel" => {
                    kernel = Some(
                        it.next()
                            .ok_or_else(|| "--kernel requires a value".to_string())?
                            .into(),
                    );
                }
                "--initrd" => {
                    initrd = Some(
                        it.next()
                            .ok_or_else(|| "--initrd requires a value".to_string())?
                            .into(),
                    );
                }
                "--disk" => {
                    disk = Some(
                        it.next()
                            .ok_or_else(|| "--disk requires a value".to_string())?
                            .into(),
                    );
                }
                "--cpus" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--cpus requires a value".to_string())?;
                    cpus = Some(
                        raw.parse::<u32>()
                            .map_err(|_| "Invalid --cpus".to_string())?,
                    );
                }
                "--memory-mb" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--memory-mb requires a value".to_string())?;
                    memory_mb = Some(
                        raw.parse::<u64>()
                            .map_err(|_| "Invalid --memory-mb".to_string())?,
                    );
                }
                "--cmdline" => {
                    cmdline = Some(
                        it.next()
                            .ok_or_else(|| "--cmdline requires a value".to_string())?,
                    );
                }
                "--ready-file" => {
                    ready_file = Some(
                        it.next()
                            .ok_or_else(|| "--ready-file requires a value".to_string())?
                            .into(),
                    );
                }
                "--console-log" => {
                    console_log = Some(
                        it.next()
                            .ok_or_else(|| "--console-log requires a value".to_string())?
                            .into(),
                    );
                }
                "--rosetta" => {
                    rosetta = true;
                }
                "--share" => {
                    shared_dirs.push(
                        it.next()
                            .ok_or_else(|| "--share requires a value".to_string())?,
                    );
                }
                "--vsock-forward" => {
                    vsock_forwards.push(
                        it.next()
                            .ok_or_else(|| "--vsock-forward requires a value".to_string())?,
                    );
                }
                "--tcp-forward" => {
                    tcp_forwards.push(
                        it.next()
                            .ok_or_else(|| "--tcp-forward requires a value".to_string())?,
                    );
                }
                "--host-tcp-forward" => {
                    host_tcp_forwards.push(
                        it.next()
                            .ok_or_else(|| "--host-tcp-forward requires a value".to_string())?,
                    );
                }
                "--http-connect-proxy" => {
                    http_connect_proxies.push(
                        it.next()
                            .ok_or_else(|| "--http-connect-proxy requires a value".to_string())?,
                    );
                }
                other => return Err(format!("Unknown argument: {}", other)),
            }
        }

        let kernel = kernel.ok_or_else(|| "Missing --kernel".to_string())?;
        let disk = disk.ok_or_else(|| "Missing --disk".to_string())?;
        let cpus = cpus.ok_or_else(|| "Missing --cpus".to_string())?;
        let memory_mb = memory_mb.ok_or_else(|| "Missing --memory-mb".to_string())?;
        let cmdline = cmdline.unwrap_or_else(|| "console=hvc0".to_string());

        Ok(Self {
            kernel,
            initrd,
            disk,
            cpus,
            memory_mb,
            cmdline,
            ready_file,
            console_log,
            rosetta,
            shared_dirs,
            vsock_forwards,
            tcp_forwards,
            host_tcp_forwards,
            http_connect_proxies,
        })
    }
}

#[cfg(target_os = "macos")]
fn parse_shared_dir(spec: &str) -> Result<ffi::SharedDirFFI, String> {
    // Format: "tag:host_path" or "tag:host_path:ro"
    // Tag is guaranteed to not contain colons (validated by mount_virtiofs).
    // We split on the first colon to get the tag, then check if the remainder
    // ends with ":ro" to determine read-only mode.
    let first_colon = spec.find(':').ok_or_else(|| {
        format!(
            "Invalid --share format '{}', expected 'tag:host_path[:ro]'",
            spec
        )
    })?;
    let tag = &spec[..first_colon];
    let rest = &spec[first_colon + 1..];

    let (host_path, read_only) = if let Some(stripped) = rest.strip_suffix(":ro") {
        (stripped, true)
    } else {
        (rest, false)
    };

    if tag.is_empty() || host_path.is_empty() {
        return Err(format!(
            "Invalid --share format '{}', expected 'tag:host_path[:ro]'",
            spec
        ));
    }

    let tag = std::ffi::CString::new(tag).map_err(|e| format!("invalid tag: {}", e))?;
    let host_path =
        std::ffi::CString::new(host_path).map_err(|e| format!("invalid host_path: {}", e))?;

    Ok(ffi::SharedDirFFI {
        tag,
        host_path,
        read_only,
    })
}

#[cfg(target_os = "macos")]
fn run(args: Args) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let kernel_path = args
        .kernel
        .to_str()
        .ok_or_else(|| "Kernel path is not valid UTF-8".to_string())?
        .to_string();
    let disk_path = args
        .disk
        .to_str()
        .ok_or_else(|| "Disk path is not valid UTF-8".to_string())?
        .to_string();
    let initrd_path = args
        .initrd
        .as_ref()
        .map(|p| {
            p.to_str()
                .ok_or_else(|| "Initrd path is not valid UTF-8".to_string())
                .map(|s| s.to_string())
        })
        .transpose()?;
    let console_log_path = args
        .console_log
        .as_ref()
        .map(|p| {
            p.to_str()
                .ok_or_else(|| "Console log path is not valid UTF-8".to_string())
                .map(|s| s.to_string())
        })
        .transpose()?;

    // Parse shared directory specs.
    let shared_dirs: Vec<ffi::SharedDirFFI> = args
        .shared_dirs
        .iter()
        .map(|s| parse_shared_dir(s))
        .collect::<Result<Vec<_>, _>>()?;

    let config = ffi::VmCreateConfig {
        kernel_path,
        initrd_path,
        cmdline: args.cmdline.clone(),
        disk_path,
        console_log_path,
        cpus: args.cpus,
        memory_mb: args.memory_mb,
        rosetta: args.rosetta,
        shared_dirs,
    };

    let handle = Arc::new(ffi::create_and_start_vm(&config)?);

    tracing::info!(
        "VZ VM started (pid {}, state {:?})",
        std::process::id(),
        handle.state()
    );

    // Set up SIGTERM handler for graceful ACPI shutdown.
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    {
        let flag = shutdown_requested.clone();
        // SAFETY: We only set an atomic bool inside the signal handler, which is
        // async-signal-safe in practice.
        unsafe {
            libc::signal(
                libc::SIGTERM,
                sigterm_handler as *const () as libc::sighandler_t,
            );
        }
        // Store the flag in a global so the C signal handler can access it.
        SHUTDOWN_FLAG.store(
            flag.as_ref() as *const AtomicBool as *mut AtomicBool,
            std::sync::atomic::Ordering::SeqCst,
        );
    }

    // Set up vsock forwards (host unix socket -> guest vsock port).
    let mut forward_threads = Vec::new();
    for spec in &args.vsock_forwards {
        let (guest_port, sock_path) = parse_vsock_forward(spec)?;
        let thread = start_vsock_forward(
            handle.clone(),
            guest_port,
            sock_path,
            shutdown_requested.clone(),
        )?;
        forward_threads.push(thread);
    }

    for spec in &args.http_connect_proxies {
        let (bind_host, bind_port) = parse_host_port(spec, "http connect proxy bind")?;
        let thread = start_http_connect_proxy(bind_host, bind_port, shutdown_requested.clone())?;
        forward_threads.push(thread);
    }

    for spec in &args.host_tcp_forwards {
        let ((bind_host, bind_port), (target_host, target_port)) = parse_host_tcp_forward(spec)?;
        let thread = start_host_tcp_forward(
            bind_host,
            bind_port,
            target_host,
            target_port,
            shutdown_requested.clone(),
        )?;
        forward_threads.push(thread);
    }

    // Set up TCP forwards (host unix socket -> guest tcp:port). This mode does
    // not require guest AF_VSOCK support; it discovers the guest IP from the
    // serial console log (printed by the runtime init).
    if !args.tcp_forwards.is_empty() {
        let console_log = args.console_log.as_ref().ok_or_else(|| {
            "--tcp-forward requires --console-log for guest IP discovery".to_string()
        })?;
        let guest_ip = wait_for_guest_ip(console_log, std::time::Duration::from_secs(30))?;
        for spec in &args.tcp_forwards {
            let (guest_port, sock_path) = parse_vsock_forward(spec)?;
            let thread =
                start_tcp_forward(guest_ip, guest_port, sock_path, shutdown_requested.clone())?;
            forward_threads.push(thread);
        }
    }

    // Signal readiness after forwards are bound.
    if let Some(path) = args.ready_file.as_ref() {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or_else(|| std::path::Path::new(".")));
        std::fs::write(path, b"ready\n")
            .map_err(|e| format!("Failed to write ready file: {}", e))?;
    }

    // Wait for SIGTERM or VM to stop on its own.
    loop {
        if shutdown_requested.load(Ordering::SeqCst) {
            tracing::info!("SIGTERM received, initiating graceful ACPI shutdown...");
            match handle.stop(15.0) {
                Ok(()) => tracing::info!("VM stopped gracefully"),
                Err(e) => tracing::warn!("VM stop error: {}", e),
            }
            break;
        }

        // Check if the VM has stopped on its own (e.g., guest shutdown).
        let state = handle.state();
        if state == ffi::VzState::Stopped || state == ffi::VzState::Error {
            tracing::info!("VM entered state {:?}, exiting.", state);
            shutdown_requested.store(true, Ordering::SeqCst);
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    for thread in forward_threads {
        let _ = thread.join();
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_vsock_forward(spec: &str) -> Result<(u32, std::path::PathBuf), String> {
    let first_colon = spec.find(':').ok_or_else(|| {
        format!(
            "Invalid --vsock-forward '{}', expected 'guest_port:unix_socket_path'",
            spec
        )
    })?;
    let port_str = &spec[..first_colon];
    let path_str = &spec[first_colon + 1..];

    let port = port_str
        .parse::<u32>()
        .map_err(|_| format!("Invalid vsock guest_port '{}'", port_str))?;
    if port == 0 {
        return Err("vsock guest_port must be > 0".to_string());
    }

    let path = std::path::PathBuf::from(path_str);

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        // macOS sockaddr_un.sun_path limit.
        const MAX_SUN_PATH: usize = 103;
        let bytes = path.as_os_str().as_bytes();
        if bytes.len() > MAX_SUN_PATH {
            return Err(format!(
                "Unix socket path too long ({} > {}): {}",
                bytes.len(),
                MAX_SUN_PATH,
                path.display()
            ));
        }
    }

    Ok((port, path))
}

#[cfg(target_os = "macos")]
fn parse_host_port(spec: &str, label: &str) -> Result<(String, u16), String> {
    if spec.starts_with('[') {
        let end = spec
            .find(']')
            .ok_or_else(|| format!("Invalid {} '{}': missing closing ']'", label, spec))?;
        let host = spec[1..end].to_string();
        let port = spec[end + 1..]
            .strip_prefix(':')
            .ok_or_else(|| format!("Invalid {} '{}': missing port", label, spec))?
            .parse::<u16>()
            .map_err(|_| format!("Invalid {} '{}': bad port", label, spec))?;
        return Ok((host, port));
    }

    let colon = spec
        .rfind(':')
        .ok_or_else(|| format!("Invalid {} '{}': expected host:port", label, spec))?;
    let host = spec[..colon].to_string();
    let port = spec[colon + 1..]
        .parse::<u16>()
        .map_err(|_| format!("Invalid {} '{}': bad port", label, spec))?;
    if host.trim().is_empty() {
        return Err(format!("Invalid {} '{}': empty host", label, spec));
    }
    Ok((host, port))
}

#[cfg(target_os = "macos")]
fn connect_target_tcp(target_host: &str, target_port: u16) -> Result<std::net::TcpStream, String> {
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let mut addresses = (target_host, target_port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve {}:{}: {}", target_host, target_port, error))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(format!(
            "resolve {}:{}: no addresses returned",
            target_host, target_port
        ));
    }

    addresses.sort_by_key(|address| if address.is_ipv4() { 0 } else { 1 });

    let mut last_error = None;
    for address in addresses {
        match std::net::TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(format!("{}: {}", address, error)),
        }
    }

    Err(format!(
        "connect to {}:{} failed: {}",
        target_host,
        target_port,
        last_error.unwrap_or_else(|| "unknown error".to_string())
    ))
}

#[cfg(target_os = "macos")]
type HostTcpForwardEndpoint = (String, u16);

#[cfg(target_os = "macos")]
fn parse_host_tcp_forward(
    spec: &str,
) -> Result<(HostTcpForwardEndpoint, HostTcpForwardEndpoint), String> {
    let (bind_spec, target_spec) = spec.split_once('=').ok_or_else(|| {
        format!(
            "Invalid --host-tcp-forward '{}': expected bind_host:bind_port=target_host:target_port",
            spec
        )
    })?;

    Ok((
        parse_host_port(bind_spec, "host tcp forward bind")?,
        parse_host_port(target_spec, "host tcp forward target")?,
    ))
}

#[cfg(target_os = "macos")]
fn start_vsock_forward(
    handle: std::sync::Arc<ffi::VmHandle>,
    guest_port: u32,
    sock_path: std::path::PathBuf,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use std::io;
    use std::os::unix::net::UnixListener;
    use std::time::Duration;

    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create socket dir {}: {}", parent.display(), e))?;
    }

    // Remove any stale socket from previous runs.
    let _ = std::fs::remove_file(&sock_path);

    let listener = UnixListener::bind(&sock_path)
        .map_err(|e| format!("Failed to bind unix socket {}: {}", sock_path.display(), e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set socket nonblocking: {}", e))?;

    tracing::info!(
        "vsock forward enabled: {} -> guest vsock:{}",
        sock_path.display(),
        guest_port
    );

    Ok(std::thread::spawn(move || {
        while !shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    let handle = handle.clone();
                    std::thread::spawn(move || {
                        let vsock_fd = match handle.vsock_connect(guest_port) {
                            Ok(fd) => fd,
                            Err(e) => {
                                tracing::warn!("vsock connect failed: {}", e);
                                return;
                            }
                        };

                        let vsock: std::fs::File = vsock_fd.into();
                        if let Err(e) = proxy_bidirectional(stream, vsock) {
                            tracing::debug!("vsock proxy ended: {}", e);
                        }
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    tracing::warn!(
                        "unix socket accept failed on {}: {}",
                        sock_path.display(),
                        e
                    );
                    break;
                }
            }
        }

        let _ = std::fs::remove_file(&sock_path);
    }))
}

#[cfg(target_os = "macos")]
fn wait_for_guest_ip(
    console_log_path: &std::path::Path,
    timeout: std::time::Duration,
) -> Result<std::net::IpAddr, String> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Ok(content) = std::fs::read_to_string(console_log_path) {
            for line in content.lines().rev().take(300) {
                let Some(idx) = line.find("guest_ip=") else {
                    continue;
                };
                let rest = &line[idx + "guest_ip=".len()..];
                let token = rest.split_whitespace().next().unwrap_or_default();
                if let Ok(ip) = token.parse::<std::net::IpAddr>() {
                    tracing::info!("guest ip discovered: {}", ip);
                    return Ok(ip);
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    Err(format!(
        "Timed out waiting for guest_ip=... in console log: {}",
        console_log_path.display()
    ))
}

#[cfg(target_os = "macos")]
fn start_tcp_forward(
    guest_ip: std::net::IpAddr,
    guest_port: u32,
    sock_path: std::path::PathBuf,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use std::io;
    use std::net::TcpStream;
    use std::os::unix::net::UnixListener;
    use std::time::Duration;

    let port = u16::try_from(guest_port)
        .map_err(|_| format!("Invalid tcp guest_port '{}': must be 1-65535", guest_port))?;
    if port == 0 {
        return Err("tcp guest_port must be > 0".to_string());
    }

    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create socket dir {}: {}", parent.display(), e))?;
    }

    // Remove any stale socket from previous runs.
    let _ = std::fs::remove_file(&sock_path);

    let listener = UnixListener::bind(&sock_path)
        .map_err(|e| format!("Failed to bind unix socket {}: {}", sock_path.display(), e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set socket nonblocking: {}", e))?;

    tracing::info!(
        "tcp forward enabled: {} -> guest tcp:{}:{}",
        sock_path.display(),
        guest_ip,
        port
    );

    Ok(std::thread::spawn(move || {
        while !shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    std::thread::spawn(move || {
                        let tcp = match TcpStream::connect_timeout(
                            &std::net::SocketAddr::new(guest_ip, port),
                            Duration::from_secs(2),
                        ) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!("tcp connect failed ({}:{}): {}", guest_ip, port, e);
                                return;
                            }
                        };
                        let _ = tcp.set_nodelay(true);
                        if let Err(e) = proxy_bidirectional_tcp(stream, tcp) {
                            tracing::debug!("tcp proxy ended: {}", e);
                        }
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    tracing::warn!(
                        "unix socket accept failed on {}: {}",
                        sock_path.display(),
                        e
                    );
                    break;
                }
            }
        }

        let _ = std::fs::remove_file(&sock_path);
    }))
}

#[cfg(target_os = "macos")]
fn start_host_tcp_forward(
    bind_host: String,
    bind_port: u16,
    target_host: String,
    target_port: u16,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use std::io;
    use std::net::TcpListener;
    use std::time::Duration;

    let bind_addr = format!("{}:{}", bind_host, bind_port);
    let listener = TcpListener::bind(&bind_addr)
        .map_err(|error| format!("Failed to bind host TCP forward {}: {}", bind_addr, error))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to set host TCP forward nonblocking: {}", error))?;

    tracing::info!(
        "host tcp forward enabled on {} -> {}:{}",
        bind_addr,
        target_host,
        target_port
    );

    Ok(std::thread::spawn(move || {
        while !shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((client, _addr)) => {
                    let target_host = target_host.clone();
                    std::thread::spawn(move || {
                        match connect_target_tcp(target_host.as_str(), target_port) {
                            Ok(server) => {
                                let _ = server.set_nodelay(true);
                                let _ = client.set_nodelay(true);
                                if let Err(error) = proxy_bidirectional_host_tcp(client, server) {
                                    tracing::debug!("host tcp forward ended: {}", error);
                                }
                            }
                            Err(error) => {
                                tracing::warn!(
                                    "host tcp forward connect failed ({}:{}): {}",
                                    target_host,
                                    target_port,
                                    error
                                );
                            }
                        }
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    tracing::warn!("host tcp forward accept failed on {}: {}", bind_addr, error);
                    break;
                }
            }
        }
    }))
}

#[cfg(target_os = "macos")]
fn start_http_connect_proxy(
    bind_host: String,
    bind_port: u16,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use std::io;
    use std::net::TcpListener;
    use std::time::Duration;

    let bind_addr = format!("{}:{}", bind_host, bind_port);
    let listener = TcpListener::bind(&bind_addr)
        .map_err(|error| format!("Failed to bind HTTP CONNECT proxy {}: {}", bind_addr, error))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to set HTTP CONNECT proxy nonblocking: {}", error))?;

    tracing::info!("http connect proxy enabled on {}", bind_addr);

    Ok(std::thread::spawn(move || {
        while !shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    std::thread::spawn(move || {
                        if let Err(error) = handle_http_connect_client(stream) {
                            tracing::debug!("http connect proxy client ended: {}", error);
                        }
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    tracing::warn!(
                        "http connect proxy accept failed on {}: {}",
                        bind_addr,
                        error
                    );
                    break;
                }
            }
        }
    }))
}

#[cfg(target_os = "macos")]
fn handle_http_connect_client(mut client: std::net::TcpStream) -> Result<(), String> {
    use std::io::{Read, Write};

    client
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|error| format!("client timeout: {}", error))?;

    let mut request = Vec::with_capacity(1024);
    let mut scratch = [0u8; 1024];
    let header_end = loop {
        let read = client
            .read(&mut scratch)
            .map_err(|error| format!("read request: {}", error))?;
        if read == 0 {
            return Err("client closed before proxy request completed".to_string());
        }
        request.extend_from_slice(&scratch[..read]);
        if request.len() > 16 * 1024 {
            return Err("proxy request headers too large".to_string());
        }

        if let Some(index) = request.windows(4).position(|chunk| chunk == b"\r\n\r\n") {
            break index + 4;
        }
        if let Some(index) = request.windows(2).position(|chunk| chunk == b"\n\n") {
            break index + 2;
        }
    };

    let request_head = &request[..header_end];
    let buffered_body = &request[header_end..];
    let request_text = String::from_utf8_lossy(request_head);
    let first_line = request_text
        .lines()
        .next()
        .ok_or_else(|| "empty proxy request".to_string())?;
    let Some(rest) = first_line.strip_prefix("CONNECT ") else {
        client
            .write_all(b"HTTP/1.1 501 Not Implemented\r\nConnection: close\r\n\r\n")
            .map_err(|error| format!("write 501: {}", error))?;
        return Err(format!("unsupported proxy request '{}'", first_line));
    };

    let target = rest
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("invalid CONNECT request '{}'", first_line))?;
    let (target_host, target_port) = parse_host_port(target, "CONNECT target")?;
    let server = connect_target_tcp(target_host.as_str(), target_port)?;
    let _ = server.set_nodelay(true);
    let _ = client.set_nodelay(true);
    let _ = client.set_read_timeout(None);

    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .map_err(|error| format!("write proxy response: {}", error))?;
    client
        .flush()
        .map_err(|error| format!("flush proxy response: {}", error))?;

    let mut server = server;
    if !buffered_body.is_empty() {
        server
            .write_all(buffered_body)
            .map_err(|error| format!("forward buffered client bytes: {}", error))?;
        server
            .flush()
            .map_err(|error| format!("flush buffered client bytes: {}", error))?;
    }

    proxy_bidirectional_host_tcp(client, server)
}

#[cfg(target_os = "macos")]
fn proxy_bidirectional(
    unix: std::os::unix::net::UnixStream,
    vsock: std::fs::File,
) -> Result<(), String> {
    use std::net::Shutdown;
    use std::os::fd::AsRawFd;

    let mut unix_r = unix.try_clone().map_err(|e| format!("unix clone: {}", e))?;
    let mut unix_w = unix;

    let mut vsock_r = vsock
        .try_clone()
        .map_err(|e| format!("vsock clone: {}", e))?;
    let mut vsock_w = vsock;

    let t1 = std::thread::spawn(move || {
        let _ = std::io::copy(&mut unix_r, &mut vsock_w);
        let _ = unsafe { libc::shutdown(vsock_w.as_raw_fd(), libc::SHUT_WR) };
    });

    let t2 = std::thread::spawn(move || {
        let _ = std::io::copy(&mut vsock_r, &mut unix_w);
        let _ = unix_w.shutdown(Shutdown::Write);
    });

    let _ = t1.join();
    let _ = t2.join();
    Ok(())
}

#[cfg(target_os = "macos")]
fn proxy_bidirectional_tcp(
    unix: std::os::unix::net::UnixStream,
    tcp: std::net::TcpStream,
) -> Result<(), String> {
    use std::net::Shutdown;

    let mut unix_r = unix.try_clone().map_err(|e| format!("unix clone: {}", e))?;
    let mut unix_w = unix;

    let mut tcp_r = tcp.try_clone().map_err(|e| format!("tcp clone: {}", e))?;
    let mut tcp_w = tcp;

    let t1 = std::thread::spawn(move || {
        let _ = std::io::copy(&mut unix_r, &mut tcp_w);
        let _ = tcp_w.shutdown(Shutdown::Write);
    });

    let t2 = std::thread::spawn(move || {
        let _ = std::io::copy(&mut tcp_r, &mut unix_w);
        let _ = unix_w.shutdown(Shutdown::Write);
    });

    let _ = t1.join();
    let _ = t2.join();
    Ok(())
}

#[cfg(target_os = "macos")]
fn proxy_bidirectional_host_tcp(
    inbound: std::net::TcpStream,
    outbound: std::net::TcpStream,
) -> Result<(), String> {
    inbound
        .set_nonblocking(true)
        .map_err(|error| format!("set inbound nonblocking: {}", error))?;
    outbound
        .set_nonblocking(true)
        .map_err(|error| format!("set outbound nonblocking: {}", error))?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|error| format!("build proxy runtime: {}", error))?;

    runtime.block_on(async move {
        let mut inbound = tokio::net::TcpStream::from_std(inbound)
            .map_err(|error| format!("wrap inbound tcp stream: {}", error))?;
        let mut outbound = tokio::net::TcpStream::from_std(outbound)
            .map_err(|error| format!("wrap outbound tcp stream: {}", error))?;

        tokio::io::copy_bidirectional(&mut inbound, &mut outbound)
            .await
            .map_err(|error| format!("copy bidirectional host tcp: {}", error))?;

        Ok(())
    })
}

#[cfg(target_os = "macos")]
static SHUTDOWN_FLAG: std::sync::atomic::AtomicPtr<std::sync::atomic::AtomicBool> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(target_os = "macos")]
extern "C" fn sigterm_handler(_sig: libc::c_int) {
    let ptr = SHUTDOWN_FLAG.load(std::sync::atomic::Ordering::SeqCst);
    if !ptr.is_null() {
        unsafe {
            (*ptr).store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}
