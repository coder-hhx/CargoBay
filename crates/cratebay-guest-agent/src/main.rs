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
    let cfg = Config::from_env_and_args()?;

    match cfg.listen {
        ListenMode::Vsock { port } => run_vsock(port, cfg.docker_socket),
        ListenMode::Tcp { addr } => run_tcp(addr, cfg.docker_socket),
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct Config {
    listen: ListenMode,
    docker_socket: std::path::PathBuf,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
enum ListenMode {
    Vsock { port: u32 },
    Tcp { addr: std::net::SocketAddr },
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

        let mut tcp_mode = false;
        let mut tcp_listen: Option<std::net::SocketAddr> =
            std::env::var("CRATEBAY_GUEST_TCP_LISTEN")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| v.parse::<std::net::SocketAddr>().ok());

        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--tcp" => {
                    tcp_mode = true;
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
                "--help" | "-h" => {
                    return Err(Self::usage().to_string());
                }
                other => return Err(format!("Unknown argument: {}", other)),
            }
        }

        let listen = if tcp_mode {
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
        })
    }

    fn usage() -> &'static str {
        "Usage:\n  cratebay-guest-agent [--tcp] [--port <port>] [--listen <ip:port>] [--docker-sock <path>]\n\n\
Modes:\n  (default) vsock: listen on AF_VSOCK port\n  --tcp          : listen on TCP (default 0.0.0.0:<port>)\n\n\
Env:\n  CRATEBAY_DOCKER_PROXY_PORT  Guest proxy listen port (default 6237)\n  \
CRATEBAY_DOCKER_VSOCK_PORT   Back-compat for proxy port (default 6237)\n  \
CRATEBAY_GUEST_TCP_LISTEN    TCP listen addr override (e.g. 0.0.0.0:6237)\n  \
CRATEBAY_GUEST_DOCKER_SOCK   Guest Docker unix socket path (default /var/run/docker.sock)\n"
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

    let listener_fd = vsock_listen(port)?;
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

    let listener = TcpListener::bind(addr).map_err(|e| format!("bind tcp {}: {}", addr, e))?;
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

    let t1 = std::thread::spawn(move || {
        let _ = std::io::copy(&mut docker_r, &mut client_w);
        let _ = unsafe { libc::shutdown(client_w.as_raw_fd(), libc::SHUT_WR) };
    });

    let t2 = std::thread::spawn(move || {
        let _ = std::io::copy(&mut client_r, &mut docker_w);
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
    });

    let _ = t1.join();
    let _ = t2.join();
    Ok(())
}

#[cfg(target_os = "linux")]
fn proxy_unix_to_tcp(
    docker: std::os::unix::net::UnixStream,
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

    let t1 = std::thread::spawn(move || {
        let _ = std::io::copy(&mut docker_r, &mut client_w);
        let _ = client_w.shutdown(Shutdown::Write);
    });

    let t2 = std::thread::spawn(move || {
        let _ = std::io::copy(&mut client_r, &mut docker_w);
        // See note in `proxy_unix_to_file`: avoid half-closing dockerd's socket
        // to prevent request-context cancellation in dockerd.
    });

    let _ = t1.join();
    let _ = t2.join();
    Ok(())
}
