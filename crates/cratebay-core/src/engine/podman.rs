//! Podman (Docker-compatible) engine bring-up helpers.
//!
//! This module is used by [`crate::engine::ensure_docker`] when
//! `CRATEBAY_ENGINE_PROVIDER=podman` is set, or as a fallback escape hatch when
//! the built-in runtime cannot be brought up successfully.
//!
//! Podman support is a compatibility path, not a parallel product roadmap.
//! CrateBay's primary runtime path remains the built-in runtime.
//!
//! Goal: provide a reachable Docker API endpoint backed by Podman and return a
//! `bollard::Docker` client.

use std::process::Stdio;
use std::time::{Duration, Instant};

use bollard::Docker;

use crate::error::AppError;

const PODMAN_CMD_TIMEOUT: Duration = Duration::from_secs(30);
const PING_TIMEOUT_SECS: u64 = 5;
const DOCKER_CLIENT_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone)]
enum PodmanEndpoint {
    UnixSocket(String),
    NamedPipe(String),
    Http(String),
}

pub async fn ensure_podman_docker(start_timeout: Duration) -> Result<Docker, AppError> {
    // Fast path: if user already configured DOCKER_HOST, try that first.
    if let Ok(host) = std::env::var("DOCKER_HOST") {
        if let Some(endpoint) = parse_endpoint(&host) {
            if let Some(docker) = try_connect_endpoint(endpoint).await {
                tracing::info!("Connected to Podman via DOCKER_HOST");
                return Ok(docker);
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        ensure_podman_machine_running(start_timeout).await?;
        let endpoint = podman_machine_endpoint(start_timeout).await?;
        connect_or_err(endpoint).await
    }

    #[cfg(target_os = "linux")]
    {
        let endpoint = ensure_podman_service_linux(start_timeout).await?;
        connect_or_err(endpoint).await
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = start_timeout;
        Err(AppError::Runtime(
            "Podman engine provider is not supported on this platform".to_string(),
        ))
    }
}

async fn connect_or_err(endpoint: PodmanEndpoint) -> Result<Docker, AppError> {
    try_connect_endpoint(endpoint.clone()).await.ok_or_else(|| {
        AppError::Runtime(format!(
            "Podman is not reachable at {}",
            describe(&endpoint)
        ))
    })
}

fn describe(endpoint: &PodmanEndpoint) -> String {
    match endpoint {
        PodmanEndpoint::UnixSocket(p) => format!("unix://{}", p),
        PodmanEndpoint::NamedPipe(p) => p.to_string(),
        PodmanEndpoint::Http(u) => u.to_string(),
    }
}

fn parse_endpoint(raw: &str) -> Option<PodmanEndpoint> {
    let host = raw.trim();
    if host.is_empty() {
        return None;
    }

    if let Some(path) = host.strip_prefix("unix://") {
        return Some(PodmanEndpoint::UnixSocket(path.to_string()));
    }

    if let Some(path) = host.strip_prefix("tcp://") {
        return Some(PodmanEndpoint::Http(format!("http://{}", path)));
    }

    if host.starts_with("http://") || host.starts_with("https://") {
        return Some(PodmanEndpoint::Http(host.to_string()));
    }

    // Windows named pipe formats:
    // - npipe:////./pipe/docker_engine
    // - \\.\pipe\docker_engine
    if let Some(rest) = host.strip_prefix("npipe:") {
        let rest = rest.trim_start_matches('/');
        let rest = rest
            .strip_prefix("./pipe/")
            .or_else(|| rest.strip_prefix(".\\pipe\\"))
            .unwrap_or(rest);
        if !rest.is_empty() {
            return Some(PodmanEndpoint::NamedPipe(format!(r"\\.\pipe\{}", rest)));
        }
    }

    if host.starts_with(r"\\.\pipe\") {
        return Some(PodmanEndpoint::NamedPipe(host.to_string()));
    }

    // Support bare Unix socket paths.
    if host.starts_with('/') {
        return Some(PodmanEndpoint::UnixSocket(host.to_string()));
    }

    None
}

async fn try_connect_endpoint(endpoint: PodmanEndpoint) -> Option<Docker> {
    match endpoint {
        PodmanEndpoint::UnixSocket(path) => {
            #[cfg(unix)]
            {
                let docker = Docker::connect_with_unix(
                    &path,
                    PING_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()?;
                if !crate::docker::is_available(&docker).await {
                    return None;
                }
                Docker::connect_with_unix(
                    &path,
                    DOCKER_CLIENT_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()
            }
            #[cfg(not(unix))]
            {
                let _ = path;
                None
            }
        }
        PodmanEndpoint::NamedPipe(pipe) => {
            #[cfg(windows)]
            {
                let docker = Docker::connect_with_named_pipe(
                    &pipe,
                    PING_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()?;
                if !crate::docker::is_available(&docker).await {
                    return None;
                }
                Docker::connect_with_named_pipe(
                    &pipe,
                    DOCKER_CLIENT_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()
            }
            #[cfg(not(windows))]
            {
                let _ = pipe;
                None
            }
        }
        PodmanEndpoint::Http(url) => {
            let docker =
                Docker::connect_with_http(&url, PING_TIMEOUT_SECS, bollard::API_DEFAULT_VERSION)
                    .ok()?;
            if !crate::docker::is_available(&docker).await {
                return None;
            }
            Docker::connect_with_http(
                &url,
                DOCKER_CLIENT_TIMEOUT_SECS,
                bollard::API_DEFAULT_VERSION,
            )
            .ok()
        }
    }
}

// ---------------------------------------------------------------------------
// macOS / Windows: podman machine
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "macos", target_os = "windows"))]
async fn ensure_podman_machine_running(start_timeout: Duration) -> Result<(), AppError> {
    // `podman machine start` is idempotent; if it fails due to missing machine,
    // attempt init + start.
    match run_podman(&["machine", "start"], start_timeout).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string().to_ascii_lowercase();
            let missing = msg.contains("no such machine")
                || msg.contains("no machine")
                || msg.contains("does not exist")
                || msg.contains("not found");
            if !missing {
                return Err(e);
            }

            tracing::info!("Podman machine not found; initializing...");
            run_podman(&["machine", "init"], start_timeout).await?;
            run_podman(&["machine", "start"], start_timeout).await?;
            Ok(())
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
async fn podman_machine_endpoint(start_timeout: Duration) -> Result<PodmanEndpoint, AppError> {
    let deadline = Instant::now() + start_timeout;
    let mut last_error: Option<String> = None;

    while Instant::now() < deadline {
        match podman_machine_socket_descriptor().await {
            Ok(raw) => {
                if let Some(ep) = parse_endpoint(&raw) {
                    return Ok(ep);
                }
                // Some versions return a bare path; treat it as unix socket.
                if raw.trim().starts_with('/') {
                    return Ok(PodmanEndpoint::UnixSocket(raw.trim().to_string()));
                }
                last_error = Some(format!("Unsupported socket descriptor: {}", raw));
            }
            Err(e) => {
                last_error = Some(e.to_string());
            }
        }

        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    Err(AppError::Runtime(format!(
        "Timed out waiting for Podman machine socket (timeout {:?}): {}",
        start_timeout,
        last_error.unwrap_or_else(|| "unknown".to_string())
    )))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
async fn podman_machine_socket_descriptor() -> Result<String, AppError> {
    // Prefer `.Path` when present.
    let path = run_podman(
        &[
            "machine",
            "inspect",
            "--format",
            "{{.ConnectionInfo.PodmanSocket.Path}}",
        ],
        PODMAN_CMD_TIMEOUT,
    )
    .await?;
    if !path.trim().is_empty() {
        return Ok(path.trim().to_string());
    }

    // Fallback: `.URL` (often `unix:///...` or `npipe:...`).
    let url = run_podman(
        &[
            "machine",
            "inspect",
            "--format",
            "{{.ConnectionInfo.PodmanSocket.URL}}",
        ],
        PODMAN_CMD_TIMEOUT,
    )
    .await?;
    if !url.trim().is_empty() {
        return Ok(url.trim().to_string());
    }

    Err(AppError::Runtime(
        "Podman machine inspect did not return a socket path/URL".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Linux: podman system service
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
async fn ensure_podman_service_linux(start_timeout: Duration) -> Result<PodmanEndpoint, AppError> {
    let socket_path = linux_podman_socket_path();
    if let Some(docker) = try_connect_endpoint(PodmanEndpoint::UnixSocket(
        socket_path.to_string_lossy().to_string(),
    ))
    .await
    {
        tracing::info!("Connected to Podman via existing socket");
        return Ok(PodmanEndpoint::UnixSocket(
            socket_path.to_string_lossy().to_string(),
        ));
    }

    // Start a long-running Podman service exposing a Docker-compatible API.
    // We intentionally detach the child process so it can outlive a short-lived CLI.
    let uri = format!("unix://{}", socket_path.display());
    let _ = spawn_podman_detached(&["system", "service", "--time=0", &uri])?;

    // Wait for the socket to appear and become responsive.
    let deadline = Instant::now() + start_timeout;
    let mut last_error: Option<String> = None;
    while Instant::now() < deadline {
        if socket_path.exists() {
            if try_connect_endpoint(PodmanEndpoint::UnixSocket(
                socket_path.to_string_lossy().to_string(),
            ))
            .await
            .is_some()
            {
                return Ok(PodmanEndpoint::UnixSocket(
                    socket_path.to_string_lossy().to_string(),
                ));
            }
            last_error = Some("socket exists but ping failed".to_string());
        } else {
            last_error = Some("socket not created yet".to_string());
        }

        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    Err(AppError::Runtime(format!(
        "Timed out waiting for Podman service socket (timeout {:?}): {}",
        start_timeout,
        last_error.unwrap_or_else(|| "unknown".to_string())
    )))
}

#[cfg(target_os = "linux")]
fn linux_podman_socket_path() -> PathBuf {
    use std::path::{Path, PathBuf};

    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            return Path::new(dir).join("podman").join("podman.sock");
        }
    }

    // Fallback: /run/user/<uid>/podman/podman.sock (rootless default)
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/run/user/{}/podman/podman.sock", uid))
}

// ---------------------------------------------------------------------------
// Command helpers
// ---------------------------------------------------------------------------

async fn run_podman(args: &[&str], timeout: Duration) -> Result<String, AppError> {
    let mut cmd = tokio::process::Command::new("podman");
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false);

    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Timed out running podman {} (timeout {:?})",
                args.join(" "),
                timeout
            ))
        })?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::Runtime(
                    "Podman executable not found. Please install Podman and ensure `podman` is in PATH."
                        .to_string(),
                )
            } else {
                AppError::Io(e)
            }
        })?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let combined = [stdout, stderr]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    Err(AppError::Runtime(format!(
        "podman {} failed: {}",
        args.join(" "),
        if combined.is_empty() {
            format!("exit status {}", output.status)
        } else {
            combined
        }
    )))
}

#[cfg(target_os = "linux")]
fn spawn_podman_detached(args: &[&str]) -> Result<(), AppError> {
    let mut cmd = std::process::Command::new("podman");
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // Detach from the current session so it survives GUI/CLI exit.
    //
    // This is intentionally Linux-only; on macOS/Windows we use `podman machine`
    // which manages its own long-running components.
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(|| {
            let _ = libc::setsid();
            Ok(())
        });
    }

    cmd.spawn().map(|_| ()).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::Runtime(
                "Podman executable not found. Please install Podman and ensure `podman` is in PATH."
                    .to_string(),
            )
        } else {
            AppError::Io(e)
        }
    })
}
