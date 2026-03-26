#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("cratebay-guest-agent is only supported on Linux guests");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(e) = run() {
        eprintln!("cratebay-guest-agent: {}", e);
        std::process::exit(1);
    }
}

#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    raise_nofile_limit();
    let cfg = Config::from_env_and_args()?;

    eprintln!(
        "cratebay-guest-agent: mode={:?} docker_socket={} docker_host_tcp={:?}",
        cfg.listen,
        cfg.docker_socket.display(),
        cfg.docker_host_tcp
    );

    match cfg.listen {
        ListenMode::Vsock { port } => run_vsock(port, cfg.docker_socket),
        ListenMode::Tcp { addr } => run_tcp(addr, cfg.docker_socket),
        ListenMode::Connect { addr } => run_connect(
            addr,
            cfg.docker_socket,
            cfg.docker_host_tcp,
            cfg.connect_pool_size,
        ),
    }
}

#[cfg(target_os = "linux")]
fn raise_nofile_limit() {
    let desired: libc::rlim_t = 65_536;
    let mut current = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    let rc = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut current) };
    if rc != 0 {
        return;
    }

    let target_max = current.rlim_max.max(desired);
    let target_cur = current.rlim_cur.max(desired).min(target_max);
    let updated = libc::rlimit {
        rlim_cur: target_cur,
        rlim_max: target_max,
    };

    let _ = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &updated) };
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct Config {
    listen: ListenMode,
    docker_socket: std::path::PathBuf,
    docker_host_tcp: Option<std::net::SocketAddr>,
    connect_pool_size: usize,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
enum ListenMode {
    Vsock { port: u32 },
    Tcp { addr: std::net::SocketAddr },
    Connect { addr: std::net::SocketAddr },
}

#[cfg(target_os = "linux")]
impl Config {
    fn from_env_and_args() -> Result<Self, String> {
        use std::path::PathBuf;

        let mut port: u32 = std::env::var("CRATEBAY_DOCKER_PROXY_PORT")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .or_else(|| {
                std::env::var("CRATEBAY_DOCKER_VSOCK_PORT")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|v| *v > 0)
            })
            .unwrap_or(6237);

        let mut docker_socket = std::env::var("CRATEBAY_GUEST_DOCKER_SOCK")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/run/docker.sock"));
        let mut docker_host_tcp: Option<std::net::SocketAddr> =
            std::env::var("CRATEBAY_GUEST_DOCKER_HOST")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| v.parse::<std::net::SocketAddr>().ok());

        let mut tcp_mode = false;
        let mut connect_mode = false;
        let mut tcp_listen: Option<std::net::SocketAddr> =
            std::env::var("CRATEBAY_GUEST_TCP_LISTEN")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| v.parse::<std::net::SocketAddr>().ok());
        let mut tcp_connect: Option<std::net::SocketAddr> =
            std::env::var("CRATEBAY_GUEST_TCP_CONNECT")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| v.parse::<std::net::SocketAddr>().ok());
        let mut connect_pool_size: usize = std::env::var("CRATEBAY_GUEST_CONNECT_POOL_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(4);

        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--tcp" => {
                    tcp_mode = true;
                }
                "--connect" => {
                    connect_mode = true;
                    let raw = it
                        .next()
                        .ok_or_else(|| "--connect requires a value".to_string())?;
                    tcp_connect = Some(
                        raw.parse::<std::net::SocketAddr>()
                            .map_err(|_| "Invalid --connect (expected ip:port)".to_string())?,
                    );
                }
                "--port" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--port requires a value".to_string())?;
                    port = raw
                        .parse::<u32>()
                        .map_err(|_| "Invalid --port".to_string())?;
                    if port == 0 {
                        return Err("--port must be > 0".to_string());
                    }
                }
                "--listen" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--listen requires a value".to_string())?;
                    tcp_listen = Some(
                        raw.parse::<std::net::SocketAddr>()
                            .map_err(|_| "Invalid --listen (expected ip:port)".to_string())?,
                    );
                }
                "--docker-sock" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--docker-sock requires a value".to_string())?;
                    docker_socket = PathBuf::from(raw);
                }
                "--docker-host-tcp" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--docker-host-tcp requires a value".to_string())?;
                    docker_host_tcp =
                        Some(raw.parse::<std::net::SocketAddr>().map_err(|_| {
                            "Invalid --docker-host-tcp (expected ip:port)".to_string()
                        })?);
                }
                "--connect-pool-size" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--connect-pool-size requires a value".to_string())?;
                    connect_pool_size = raw
                        .parse::<usize>()
                        .map_err(|_| "Invalid --connect-pool-size".to_string())?;
                    if connect_pool_size == 0 {
                        return Err("--connect-pool-size must be > 0".to_string());
                    }
                }
                "--help" | "-h" => {
                    return Err(Self::usage().to_string());
                }
                other => return Err(format!("Unknown argument: {}", other)),
            }
        }

        let listen = if connect_mode {
            let addr =
                tcp_connect.ok_or_else(|| "--connect requires an ip:port target".to_string())?;
            ListenMode::Connect { addr }
        } else if tcp_mode {
            let port_u16 = u16::try_from(port)
                .map_err(|_| format!("--port out of range for TCP (must be 1-65535): {}", port))?;
            let addr =
                tcp_listen.unwrap_or_else(|| std::net::SocketAddr::from(([0, 0, 0, 0], port_u16)));
            ListenMode::Tcp { addr }
        } else {
            ListenMode::Vsock { port }
        };

        Ok(Self {
            listen,
            docker_socket,
            docker_host_tcp,
            connect_pool_size,
        })
    }

    fn usage() -> &'static str {
        "Usage:\n  cratebay-guest-agent [--tcp] [--connect <ip:port>] [--connect-pool-size <n>] [--port <port>] [--listen <ip:port>] [--docker-sock <path>] [--docker-host-tcp <ip:port>]\n\n\
Modes:\n  (default) vsock: listen on AF_VSOCK port\n  --tcp               : listen on TCP (default 0.0.0.0:<port>)\n  --connect <ip:port> : connect outward over TCP and proxy to Docker socket\n  --connect-pool-size : number of concurrent reverse-TCP workers (default 4)\n\n\
Env:\n  CRATEBAY_DOCKER_PROXY_PORT        Guest proxy listen port (default 6237)\n  \
CRATEBAY_DOCKER_VSOCK_PORT         Back-compat for proxy port (default 6237)\n  \
CRATEBAY_GUEST_TCP_LISTEN          TCP listen addr override (e.g. 0.0.0.0:6237)\n  \
CRATEBAY_GUEST_TCP_CONNECT         TCP connect target override (e.g. 192.168.64.1:6237)\n  \
CRATEBAY_GUEST_CONNECT_POOL_SIZE   Reverse-TCP worker pool size (default 4)\n  \
CRATEBAY_GUEST_DOCKER_HOST         Guest Docker TCP endpoint override (e.g. 127.0.0.1:2375)\n  \
CRATEBAY_GUEST_DOCKER_SOCK         Guest Docker unix socket path (default /var/run/docker.sock)\n"
    }
}

#[cfg(target_os = "linux")]
fn vsock_listen(port: u32) -> Result<i32, String> {
    // Some libc versions don't expose VMADDR_CID_ANY, keep it local.
    const VMADDR_CID_ANY: u32 = 0xFFFF_FFFF;

    #[repr(C)]
    struct SockAddrVm {
        svm_family: libc::sa_family_t,
        svm_reserved1: libc::c_ushort,
        svm_port: libc::c_uint,
        svm_cid: libc::c_uint,
        svm_zero: [libc::c_uchar; 4],
    }

    let fd = unsafe { libc::socket(libc::AF_VSOCK, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(format!(
            "socket(AF_VSOCK) failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    let addr = SockAddrVm {
        svm_family: libc::AF_VSOCK as libc::sa_family_t,
        svm_reserved1: 0,
        svm_port: port as libc::c_uint,
        svm_cid: VMADDR_CID_ANY as libc::c_uint,
        svm_zero: [0; 4],
    };

    let rc = unsafe {
        libc::bind(
            fd,
            &addr as *const SockAddrVm as *const libc::sockaddr,
            std::mem::size_of::<SockAddrVm>() as libc::socklen_t,
        )
    };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(format!("bind(vsock:{}) failed: {}", port, err));
    }

    let rc = unsafe { libc::listen(fd, 128) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(format!("listen failed: {}", err));
    }

    Ok(fd)
}

#[cfg(target_os = "linux")]
fn run_vsock(port: u32, docker_socket: std::path::PathBuf) -> Result<(), String> {
    use std::os::fd::FromRawFd;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let listener_fd = vsock_listen(port)?;
    eprintln!("vsock: listening on port {}", port);
    eprintln!(
        "cratebay-guest-agent listening: vsock:{} -> {}",
        port,
        docker_socket.display()
    );

    loop {
        let conn_fd =
            unsafe { libc::accept(listener_fd, std::ptr::null_mut(), std::ptr::null_mut()) };
        if conn_fd < 0 {
            return Err(format!(
                "accept failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        let docker_socket = docker_socket.clone();
        std::thread::spawn(move || {
            let client = unsafe { std::fs::File::from_raw_fd(conn_fd) };
            if let Err(error) =
                wait_for_docker_socket_ready(&docker_socket, Duration::from_secs(30))
            {
                eprintln!(
                    "cratebay-guest-agent: wait for {} failed: {}",
                    docker_socket.display(),
                    error
                );
                return;
            }
            let docker = match UnixStream::connect(&docker_socket) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "cratebay-guest-agent: connect {} failed: {}",
                        docker_socket.display(),
                        e
                    );
                    return;
                }
            };

            if let Err(e) = proxy_unix_to_file(docker, client) {
                eprintln!("cratebay-guest-agent: proxy ended: {}", e);
            }
        });
    }
}

#[cfg(target_os = "linux")]
fn run_tcp(addr: std::net::SocketAddr, docker_socket: std::path::PathBuf) -> Result<(), String> {
    use std::net::TcpListener;
    use std::net::TcpStream;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let listener = TcpListener::bind(addr).map_err(|e| format!("bind tcp {}: {}", addr, e))?;
    eprintln!("tcp: listening on {}", addr);
    eprintln!(
        "cratebay-guest-agent listening: tcp:{} -> {}",
        addr,
        docker_socket.display()
    );

    for conn in listener.incoming() {
        let stream: TcpStream = match conn {
            Ok(s) => s,
            Err(e) => return Err(format!("accept tcp failed: {}", e)),
        };

        let docker_socket = docker_socket.clone();
        std::thread::spawn(move || {
            if let Err(error) =
                wait_for_docker_socket_ready(&docker_socket, Duration::from_secs(30))
            {
                eprintln!(
                    "cratebay-guest-agent: wait for {} failed: {}",
                    docker_socket.display(),
                    error
                );
                return;
            }
            let docker = match UnixStream::connect(&docker_socket) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "cratebay-guest-agent: connect {} failed: {}",
                        docker_socket.display(),
                        e
                    );
                    return;
                }
            };

            if let Err(e) = proxy_unix_to_tcp(docker, stream) {
                eprintln!("cratebay-guest-agent: proxy ended: {}", e);
            }
        });
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn run_connect(
    addr: std::net::SocketAddr,
    docker_socket: std::path::PathBuf,
    docker_host_tcp: Option<std::net::SocketAddr>,
    connect_pool_size: usize,
) -> Result<(), String> {
    let docker_target = docker_host_tcp
        .map(|target| target.to_string())
        .unwrap_or_else(|| docker_socket.display().to_string());
    eprintln!(
        "connect: dialing {} (pool_size={})",
        addr, connect_pool_size
    );
    eprintln!(
        "cratebay-guest-agent connecting: tcp:{} -> {} (workers={})",
        addr, docker_target, connect_pool_size
    );

    let mut workers = Vec::with_capacity(connect_pool_size);
    for worker_id in 0..connect_pool_size {
        let docker_socket = docker_socket.clone();
        workers.push(std::thread::spawn(move || {
            run_connect_worker(worker_id, addr, docker_socket, docker_host_tcp)
        }));
    }

    for worker in workers {
        worker
            .join()
            .map_err(|_| "connect worker thread panicked".to_string())?;
    }

    Err("all connect workers exited unexpectedly".to_string())
}

#[cfg(target_os = "linux")]
fn run_connect_worker(
    worker_id: usize,
    addr: std::net::SocketAddr,
    docker_socket: std::path::PathBuf,
    docker_host_tcp: Option<std::net::SocketAddr>,
) {
    use std::net::TcpStream;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    loop {
        match docker_host_tcp {
            Some(target) => {
                if let Err(error) = wait_for_docker_host_ready(target, Duration::from_secs(30)) {
                    eprintln!(
                        "cratebay-guest-agent[{}]: wait for {} failed: {}",
                        worker_id, target, error
                    );
                    std::thread::sleep(Duration::from_millis(250));
                    continue;
                }
            }
            None => {
                if let Err(error) =
                    wait_for_docker_socket_ready(&docker_socket, Duration::from_secs(30))
                {
                    eprintln!(
                        "cratebay-guest-agent[{}]: wait for {} failed: {}",
                        worker_id,
                        docker_socket.display(),
                        error
                    );
                    std::thread::sleep(Duration::from_millis(250));
                    continue;
                }
            }
        }

        match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                eprintln!(
                    "cratebay-guest-agent[{}]: connected to host {}",
                    worker_id, addr
                );

                if let Some(target) = docker_host_tcp {
                    let docker = match TcpStream::connect_timeout(&target, Duration::from_secs(2)) {
                        Ok(socket) => socket,
                        Err(error) => {
                            eprintln!(
                                "cratebay-guest-agent[{}]: connect {} failed: {}",
                                worker_id, target, error
                            );
                            std::thread::sleep(Duration::from_millis(250));
                            continue;
                        }
                    };
                    let _ = docker.set_nodelay(true);
                    eprintln!(
                        "cratebay-guest-agent[{}]: connected to docker tcp {}",
                        worker_id, target
                    );

                    if let Err(error) = proxy_tcp_to_tcp(docker, stream) {
                        eprintln!(
                            "cratebay-guest-agent[{}]: proxy ended: {}",
                            worker_id, error
                        );
                    } else {
                        eprintln!("cratebay-guest-agent[{}]: proxy completed", worker_id);
                    }
                } else {
                    let docker = match UnixStream::connect(&docker_socket) {
                        Ok(s) => s,
                        Err(error) => {
                            eprintln!(
                                "cratebay-guest-agent[{}]: connect {} failed: {}",
                                worker_id,
                                docker_socket.display(),
                                error
                            );
                            std::thread::sleep(Duration::from_millis(250));
                            continue;
                        }
                    };
                    eprintln!(
                        "cratebay-guest-agent[{}]: connected to docker socket {}",
                        worker_id,
                        docker_socket.display()
                    );

                    if let Err(error) = proxy_unix_to_tcp(docker, stream) {
                        eprintln!(
                            "cratebay-guest-agent[{}]: proxy ended: {}",
                            worker_id, error
                        );
                    } else {
                        eprintln!("cratebay-guest-agent[{}]: proxy completed", worker_id);
                    }
                }
            }
            Err(error) => {
                eprintln!(
                    "cratebay-guest-agent[{}]: connect {} failed: {}",
                    worker_id, addr, error
                );
            }
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(target_os = "linux")]
fn wait_for_docker_socket_ready(
    docker_socket: &std::path::Path,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if docker_socket.exists() && docker_ping_unix_socket(docker_socket).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Err(format!(
        "Docker socket was not ready within {}s: {}",
        timeout.as_secs(),
        docker_socket.display()
    ))
}

#[cfg(target_os = "linux")]
fn wait_for_docker_host_ready(
    docker_host: std::net::SocketAddr,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if docker_ping_tcp_host(docker_host).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Err(format!(
        "Docker TCP endpoint was not ready within {}s: {}",
        timeout.as_secs(),
        docker_host
    ))
}

#[cfg(target_os = "linux")]
fn docker_ping_unix_socket(docker_socket: &std::path::Path) -> Result<(), String> {
    use std::io::{Read, Write as _};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut stream = UnixStream::connect(docker_socket)
        .map_err(|e| format!("connect {}: {}", docker_socket.display(), e))?;

    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));

    stream
        .write_all(
            b"GET /_ping HTTP/1.1\r\nHost: docker\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        )
        .map_err(|e| format!("write _ping: {}", e))?;

    let mut out = Vec::with_capacity(512);
    let mut buf = [0u8; 256];
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return Err(format!("read _ping: {}", e)),
        };
        out.extend_from_slice(&buf[..n]);
        if out.windows(2).any(|window| window == b"OK") || out.len() >= 4096 {
            break;
        }
    }

    let resp = String::from_utf8_lossy(&out);
    if resp.contains("200 OK") && resp.contains("OK") {
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

#[cfg(target_os = "linux")]
fn docker_ping_tcp_host(docker_host: std::net::SocketAddr) -> Result<(), String> {
    use std::io::{Read, Write as _};
    use std::net::TcpStream;
    use std::time::Duration;

    let mut stream = TcpStream::connect_timeout(&docker_host, Duration::from_secs(2))
        .map_err(|e| format!("connect {}: {}", docker_host, e))?;

    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));

    stream
        .write_all(
            b"GET /_ping HTTP/1.1\r\nHost: docker\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        )
        .map_err(|e| format!("write _ping: {}", e))?;

    let mut out = Vec::with_capacity(512);
    let mut buf = [0u8; 256];
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return Err(format!("read _ping: {}", e)),
        };
        out.extend_from_slice(&buf[..n]);
        if out.windows(2).any(|window| window == b"OK") || out.len() >= 4096 {
            break;
        }
    }

    let resp = String::from_utf8_lossy(&out);
    if resp.contains("200 OK") && resp.contains("OK") {
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

#[cfg(target_os = "linux")]
fn proxy_unix_to_file(
    docker: std::os::unix::net::UnixStream,
    client: std::fs::File,
) -> Result<(), String> {
    use std::os::fd::AsRawFd;

    let mut docker_r = docker
        .try_clone()
        .map_err(|e| format!("docker clone: {}", e))?;
    let mut docker_w = docker;

    let mut client_r = client
        .try_clone()
        .map_err(|e| format!("client clone: {}", e))?;
    let mut client_w = client;

    let t1 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut docker_r, &mut client_w)
            .map_err(|e| format!("docker->client copy: {}", e))?;
        let _ = unsafe { libc::shutdown(client_w.as_raw_fd(), libc::SHUT_WR) };
        Ok(())
    });

    let t2 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut client_r, &mut docker_w)
            .map_err(|e| format!("client->docker copy: {}", e))?;
        // Do not propagate client half-close into dockerd.
        //
        // Docker's HTTP API can return `500 {"message":"context canceled"}` for
        // endpoints like `/version` if the client shuts down its write-half
        // (FIN) immediately after sending the request. Many proxies propagate
        // EOF by half-closing the upstream socket, but for dockerd this can
        // cancel the request context even when the full request has already
        // been received.
        //
        // We keep the dockerd socket write-half open and rely on the overall
        // connection lifecycle (client close or docker close) to tear down the
        // tunnel.
        Ok(())
    });

    t1.join()
        .map_err(|_| "docker->client proxy thread panicked".to_string())??;
    t2.join()
        .map_err(|_| "client->docker proxy thread panicked".to_string())??;
    Ok(())
}

#[cfg(target_os = "linux")]
fn proxy_unix_to_tcp(
    docker: std::os::unix::net::UnixStream,
    client: std::net::TcpStream,
) -> Result<(), String> {
    use std::net::Shutdown;
    use std::os::fd::AsRawFd;

    let mut docker_r = docker
        .try_clone()
        .map_err(|e| format!("docker clone: {}", e))?;
    let mut docker_w = docker;

    let mut client_r = client
        .try_clone()
        .map_err(|e| format!("client clone: {}", e))?;
    let client_shutdown = client
        .try_clone()
        .map_err(|e| format!("client shutdown clone: {}", e))?;
    let mut client_w = client;

    let t1 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut docker_r, &mut client_w)
            .map_err(|e| format!("docker->client copy: {}", e))?;
        let _ = unsafe { libc::shutdown(client_w.as_raw_fd(), libc::SHUT_WR) };
        Ok(())
    });

    let t2 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut client_r, &mut docker_w)
            .map_err(|e| format!("client->docker copy: {}", e))?;
        // Do not propagate client half-close into dockerd.
        Ok(())
    });

    t1.join()
        .map_err(|_| "docker->client proxy thread panicked".to_string())??;
    t2.join()
        .map_err(|_| "client->docker proxy thread panicked".to_string())??;

    let _ = client_shutdown.shutdown(Shutdown::Both);
    Ok(())
}

#[cfg(target_os = "linux")]
fn proxy_tcp_to_tcp(
    docker: std::net::TcpStream,
    client: std::net::TcpStream,
) -> Result<(), String> {
    use std::net::Shutdown;

    let mut docker_r = docker
        .try_clone()
        .map_err(|e| format!("docker clone: {}", e))?;
    let mut docker_w = docker;

    let mut client_r = client
        .try_clone()
        .map_err(|e| format!("client clone: {}", e))?;
    let mut client_w = client;

    let t1 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut docker_r, &mut client_w)
            .map_err(|e| format!("docker->client copy: {}", e))?;
        let _ = client_w.shutdown(Shutdown::Write);
        Ok(())
    });

    let t2 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut client_r, &mut docker_w)
            .map_err(|e| format!("client->docker copy: {}", e))?;
        // Do not propagate client half-close into dockerd.
        Ok(())
    });

    t1.join()
        .map_err(|_| "docker->client proxy thread panicked".to_string())??;
    t2.join()
        .map_err(|_| "client->docker proxy thread panicked".to_string())??;
    Ok(())
}
