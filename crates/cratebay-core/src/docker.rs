//! Docker connection management.
//!
//! All Docker operations use `bollard` with `Arc<Docker>` for shared access.
//! Supports multi-platform socket detection.

use bollard::Docker;
use std::time::Duration;

use crate::error::AppError;
use crate::models::DockerSource;
use crate::runtime;

#[derive(Debug, Clone)]
enum DockerHostTarget {
    UnixSocket(String),
    NamedPipe(String),
    Http(String),
}

const DOCKER_PING_TIMEOUT_SECS: u64 = 5;
const DOCKER_DEFAULT_TIMEOUT_SECS: u64 = 120;

fn parse_docker_host_target(raw: &str) -> Option<DockerHostTarget> {
    let host = raw.trim();
    if host.is_empty() {
        return None;
    }

    if let Some(path) = host.strip_prefix("unix://") {
        return Some(DockerHostTarget::UnixSocket(path.to_string()));
    }

    if let Some(path) = host.strip_prefix("tcp://") {
        return Some(DockerHostTarget::Http(format!("http://{}", path)));
    }

    if host.starts_with("http://") || host.starts_with("https://") {
        return Some(DockerHostTarget::Http(host.to_string()));
    }

    // Named pipe formats commonly used by Docker on Windows:
    // - npipe:////./pipe/docker_engine
    // - \\.\pipe\docker_engine
    if let Some(rest) = host.strip_prefix("npipe:") {
        let rest = rest.trim_start_matches('/');
        let rest = rest
            .strip_prefix("./pipe/")
            .or_else(|| rest.strip_prefix(".\\pipe\\"))
            .unwrap_or(rest);
        if !rest.is_empty() {
            return Some(DockerHostTarget::NamedPipe(format!(r"\\.\pipe\{}", rest)));
        }
    }

    if host.starts_with(r"\\.\pipe\") {
        return Some(DockerHostTarget::NamedPipe(host.to_string()));
    }

    // Support bare Unix socket paths when users pass `--docker-host /path/to/docker.sock`.
    if host.starts_with('/') {
        return Some(DockerHostTarget::UnixSocket(host.to_string()));
    }

    None
}

async fn try_connect_target(target: DockerHostTarget) -> Option<Docker> {
    match target {
        DockerHostTarget::UnixSocket(path) => {
            #[cfg(unix)]
            {
                let docker = Docker::connect_with_unix(
                    &path,
                    DOCKER_PING_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()?;
                if !crate::docker::is_available(&docker).await {
                    return None;
                }
                Docker::connect_with_unix(
                    &path,
                    DOCKER_DEFAULT_TIMEOUT_SECS,
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
        DockerHostTarget::NamedPipe(pipe) => {
            #[cfg(windows)]
            {
                let docker = Docker::connect_with_named_pipe(
                    &pipe,
                    DOCKER_PING_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()?;
                if !crate::docker::is_available(&docker).await {
                    return None;
                }
                Docker::connect_with_named_pipe(
                    &pipe,
                    DOCKER_DEFAULT_TIMEOUT_SECS,
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
        DockerHostTarget::Http(url) => {
            let docker = Docker::connect_with_http(
                &url,
                DOCKER_PING_TIMEOUT_SECS,
                bollard::API_DEFAULT_VERSION,
            )
            .ok()?;
            if !crate::docker::is_available(&docker).await {
                return None;
            }
            Docker::connect_with_http(
                &url,
                DOCKER_DEFAULT_TIMEOUT_SECS,
                bollard::API_DEFAULT_VERSION,
            )
            .ok()
        }
    }
}

/// Create a Docker client connection.
///
/// Attempts connections in priority order (per runtime-spec.md §10):
/// 1. `DOCKER_HOST` environment variable
/// 2. Platform-specific external Docker sockets (via `runtime::detect_external_docker`)
/// 3. Built-in runtime socket
/// 4. Bollard local defaults as fallback
pub async fn connect() -> Result<Docker, AppError> {
    // 1. DOCKER_HOST environment variable (supports unix/tcp/http/npipe)
    if let Ok(host) = std::env::var("DOCKER_HOST") {
        if let Some(target) = parse_docker_host_target(&host) {
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!("Connected via DOCKER_HOST");
                return Ok(docker);
            }
            tracing::warn!("DOCKER_HOST is set but Docker is not reachable");
        } else if !host.trim().is_empty() {
            tracing::warn!("DOCKER_HOST is set but has an unsupported format");
        }
    }

    // 2. Platform-specific external Docker sockets (via `runtime::detect_external_docker`)
    if let Some(socket_path) = runtime::detect_external_docker() {
        tracing::info!("External Docker detected at: {}", socket_path.display());
        let socket_str = socket_path.to_str().unwrap_or_default();

        // Bound ping time so a stale socket/pipe can't hang the app.
        #[cfg(unix)]
        {
            if let Some(docker) =
                try_connect_target(DockerHostTarget::UnixSocket(socket_str.to_string())).await
            {
                tracing::info!("Connected via external Docker: {}", socket_path.display());
                return Ok(docker);
            }
            tracing::warn!(
                "External Docker socket exists but not responsive: {}",
                socket_path.display()
            );
        }

        #[cfg(windows)]
        {
            if let Some(docker) =
                try_connect_target(DockerHostTarget::NamedPipe(socket_str.to_string())).await
            {
                tracing::info!("Connected via external Docker: {}", socket_path.display());
                return Ok(docker);
            }
            tracing::warn!(
                "External Docker pipe exists but not responsive: {}",
                socket_path.display()
            );
        }
    }

    // 3. Try built-in runtime socket
    let runtime_mgr = runtime::create_runtime_manager();
    let runtime_socket = runtime_mgr.docker_socket_path();
    if runtime_socket.exists() {
        tracing::debug!(
            "Trying built-in runtime socket: {}",
            runtime_socket.display()
        );
        #[cfg(unix)]
        {
            let socket_str = runtime_socket.to_str().unwrap_or_default();
            if let Some(docker) =
                try_connect_target(DockerHostTarget::UnixSocket(socket_str.to_string())).await
            {
                tracing::info!(
                    "Connected via built-in runtime: {}",
                    runtime_socket.display()
                );
                return Ok(docker);
            }
            tracing::debug!(
                "Built-in runtime socket not responsive: {}",
                runtime_socket.display()
            );
        }
    }

    // 3b. Try built-in runtime TCP endpoint (Linux/Windows)
    //
    // On Linux and Windows the built-in runtime exposes Docker via a TCP
    // endpoint (hostfwd / WSL localhost forwarding). `docker_socket_path()`
    // may not exist, so we attempt these endpoints opportunistically.
    #[cfg(target_os = "linux")]
    {
        let host = runtime::linux::linux_docker_host();
        if let Some(target) = parse_docker_host_target(&host) {
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!("Connected via built-in Linux runtime TCP endpoint");
                return Ok(docker);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let host = runtime::windows::windows_docker_host();
        if let Some(target) = parse_docker_host_target(&host) {
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!("Connected via built-in Windows runtime TCP endpoint");
                return Ok(docker);
            }
        }

        // Optional future: named pipe proxy for the built-in runtime.
        let pipe = r"\\.\pipe\cratebay-docker";
        if let Some(target) = parse_docker_host_target(pipe) {
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!("Connected via built-in Windows runtime named pipe");
                return Ok(docker);
            }
        }
    }

    // 4. Try local defaults as a final fallback
    tracing::debug!("Trying Docker local defaults");
    let docker = Docker::connect_with_local_defaults()?;
    if !crate::docker::is_available(&docker).await {
        return Err(AppError::Runtime(
            "Docker local defaults detected but daemon is not reachable".to_string(),
        ));
    }
    Ok(docker)
}

/// Attempt to connect, returning None if Docker is not available.
pub async fn try_connect() -> Option<Docker> {
    connect().await.ok()
}

/// Attempt to connect, returning both the Docker client and the source it
/// was connected through.  Returns `None` if Docker is not available.
pub async fn try_connect_with_source() -> Option<(Docker, DockerSource)> {
    connect_with_source().await.ok()
}

/// Check if Docker daemon is accessible.
pub async fn is_available(docker: &Docker) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(DOCKER_PING_TIMEOUT_SECS), docker.ping()).await,
        Ok(Ok(_))
    )
}

/// Infer the [`DockerSource`] from a Unix socket path.
fn source_from_socket_path(path: &str) -> DockerSource {
    if path.contains(".cratebay") {
        DockerSource::BuiltinRuntime
    } else if path.contains(".colima") {
        DockerSource::Colima
    } else {
        DockerSource::External
    }
}

/// Like [`connect`] but also returns where the connection was established.
pub async fn connect_with_source() -> Result<(Docker, DockerSource), AppError> {
    // 1. DOCKER_HOST
    if let Ok(host) = std::env::var("DOCKER_HOST") {
        if let Some(target) = parse_docker_host_target(&host) {
            let source = if host.contains(".cratebay") {
                DockerSource::BuiltinRuntime
            } else if host.contains(".colima") {
                DockerSource::Colima
            } else {
                DockerSource::External
            };
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!("Connected via DOCKER_HOST (source={:?})", source);
                return Ok((docker, source));
            }
        }
    }

    // 2. Platform-specific external Docker (detect_external_docker)
    if let Some(socket_path) = runtime::detect_external_docker() {
        let socket_str = socket_path.to_str().unwrap_or_default();
        let source = source_from_socket_path(socket_str);

        #[cfg(unix)]
        if let Some(docker) =
            try_connect_target(DockerHostTarget::UnixSocket(socket_str.to_string())).await
        {
            tracing::info!(
                "Connected via external Docker: {} (source={:?})",
                socket_path.display(),
                source
            );
            return Ok((docker, source));
        }

        #[cfg(windows)]
        if let Some(docker) =
            try_connect_target(DockerHostTarget::NamedPipe(socket_str.to_string())).await
        {
            tracing::info!(
                "Connected via external Docker: {} (source={:?})",
                socket_path.display(),
                source
            );
            return Ok((docker, source));
        }
    }

    // 3. Built-in runtime socket
    let runtime_mgr = runtime::create_runtime_manager();
    let runtime_socket = runtime_mgr.docker_socket_path();
    if runtime_socket.exists() {
        #[cfg(unix)]
        {
            let socket_str = runtime_socket.to_str().unwrap_or_default();
            if let Some(docker) =
                try_connect_target(DockerHostTarget::UnixSocket(socket_str.to_string())).await
            {
                tracing::info!("Connected via built-in runtime: {}", runtime_socket.display());
                return Ok((docker, DockerSource::BuiltinRuntime));
            }
        }
    }

    // 3b. Built-in runtime TCP endpoint (Linux/Windows)
    #[cfg(target_os = "linux")]
    {
        let host = runtime::linux::linux_docker_host();
        if let Some(target) = parse_docker_host_target(&host) {
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!("Connected via built-in Linux runtime TCP");
                return Ok((docker, DockerSource::BuiltinRuntime));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let host = runtime::windows::windows_docker_host();
        if let Some(target) = parse_docker_host_target(&host) {
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!("Connected via built-in Windows runtime TCP");
                return Ok((docker, DockerSource::BuiltinRuntime));
            }
        }
        let pipe = r"\\.\pipe\cratebay-docker";
        if let Some(target) = parse_docker_host_target(pipe) {
            if let Some(docker) = try_connect_target(target).await {
                return Ok((docker, DockerSource::BuiltinRuntime));
            }
        }
    }

    // 4. Bollard local defaults
    let docker = Docker::connect_with_local_defaults()?;
    if !crate::docker::is_available(&docker).await {
        return Err(AppError::Runtime(
            "Docker local defaults detected but daemon is not reachable".to_string(),
        ));
    }
    Ok((docker, DockerSource::External))
}

/// Get Docker version information.
pub async fn version(docker: &Docker) -> Result<bollard::system::Version, AppError> {
    docker.version().await.map_err(AppError::Docker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_docker_host_target_supports_unix_socket() {
        let target = parse_docker_host_target("unix:///var/run/docker.sock");
        assert!(matches!(target, Some(DockerHostTarget::UnixSocket(_))));
    }

    #[test]
    fn parse_docker_host_target_supports_bare_unix_path() {
        let target = parse_docker_host_target("/var/run/docker.sock");
        assert!(matches!(target, Some(DockerHostTarget::UnixSocket(_))));
    }

    #[test]
    fn parse_docker_host_target_supports_tcp() {
        let target = parse_docker_host_target("tcp://127.0.0.1:2375");
        match target {
            Some(DockerHostTarget::Http(url)) => assert_eq!(url, "http://127.0.0.1:2375"),
            other => panic!("unexpected target: {:?}", other),
        }
    }

    #[test]
    fn parse_docker_host_target_supports_http() {
        let target = parse_docker_host_target("http://localhost:2375");
        match target {
            Some(DockerHostTarget::Http(url)) => assert_eq!(url, "http://localhost:2375"),
            other => panic!("unexpected target: {:?}", other),
        }
    }

    #[test]
    fn parse_docker_host_target_supports_npipe() {
        let target = parse_docker_host_target("npipe:////./pipe/docker_engine");
        match target {
            Some(DockerHostTarget::NamedPipe(pipe)) => {
                assert!(pipe.contains("docker_engine"), "pipe: {}", pipe);
            }
            other => panic!("unexpected target: {:?}", other),
        }
    }
}
