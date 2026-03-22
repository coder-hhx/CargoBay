//! Container CRUD operations.
//!
//! Provides high-level container management on top of the bollard Docker SDK.

use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions, LogsOptions,
    RemoveContainerOptions, StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::ListImagesOptions;
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;

use crate::error::AppError;
use crate::models::{
    ContainerCreateRequest, ContainerDetail, ContainerInfo, ContainerListFilters, ContainerState,
    ContainerStatus, DockerImageInfo, ExecResult, ExecStreamChunk, LogEntry, LogOptions,
    PortMapping,
};

/// List all containers, optionally filtered.
pub async fn list(
    docker: &Docker,
    all: bool,
    filters: Option<ContainerListFilters>,
) -> Result<Vec<ContainerInfo>, AppError> {
    let mut list_filters = HashMap::new();
    if let Some(ref f) = filters {
        if let Some(ref statuses) = f.status {
            let status_strings: Vec<String> = statuses
                .iter()
                .map(|s| {
                    format!("{}", serde_json::to_value(s).unwrap_or_default())
                        .trim_matches('"')
                        .to_string()
                })
                .collect();
            list_filters.insert("status".to_string(), status_strings);
        }
        if let Some(ref name) = f.name {
            list_filters.insert("name".to_string(), vec![name.clone()]);
        }
        if let Some(ref label) = f.label {
            let label_filters: Vec<String> =
                label.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            list_filters.insert("label".to_string(), label_filters);
        }
    }

    let options = Some(ListContainersOptions {
        all,
        filters: list_filters,
        ..Default::default()
    });

    let containers = docker.list_containers(options).await?;

    let mut results: Vec<ContainerInfo> = containers
        .into_iter()
        .map(|c| {
            let id = c.id.unwrap_or_default();
            let short_id = id.chars().take(12).collect();
            let names = c.names.unwrap_or_default();
            let name = names
                .first()
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_default();

            let status = match c.state.as_deref() {
                Some("running") => ContainerStatus::Running,
                Some("exited") => ContainerStatus::Exited,
                Some("created") => ContainerStatus::Created,
                Some("restarting") => ContainerStatus::Restarting,
                Some("removing") => ContainerStatus::Removing,
                Some("paused") => ContainerStatus::Paused,
                Some("dead") => ContainerStatus::Dead,
                _ => ContainerStatus::Stopped,
            };

            let ports = c
                .ports
                .unwrap_or_default()
                .into_iter()
                .filter_map(|p| {
                    Some(PortMapping {
                        host_port: p.public_port? as u16,
                        container_port: p.private_port as u16,
                        protocol: p
                            .typ
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "tcp".to_string()),
                    })
                })
                .collect();

            let labels = c.labels.unwrap_or_default();

            // Extract resource limits from labels (if set by CrateBay)
            let cpu_cores = labels
                .get("com.cratebay.cpu_cores")
                .and_then(|v| v.parse().ok());
            let memory_mb = labels
                .get("com.cratebay.memory_mb")
                .and_then(|v| v.parse().ok());

            ContainerInfo {
                id,
                short_id,
                name,
                image: c.image.unwrap_or_default(),
                status,
                state: c.state.unwrap_or_default(),
                created_at: c
                    .created
                    .map(|t| {
                        chrono::DateTime::from_timestamp(t, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default(),
                ports,
                labels,
                cpu_cores,
                memory_mb,
            }
        })
        .collect();

    // Apply client-side image filter (Docker API doesn't support image substring match)
    if let Some(ref f) = filters {
        if let Some(ref image_filter) = f.image {
            results.retain(|c| c.image.contains(image_filter.as_str()));
        }
    }

    Ok(results)
}

/// Create a new container from a request.
///
/// Assumes the image is already available locally. The caller (Tauri command)
/// is responsible for pulling the image before calling this function.
pub async fn create(
    docker: &Docker,
    request: ContainerCreateRequest,
) -> Result<ContainerInfo, AppError> {
    let mut labels: HashMap<String, String> = request.labels.clone().unwrap_or_default();
    labels.insert("com.cratebay.managed".to_string(), "true".to_string());
    if let Some(cpu) = request.cpu_cores {
        labels.insert("com.cratebay.cpu_cores".to_string(), cpu.to_string());
    }
    if let Some(mem) = request.memory_mb {
        labels.insert("com.cratebay.memory_mb".to_string(), mem.to_string());
    }

    let host_config = bollard::models::HostConfig {
        memory: request.memory_mb.map(|m| (m * 1024 * 1024) as i64),
        nano_cpus: request.cpu_cores.map(|c| (c as i64) * 1_000_000_000),
        ..Default::default()
    };

    let config = Config {
        image: Some(request.image.clone()),
        cmd: Some(
            request
                .command
                .as_ref()
                .map(|c| vec!["/bin/sh".to_string(), "-c".to_string(), c.clone()])
                .unwrap_or_else(|| vec!["/bin/sh".to_string()]),
        ),
        env: request.env.clone(),
        host_config: Some(host_config),
        labels: Some(labels),
        working_dir: request.working_dir.clone(),
        tty: Some(true),
        open_stdin: Some(true),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: request.name.as_str(),
        platform: None,
    };

    let response = docker.create_container(Some(options), config).await?;

    // Auto-start if requested (default: true)
    // If start fails (e.g. OCI shim error for images without /bin/sh),
    // the container is still created — return it with "created" status
    // rather than failing the entire operation.
    if request.auto_start.unwrap_or(true) {
        if let Err(e) = docker.start_container::<String>(&response.id, None).await {
            tracing::warn!(
                "Container {} created but auto-start failed: {}. Container remains in 'created' state.",
                response.id,
                e
            );
        }
    }

    inspect(docker, &response.id)
        .await
        .map(|detail| detail.info)
}

/// Start a stopped container.
pub async fn start(docker: &Docker, id: &str) -> Result<(), AppError> {
    docker.start_container::<String>(id, None).await?;
    Ok(())
}

/// Stop a running container.
pub async fn stop(docker: &Docker, id: &str, timeout: Option<u32>) -> Result<(), AppError> {
    let options = Some(StopContainerOptions {
        t: timeout.unwrap_or(10) as i64,
    });
    docker.stop_container(id, options).await?;
    Ok(())
}

/// Remove a container. Must be stopped first unless force=true.
pub async fn delete(docker: &Docker, id: &str, force: bool) -> Result<(), AppError> {
    let options = Some(RemoveContainerOptions {
        force,
        ..Default::default()
    });
    docker.remove_container(id, options).await?;
    Ok(())
}

/// Inspect a container for detailed information.
pub async fn inspect(docker: &Docker, id: &str) -> Result<ContainerDetail, AppError> {
    let data = docker
        .inspect_container(id, Some(InspectContainerOptions { size: false }))
        .await?;

    let container_id = data.id.unwrap_or_default();
    let short_id = container_id.chars().take(12).collect();
    let config = data.config.as_ref();
    let host_config = data.host_config.as_ref();
    let state = data.state.as_ref();

    let name = data
        .name
        .as_deref()
        .map(|n| n.trim_start_matches('/').to_string())
        .unwrap_or_default();

    let image = config.and_then(|c| c.image.clone()).unwrap_or_default();

    let labels = config.and_then(|c| c.labels.clone()).unwrap_or_default();

    let cpu_cores = labels
        .get("com.cratebay.cpu_cores")
        .and_then(|v| v.parse().ok())
        .or_else(|| host_config.and_then(|h| h.nano_cpus.map(|n| (n / 1_000_000_000) as u32)));

    let memory_mb = labels
        .get("com.cratebay.memory_mb")
        .and_then(|v| v.parse().ok())
        .or_else(|| host_config.and_then(|h| h.memory.map(|m| (m / 1024 / 1024) as u64)));

    let running = state.and_then(|s| s.running).unwrap_or(false);
    let status_str = state
        .and_then(|s| s.status.as_ref())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let container_status = match status_str.as_str() {
        "running" => ContainerStatus::Running,
        "exited" => ContainerStatus::Exited,
        "created" => ContainerStatus::Created,
        "restarting" => ContainerStatus::Restarting,
        "removing" => ContainerStatus::Removing,
        "paused" => ContainerStatus::Paused,
        "dead" => ContainerStatus::Dead,
        _ => ContainerStatus::Stopped,
    };

    let created_at = data.created.unwrap_or_default();

    let info = ContainerInfo {
        id: container_id,
        short_id,
        name,
        image,
        status: container_status,
        state: status_str.clone(),
        created_at,
        ports: Vec::new(), // Ports are complex to extract from inspect; use list for port info
        labels,
        cpu_cores,
        memory_mb,
    };

    let container_state = ContainerState {
        status: status_str,
        running,
        started_at: state.and_then(|s| s.started_at.clone()),
        finished_at: state.and_then(|s| s.finished_at.clone()),
        exit_code: state.and_then(|s| s.exit_code),
        error: state
            .and_then(|s| s.error.clone())
            .filter(|e| !e.is_empty()),
        pid: state.and_then(|s| s.pid).map(|p| p as u64),
    };

    let network_settings = data
        .network_settings
        .map(|ns| serde_json::to_value(ns).unwrap_or_default())
        .unwrap_or_default();

    let mounts = data
        .mounts
        .unwrap_or_default()
        .into_iter()
        .map(|m| serde_json::to_value(m).unwrap_or_default())
        .collect();

    Ok(ContainerDetail {
        info,
        network_settings,
        mounts,
        state: container_state,
    })
}

/// Execute a command inside a running container and return the complete result.
pub async fn exec(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    working_dir: Option<String>,
) -> Result<ExecResult, AppError> {
    let exec_options = CreateExecOptions {
        cmd: Some(cmd),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        working_dir,
        ..Default::default()
    };

    let exec_instance = docker.create_exec(id, exec_options).await?;

    let start_result = docker.start_exec(&exec_instance.id, None).await?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    if let StartExecResults::Attached { mut output, .. } = start_result {
        while let Some(chunk) = output.next().await {
            match chunk? {
                bollard::container::LogOutput::StdOut { message } => {
                    stdout.push_str(&String::from_utf8_lossy(&message));
                }
                bollard::container::LogOutput::StdErr { message } => {
                    stderr.push_str(&String::from_utf8_lossy(&message));
                }
                _ => {}
            }
        }
    }

    // Get exit code
    let inspect_result = docker.inspect_exec(&exec_instance.id).await?;
    let exit_code = inspect_result.exit_code.unwrap_or(-1);

    Ok(ExecResult {
        exit_code,
        stdout,
        stderr,
    })
}

/// Execute a command with streaming output via callback.
pub async fn exec_stream(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    working_dir: Option<String>,
    on_output: impl Fn(ExecStreamChunk) + Send + 'static,
) -> Result<i64, AppError> {
    let exec_options = CreateExecOptions {
        cmd: Some(cmd),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        working_dir,
        ..Default::default()
    };

    let exec_instance = docker.create_exec(id, exec_options).await?;

    let start_result = docker.start_exec(&exec_instance.id, None).await?;

    if let StartExecResults::Attached { mut output, .. } = start_result {
        while let Some(chunk) = output.next().await {
            match chunk {
                Ok(bollard::container::LogOutput::StdOut { message }) => {
                    on_output(ExecStreamChunk::Stdout {
                        data: String::from_utf8_lossy(&message).to_string(),
                    });
                }
                Ok(bollard::container::LogOutput::StdErr { message }) => {
                    on_output(ExecStreamChunk::Stderr {
                        data: String::from_utf8_lossy(&message).to_string(),
                    });
                }
                Ok(_) => {}
                Err(e) => {
                    on_output(ExecStreamChunk::Error {
                        message: e.to_string(),
                    });
                    break;
                }
            }
        }
    }

    let inspect_result = docker.inspect_exec(&exec_instance.id).await?;
    let exit_code = inspect_result.exit_code.unwrap_or(-1);

    on_output(ExecStreamChunk::Done { exit_code });

    Ok(exit_code)
}

/// Get container logs.
pub async fn logs(
    docker: &Docker,
    id: &str,
    options: Option<LogOptions>,
) -> Result<Vec<LogEntry>, AppError> {
    let opts = options.unwrap_or_default();
    let tail = opts
        .tail
        .map(|t| t.to_string())
        .unwrap_or_else(|| "100".to_string());

    let log_options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        tail: tail,
        timestamps: opts.timestamps.unwrap_or(false),
        ..Default::default()
    };

    let mut stream = docker.logs(id, Some(log_options));
    let mut entries = Vec::new();

    while let Some(chunk) = stream.next().await {
        match chunk? {
            bollard::container::LogOutput::StdOut { message } => {
                entries.push(LogEntry {
                    stream: "stdout".to_string(),
                    message: String::from_utf8_lossy(&message).to_string(),
                    timestamp: None,
                });
            }
            bollard::container::LogOutput::StdErr { message } => {
                entries.push(LogEntry {
                    stream: "stderr".to_string(),
                    message: String::from_utf8_lossy(&message).to_string(),
                    timestamp: None,
                });
            }
            _ => {}
        }
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Docker Image operations
// ---------------------------------------------------------------------------

/// List local Docker images.
pub async fn image_list(docker: &Docker) -> Result<Vec<DockerImageInfo>, AppError> {
    let options = ListImagesOptions::<String> {
        all: false,
        ..Default::default()
    };

    let images = docker.list_images(Some(options)).await?;

    let results = images
        .into_iter()
        .map(|img| {
            let id = img.id.clone();
            let short_id = id
                .strip_prefix("sha256:")
                .unwrap_or(&id)
                .chars()
                .take(12)
                .collect::<String>();
            DockerImageInfo {
                id: short_id,
                repo_tags: img.repo_tags,
                size: img.size,
                created: img.created,
            }
        })
        .collect();

    Ok(results)
}

/// Progress callback for image pull operations.
pub type PullProgressCallback = Box<dyn Fn(PullProgress) + Send + 'static>;

/// Pull progress info.
#[derive(Debug, Clone)]
pub struct PullProgress {
    pub status: String,
    pub progress_detail: Option<String>,
    pub current_bytes: u64,
    pub total_bytes: u64,
}

/// Pull a Docker image by name (e.g. "node:20-alpine").
///
/// If `mirror` is provided, rewrites Docker Hub images to use the mirror registry.
/// Each pull attempt has a 30-second timeout to prevent infinite blocking.
///
/// The optional `on_progress` callback receives real-time layer download progress.
pub async fn image_pull(
    docker: &Docker,
    image: &str,
    mirror: Option<&str>,
    on_progress: Option<PullProgressCallback>,
) -> Result<(), AppError> {
    use bollard::image::CreateImageOptions;

    let pull_image = match mirror {
        Some(m) if !m.is_empty() => rewrite_image_for_mirror(image, m),
        _ => image.to_string(),
    };

    tracing::info!("Pulling image: {} (original: {})", pull_image, image);

    let options = Some(CreateImageOptions {
        from_image: pull_image.as_str(),
        ..Default::default()
    });

    let mut stream = docker.create_image(options, None, None);
    let mut last_progress_time = std::time::Instant::now();

    // 60-second overall timeout for the pull stream
    let pull_timeout = std::time::Duration::from_secs(60);
    let start = std::time::Instant::now();

    loop {
        // Check overall timeout
        if start.elapsed() > pull_timeout {
            return Err(AppError::Runtime(format!(
                "Image pull timed out after {}s for '{}'",
                pull_timeout.as_secs(),
                pull_image
            )));
        }

        // Wait for next stream item with per-chunk timeout (15s)
        let chunk_timeout =
            tokio::time::timeout(std::time::Duration::from_secs(15), stream.next()).await;

        match chunk_timeout {
            Ok(Some(Ok(info))) => {
                // Report progress
                if let Some(ref cb) = on_progress {
                    let status = info.status.unwrap_or_default();
                    let progress = info.progress.unwrap_or_default();
                    let (current, total) = match info.progress_detail {
                        Some(detail) => (
                            detail.current.unwrap_or(0) as u64,
                            detail.total.unwrap_or(0) as u64,
                        ),
                        None => (0, 0),
                    };

                    // Throttle progress callbacks to max once per 500ms
                    if last_progress_time.elapsed() > std::time::Duration::from_millis(500)
                        || total > 0
                    {
                        cb(PullProgress {
                            status,
                            progress_detail: if progress.is_empty() {
                                None
                            } else {
                                Some(progress)
                            },
                            current_bytes: current,
                            total_bytes: total,
                        });
                        last_progress_time = std::time::Instant::now();
                    }
                }
            }
            Ok(Some(Err(e))) => {
                return Err(e.into());
            }
            Ok(None) => {
                // Stream finished successfully
                break;
            }
            Err(_) => {
                // Per-chunk timeout: no data in 15 seconds
                return Err(AppError::Runtime(format!(
                    "Image pull stalled (no data for 15s) for '{}'",
                    pull_image
                )));
            }
        }
    }

    Ok(())
}

/// Pull an image, trying a list of mirrors in order, falling back to direct pull.
///
/// Returns Ok(()) on the first successful pull. If all mirrors fail, attempts
/// a direct pull as final fallback.
pub async fn image_pull_with_mirrors(
    docker: &Docker,
    image: &str,
    mirrors: &[String],
    on_progress: Option<PullProgressCallback>,
) -> Result<(), AppError> {
    // Try each mirror in order
    for (i, mirror) in mirrors.iter().enumerate() {
        tracing::info!(
            "Trying mirror {}/{}: '{}' for image '{}'",
            i + 1,
            mirrors.len(),
            mirror,
            image
        );

        // For mirrors, we don't pass the progress callback since it might be a transient attempt
        match image_pull(docker, image, Some(mirror), None).await {
            Ok(()) => {
                tracing::info!("Successfully pulled '{}' via mirror '{}'", image, mirror);
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("Mirror '{}' failed for '{}': {}", mirror, image, e);
                continue;
            }
        }
    }

    // Fallback: direct pull without mirror (with progress callback)
    tracing::info!("All mirrors failed, attempting direct pull for '{}'", image);
    image_pull(docker, image, None, on_progress).await
}

/// Rewrite a Docker Hub image reference to use a mirror registry.
///
/// Rules:
/// - "node:20-alpine" → "{mirror}/library/node:20-alpine" (official image)
/// - "library/node:20-alpine" → "{mirror}/library/node:20-alpine"
/// - "myuser/myapp:latest" → "{mirror}/myuser/myapp:latest"
/// - "gcr.io/project/image:tag" → unchanged (non-Docker-Hub, has explicit registry)
fn rewrite_image_for_mirror(image: &str, mirror: &str) -> String {
    let mirror = mirror.trim_end_matches('/');

    // Check if image has an explicit registry (contains '.' or ':' before the first '/')
    // Examples with registry: "gcr.io/foo/bar", "registry.example.com:5000/foo"
    // Examples without: "node:20", "library/node:20", "myuser/myapp:latest"
    if let Some(first_slash_pos) = image.find('/') {
        let before_slash = &image[..first_slash_pos];
        if before_slash.contains('.') || before_slash.contains(':') {
            // Has explicit registry — don't rewrite
            return image.to_string();
        }
        // Has a namespace (e.g., "myuser/myapp:latest") — rewrite with namespace
        format!("{}/{}", mirror, image)
    } else {
        // Simple image name like "node:20-alpine" — add "library/" prefix
        format!("{}/library/{}", mirror, image)
    }
}

/// Check if a Docker image exists locally.
pub async fn image_exists(docker: &Docker, image: &str) -> Result<bool, AppError> {
    match docker.inspect_image(image).await {
        Ok(_) => Ok(true),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Ensure image exists locally, pulling if necessary.
pub async fn ensure_image(docker: &Docker, image: &str) -> Result<(), AppError> {
    if !image_exists(docker, image).await? {
        tracing::info!("Image '{}' not found locally, pulling...", image);
        image_pull(docker, image, None, None).await?;
        tracing::info!("Image '{}' pulled successfully", image);
    }
    Ok(())
}
