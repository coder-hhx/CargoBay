//! Docker connection management.
//!
//! All Docker operations use `bollard` with `Arc<Docker>` for shared access.
//! Supports multi-platform socket detection.

use bollard::Docker;

use crate::error::AppError;
use crate::runtime;

/// Create a Docker client connection.
///
/// Attempts connections in priority order (per runtime-spec.md §10):
/// 1. `DOCKER_HOST` environment variable
/// 2. Platform-specific external Docker sockets (via `runtime::detect_external_docker`)
/// 3. Built-in runtime socket
/// 4. Bollard local defaults as fallback
pub async fn connect() -> Result<Docker, AppError> {
    // 1+2. Use runtime module's detect_external_docker() which handles
    //       DOCKER_HOST and platform-specific socket paths in priority order
    if let Some(socket_path) = runtime::detect_external_docker() {
        tracing::info!("External Docker detected at: {}", socket_path.display());
        let socket_str = socket_path.to_str().unwrap_or_default();

        // On Unix, connect via socket; on Windows, bollard handles named pipes
        #[cfg(unix)]
        {
            match Docker::connect_with_unix(socket_str, 120, bollard::API_DEFAULT_VERSION) {
                Ok(docker) => {
                    if docker.ping().await.is_ok() {
                        tracing::info!("Connected via external Docker: {}", socket_path.display());
                        return Ok(docker);
                    }
                    tracing::warn!("External Docker socket exists but not responsive: {}", socket_path.display());
                }
                Err(e) => {
                    tracing::debug!("Failed to connect to external Docker socket: {}", e);
                }
            }
        }

        #[cfg(windows)]
        {
            match Docker::connect_with_named_pipe(socket_str, 120, bollard::API_DEFAULT_VERSION) {
                Ok(docker) => {
                    if docker.ping().await.is_ok() {
                        tracing::info!("Connected via external Docker: {}", socket_path.display());
                        return Ok(docker);
                    }
                    tracing::warn!("External Docker pipe exists but not responsive: {}", socket_path.display());
                }
                Err(e) => {
                    tracing::debug!("Failed to connect to external Docker pipe: {}", e);
                }
            }
        }
    }

    // 3. Try built-in runtime socket
    let runtime_mgr = runtime::create_runtime_manager();
    let runtime_socket = runtime_mgr.docker_socket_path();
    if runtime_socket.exists() {
        tracing::debug!("Trying built-in runtime socket: {}", runtime_socket.display());
        #[cfg(unix)]
        {
            match Docker::connect_with_unix(
                runtime_socket.to_str().unwrap_or_default(),
                120,
                bollard::API_DEFAULT_VERSION,
            ) {
                Ok(docker) => {
                    if docker.ping().await.is_ok() {
                        tracing::info!("Connected via built-in runtime: {}", runtime_socket.display());
                        return Ok(docker);
                    }
                }
                Err(e) => {
                    tracing::debug!("Built-in runtime socket failed: {}", e);
                }
            }
        }
    }

    // 4. Try local defaults as a final fallback
    tracing::debug!("Trying Docker local defaults");
    let docker = Docker::connect_with_local_defaults()?;
    docker.ping().await?;
    Ok(docker)
}

/// Attempt to connect, returning None if Docker is not available.
pub async fn try_connect() -> Option<Docker> {
    connect().await.ok()
}

/// Check if Docker daemon is accessible.
pub async fn is_available(docker: &Docker) -> bool {
    docker.ping().await.is_ok()
}

/// Get Docker version information.
pub async fn version(docker: &Docker) -> Result<bollard::system::Version, AppError> {
    docker.version().await.map_err(AppError::Docker)
}
