//! Container CRUD operations.
//!
//! Provides high-level container management on top of the bollard Docker SDK.

use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions, LogsOptions,
    RemoveContainerOptions, StatsOptions, StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::{ListImagesOptions, RemoveImageOptions, SearchImagesOptions, TagImageOptions};
use bollard::Docker;
use futures_util::StreamExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error as _;
use std::sync::Arc;
use std::time::Duration;

use crate::error::AppError;
use crate::models::{
    ContainerCreateRequest, ContainerDetail, ContainerInfo, ContainerListFilters, ContainerState,
    ContainerStats, ContainerStatus, ExecResult, ExecStreamChunk, ImageInspectInfo,
    ImageSearchResult, LocalImageInfo, LogEntry, LogOptions, PortMapping,
};

const DOCKER_LIST_TIMEOUT: Duration = Duration::from_secs(8);
const DOCKER_CREATE_TIMEOUT: Duration = Duration::from_secs(60);
const DOCKER_START_TIMEOUT: Duration = Duration::from_secs(60);
const DOCKER_STOP_TIMEOUT: Duration = Duration::from_secs(30);
const DOCKER_DELETE_TIMEOUT: Duration = Duration::from_secs(30);
const DOCKER_INSPECT_TIMEOUT: Duration = Duration::from_secs(8);
const DOCKER_STATS_TIMEOUT: Duration = Duration::from_secs(8);
const DOCKER_EXEC_SETUP_TIMEOUT: Duration = Duration::from_secs(12);
const DOCKER_LOGS_TIMEOUT: Duration = Duration::from_secs(12);
const DOCKER_IMAGE_LIST_TIMEOUT: Duration = Duration::from_secs(12);
const DOCKER_IMAGE_INSPECT_TIMEOUT: Duration = Duration::from_secs(12);
const DOCKER_IMAGE_REMOVE_TIMEOUT: Duration = Duration::from_secs(12);
const DOCKER_IMAGE_TAG_TIMEOUT: Duration = Duration::from_secs(12);

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

    let containers = tokio::time::timeout(DOCKER_LIST_TIMEOUT, docker.list_containers(options))
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker container list timed out after {:?}",
                DOCKER_LIST_TIMEOUT
            ))
        })??;

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
                        host_port: p.public_port?,
                        container_port: p.private_port,
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
    if let Some(ref template_id) = request.template_id {
        labels.insert("com.cratebay.template_id".to_string(), template_id.clone());
    }

    let host_config = bollard::models::HostConfig {
        memory: request.memory_mb.map(|m| (m * 1024 * 1024) as i64),
        nano_cpus: request.cpu_cores.map(|c| (c as i64) * 1_000_000_000),
        ..Default::default()
    };

    // Respect the image's default command unless the caller overrides it.
    //
    // `request.command` is shell-form (single string). When present, run it
    // through `/bin/sh -c` so users can pass `sleep infinity`, `bash`, etc.
    let cmd = request
        .command
        .as_ref()
        .map(|c| vec!["/bin/sh".to_string(), "-c".to_string(), c.to_string()]);

    let config = Config {
        image: Some(request.image.clone()),
        cmd,
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

    let response = tokio::time::timeout(
        DOCKER_CREATE_TIMEOUT,
        docker.create_container(Some(options), config),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker container create timed out after {:?}",
            DOCKER_CREATE_TIMEOUT
        ))
    })??;

    // Auto-start if requested (default: true)
    // If start fails (e.g. OCI shim error for images without /bin/sh),
    // the container is still created — return it with "created" status
    // rather than failing the entire operation.
    if request.auto_start.unwrap_or(true) {
        let start_result = tokio::time::timeout(
            DOCKER_START_TIMEOUT,
            docker.start_container::<String>(&response.id, None),
        )
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker container start timed out after {:?}",
                DOCKER_START_TIMEOUT
            ))
        });

        if let Err(e) = start_result.and_then(|r| r.map_err(AppError::Docker)) {
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
    match tokio::time::timeout(
        DOCKER_START_TIMEOUT,
        docker.start_container::<String>(id, None),
    )
    .await
    {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(AppError::Docker(e)),
        Err(_) => {
            // If the start request timed out, the daemon may still have started
            // the container. Best-effort inspect to avoid false negatives.
            if let Ok(detail) = inspect(docker, id).await {
                if detail.state.running {
                    return Ok(());
                }
            }
            Err(AppError::Runtime(format!(
                "Docker container start timed out after {:?}",
                DOCKER_START_TIMEOUT
            )))
        }
    }
}

/// Stop a running container.
pub async fn stop(docker: &Docker, id: &str, timeout: Option<u32>) -> Result<(), AppError> {
    let options = Some(StopContainerOptions {
        t: timeout.unwrap_or(10) as i64,
    });
    match tokio::time::timeout(DOCKER_STOP_TIMEOUT, docker.stop_container(id, options)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(AppError::Docker(e)),
        Err(_) => {
            // If stop timed out, inspect to see if it's already stopped.
            if let Ok(detail) = inspect(docker, id).await {
                if !detail.state.running {
                    return Ok(());
                }
            }
            Err(AppError::Runtime(format!(
                "Docker container stop timed out after {:?}",
                DOCKER_STOP_TIMEOUT
            )))
        }
    }
}

/// Remove a container. Must be stopped first unless force=true.
pub async fn delete(docker: &Docker, id: &str, force: bool) -> Result<(), AppError> {
    let options = Some(RemoveContainerOptions {
        force,
        ..Default::default()
    });
    tokio::time::timeout(DOCKER_DELETE_TIMEOUT, docker.remove_container(id, options))
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker container delete timed out after {:?}",
                DOCKER_DELETE_TIMEOUT
            ))
        })??;
    Ok(())
}

/// Inspect a container for detailed information.
pub async fn inspect(docker: &Docker, id: &str) -> Result<ContainerDetail, AppError> {
    let data = tokio::time::timeout(
        DOCKER_INSPECT_TIMEOUT,
        docker.inspect_container(id, Some(InspectContainerOptions { size: false })),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker container inspect timed out after {:?}",
            DOCKER_INSPECT_TIMEOUT
        ))
    })??;

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

/// Get real-time resource usage for a container.
pub async fn stats(docker: &Docker, id: &str) -> Result<ContainerStats, AppError> {
    let mut stream = docker.stats(
        id,
        Some(StatsOptions {
            stream: false,
            one_shot: true,
        }),
    );

    let stats = tokio::time::timeout(DOCKER_STATS_TIMEOUT, stream.next())
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker container stats timed out after {:?}",
                DOCKER_STATS_TIMEOUT
            ))
        })?
        .transpose()?
        .ok_or_else(|| AppError::Runtime(format!("No stats returned for container '{}'", id)))?;

    let cpu_total = stats.cpu_stats.cpu_usage.total_usage as f64;
    let cpu_prev_total = stats.precpu_stats.cpu_usage.total_usage as f64;
    let cpu_delta = cpu_total - cpu_prev_total;

    let system_total = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64;
    let system_prev_total = stats.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
    let system_delta = system_total - system_prev_total;

    let online_cpus = stats
        .cpu_stats
        .online_cpus
        .or_else(|| {
            stats
                .cpu_stats
                .cpu_usage
                .percpu_usage
                .as_ref()
                .map(|cpu| cpu.len() as u64)
        })
        .unwrap_or(1) as f64;

    let cpu_percent = if cpu_delta > 0.0 && system_delta > 0.0 {
        (cpu_delta / system_delta) * online_cpus * 100.0
    } else {
        0.0
    };

    let cpu_cores_used = cpu_percent / 100.0;

    let memory_used_bytes = stats.memory_stats.usage.unwrap_or(0) as f64;
    let memory_limit_bytes = stats.memory_stats.limit.unwrap_or(0) as f64;
    let memory_percent = if memory_limit_bytes > 0.0 {
        (memory_used_bytes / memory_limit_bytes) * 100.0
    } else {
        0.0
    };

    Ok(ContainerStats {
        id: stats.id,
        name: stats.name.trim_start_matches('/').to_string(),
        read_at: stats.read.to_string(),
        cpu_percent,
        cpu_cores_used,
        memory_used_mb: memory_used_bytes / 1024.0 / 1024.0,
        memory_limit_mb: memory_limit_bytes / 1024.0 / 1024.0,
        memory_percent,
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

    let exec_instance = tokio::time::timeout(
        DOCKER_EXEC_SETUP_TIMEOUT,
        docker.create_exec(id, exec_options),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker exec create timed out after {:?}",
            DOCKER_EXEC_SETUP_TIMEOUT
        ))
    })??;

    let start_result = tokio::time::timeout(
        DOCKER_EXEC_SETUP_TIMEOUT,
        docker.start_exec(&exec_instance.id, None),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker exec start timed out after {:?}",
            DOCKER_EXEC_SETUP_TIMEOUT
        ))
    })??;

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
    let inspect_result = tokio::time::timeout(
        DOCKER_EXEC_SETUP_TIMEOUT,
        docker.inspect_exec(&exec_instance.id),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker exec inspect timed out after {:?}",
            DOCKER_EXEC_SETUP_TIMEOUT
        ))
    })??;
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

    let exec_instance = tokio::time::timeout(
        DOCKER_EXEC_SETUP_TIMEOUT,
        docker.create_exec(id, exec_options),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker exec create timed out after {:?}",
            DOCKER_EXEC_SETUP_TIMEOUT
        ))
    })??;

    let start_result = tokio::time::timeout(
        DOCKER_EXEC_SETUP_TIMEOUT,
        docker.start_exec(&exec_instance.id, None),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker exec start timed out after {:?}",
            DOCKER_EXEC_SETUP_TIMEOUT
        ))
    })??;

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

    let inspect_result = tokio::time::timeout(
        DOCKER_EXEC_SETUP_TIMEOUT,
        docker.inspect_exec(&exec_instance.id),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker exec inspect timed out after {:?}",
            DOCKER_EXEC_SETUP_TIMEOUT
        ))
    })??;
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
    let since = parse_log_time_filter(opts.since.as_deref())?;
    let until = parse_log_time_filter(opts.until.as_deref())?;
    let with_timestamps = opts.timestamps.unwrap_or(false);
    let tail = opts
        .tail
        .map(|t| t.to_string())
        .unwrap_or_else(|| "100".to_string());

    let log_options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        since,
        until,
        tail,
        timestamps: with_timestamps,
        ..Default::default()
    };

    let mut stream = docker.logs(id, Some(log_options));
    tokio::time::timeout(DOCKER_LOGS_TIMEOUT, async move {
        let mut entries = Vec::new();

        while let Some(chunk) = stream.next().await {
            match chunk? {
                bollard::container::LogOutput::StdOut { message } => {
                    let (timestamp, parsed_message) =
                        split_log_timestamp(&String::from_utf8_lossy(&message), with_timestamps);
                    append_log_lines(&mut entries, "stdout", timestamp, parsed_message);
                }
                bollard::container::LogOutput::StdErr { message } => {
                    let (timestamp, parsed_message) =
                        split_log_timestamp(&String::from_utf8_lossy(&message), with_timestamps);
                    append_log_lines(&mut entries, "stderr", timestamp, parsed_message);
                }
                _ => {}
            }
        }

        Ok::<_, AppError>(entries)
    })
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker container logs timed out after {:?}",
            DOCKER_LOGS_TIMEOUT
        ))
    })?
}

fn append_log_lines(
    entries: &mut Vec<LogEntry>,
    stream: &str,
    timestamp: Option<String>,
    message: String,
) {
    if message.is_empty() {
        return;
    }

    // Docker log chunks can contain multiple lines; split to keep UI alignment sane.
    let mut pushed = false;
    for line in message.lines() {
        pushed = true;
        entries.push(LogEntry {
            stream: stream.to_string(),
            message: line.to_string(),
            timestamp: timestamp.clone(),
        });
    }

    // If the chunk was a single empty line (unlikely), preserve it.
    if !pushed {
        entries.push(LogEntry {
            stream: stream.to_string(),
            message,
            timestamp,
        });
    }
}

fn split_log_timestamp(raw: &str, with_timestamps: bool) -> (Option<String>, String) {
    if !with_timestamps {
        return (None, raw.to_string());
    }

    let trimmed = raw.trim_start();
    if let Some((prefix, remainder)) = trimmed.split_once(' ') {
        if chrono::DateTime::parse_from_rfc3339(prefix).is_ok() {
            return (Some(prefix.to_string()), remainder.to_string());
        }
    }

    (None, raw.to_string())
}

fn parse_log_time_filter(value: Option<&str>) -> Result<i64, AppError> {
    let Some(raw) = value else {
        return Ok(0);
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }

    if let Ok(unix_seconds) = trimmed.parse::<i64>() {
        return Ok(unix_seconds);
    }

    chrono::DateTime::parse_from_rfc3339(trimmed)
        .map(|dt| dt.timestamp())
        .map_err(|e| {
            AppError::Validation(format!(
                "Invalid timestamp '{}': expected UNIX seconds or RFC3339 ({})",
                trimmed, e
            ))
        })
}

// ---------------------------------------------------------------------------
// Docker Image operations
// ---------------------------------------------------------------------------

/// List local Docker images.
pub async fn image_list(docker: &Docker) -> Result<Vec<LocalImageInfo>, AppError> {
    let options = ListImagesOptions::<String> {
        all: false,
        ..Default::default()
    };

    let images = tokio::time::timeout(DOCKER_IMAGE_LIST_TIMEOUT, docker.list_images(Some(options)))
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker image list timed out after {:?}",
                DOCKER_IMAGE_LIST_TIMEOUT
            ))
        })??;

    let results = images
        .into_iter()
        .map(|img| {
            let sanitized_size = img.size.max(0);
            let size_bytes = sanitized_size as u64;
            let repo_tags = img
                .repo_tags
                .into_iter()
                .filter(|tag| tag != "<none>:<none>")
                .collect::<Vec<_>>();
            LocalImageInfo {
                id: img.id,
                repo_tags,
                size: sanitized_size,
                size_bytes,
                size_human: format_bytes_human(size_bytes),
                created: img.created,
            }
        })
        .collect();

    Ok(results)
}

/// Search images from registry via Docker API.
pub async fn image_search(
    docker: &Docker,
    query: &str,
    limit: Option<u64>,
) -> Result<Vec<ImageSearchResult>, AppError> {
    let term = query.trim();
    if term.is_empty() {
        return Err(AppError::Validation(
            "Image search query cannot be empty".to_string(),
        ));
    }

    let options = SearchImagesOptions {
        term: term.to_string(),
        limit,
        filters: HashMap::new(),
    };

    // Primary: Docker Engine search API (Docker Hub index via dockerd).
    // Fallback: Docker Hub HTTP API (host network), which is often more reliable
    // in environments where `index.docker.io` is blocked.
    let engine_results =
        match tokio::time::timeout(Duration::from_secs(12), docker.search_images(options)).await {
            Ok(Ok(results)) => Ok(results),
            Ok(Err(e)) => Err(AppError::Docker(e)),
            Err(_) => Err(AppError::Runtime(
                "Image search timed out after 12s".to_string(),
            )),
        };

    if let Ok(results) = engine_results {
        let mapped = results
            .into_iter()
            .filter_map(|item| {
                let reference = item.name?;
                Some(ImageSearchResult {
                    source: "dockerhub".to_string(),
                    reference,
                    description: item.description.unwrap_or_default(),
                    stars: item.star_count.and_then(|value| u64::try_from(value).ok()),
                    pulls: None,
                    official: item.is_official.unwrap_or(false),
                })
            })
            .collect();

        return Ok(mapped);
    }

    let engine_err = engine_results.err().unwrap_or_else(|| {
        AppError::Runtime("Docker Engine image search failed with unknown error".to_string())
    });
    tracing::warn!(
        "Docker Engine image search failed, trying Docker Hub API fallback: {}",
        engine_err
    );

    match dockerhub_search(term, limit).await {
        Ok(results) => Ok(results),
        Err(fallback_err) => Err(AppError::Runtime(format!(
            "Image search failed. Docker Engine: {}; Docker Hub API fallback: {}",
            engine_err, fallback_err
        ))),
    }
}

/// Search Docker Hub via HTTP API (does not require a Docker daemon).
pub async fn image_search_dockerhub(
    query: &str,
    limit: Option<u64>,
) -> Result<Vec<ImageSearchResult>, AppError> {
    let term = query.trim();
    if term.is_empty() {
        return Err(AppError::Validation(
            "Image search query cannot be empty".to_string(),
        ));
    }

    dockerhub_search(term, limit).await
}

#[derive(Debug, Deserialize)]
struct DockerHubSearchResponse {
    results: Vec<DockerHubRepo>,
}

#[derive(Debug, Deserialize)]
struct DockerHubRepo {
    name: Option<String>,
    namespace: Option<String>,
    description: Option<String>,
    star_count: Option<u64>,
    pull_count: Option<u64>,
    is_official: Option<bool>,
}

async fn dockerhub_search(
    query: &str,
    limit: Option<u64>,
) -> Result<Vec<ImageSearchResult>, AppError> {
    let page_size: u64 = limit.unwrap_or(25).clamp(1, 100);

    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(8));
    if let Ok(raw_proxy) = std::env::var("CRATEBAY_RUNTIME_HTTP_PROXY") {
        let proxy = raw_proxy.trim();
        if !proxy.is_empty() {
            let proxy_url = if proxy.contains("://") {
                proxy.to_string()
            } else {
                format!("http://{}", proxy)
            };
            builder = builder.proxy(reqwest::Proxy::all(&proxy_url).map_err(|e| {
                AppError::Runtime(format!(
                    "Invalid CRATEBAY_RUNTIME_HTTP_PROXY '{}': {}",
                    proxy, e
                ))
            })?);
        }
    }

    let client = builder
        .build()
        .map_err(|e| AppError::Runtime(format!("Failed to build HTTP client: {}", e)))?;

    let resp = client
        .get("https://hub.docker.com/v2/search/repositories/")
        .query(&[
            ("query", query),
            ("page", "1"),
            ("page_size", &page_size.to_string()),
        ])
        .send()
        .await
        .map_err(|e| {
            AppError::Runtime(format!(
                "Docker Hub request failed: {}",
                format_reqwest_error(&e)
            ))
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Runtime(format!(
            "Docker Hub search API returned {}: {}",
            status, body
        )));
    }

    let data: DockerHubSearchResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Runtime(format!("Failed to parse Docker Hub response: {}", e)))?;

    let mut mapped = Vec::new();
    for repo in data.results {
        let Some(name) = repo
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let namespace = repo
            .namespace
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let reference = match namespace {
            Some(ns) => format!("{}/{}", ns, name),
            None => name.to_string(),
        };

        let official = repo.is_official.unwrap_or(false) || namespace == Some("library");

        mapped.push(ImageSearchResult {
            source: "dockerhub".to_string(),
            reference,
            description: repo.description.unwrap_or_default(),
            stars: repo.star_count,
            pulls: repo.pull_count,
            official,
        });
    }

    Ok(mapped)
}

fn format_reqwest_error(err: &reqwest::Error) -> String {
    let mut message = err.to_string();
    let mut source = err.source();
    while let Some(next) = source {
        let next_msg = next.to_string();
        if !next_msg.is_empty() && !message.contains(&next_msg) {
            message.push_str(": ");
            message.push_str(&next_msg);
        }
        source = next.source();
    }
    message
}

/// Inspect a local image by id/tag.
pub async fn image_inspect(docker: &Docker, id: &str) -> Result<ImageInspectInfo, AppError> {
    let inspected = tokio::time::timeout(DOCKER_IMAGE_INSPECT_TIMEOUT, docker.inspect_image(id))
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker image inspect timed out after {:?}",
                DOCKER_IMAGE_INSPECT_TIMEOUT
            ))
        })??;
    let size_bytes = inspected.size.unwrap_or(0).max(0) as u64;
    let layers = inspected
        .root_fs
        .and_then(|root| root.layers)
        .map(|layers| layers.len() as u32)
        .unwrap_or(0);

    Ok(ImageInspectInfo {
        id: inspected.id.unwrap_or_else(|| id.to_string()),
        repo_tags: inspected.repo_tags.unwrap_or_default(),
        size_bytes,
        created: inspected
            .created
            .map(|value| value.to_string())
            .unwrap_or_default(),
        architecture: inspected
            .architecture
            .unwrap_or_else(|| "unknown".to_string()),
        os: inspected.os.unwrap_or_else(|| "unknown".to_string()),
        docker_version: inspected.docker_version.unwrap_or_default(),
        layers,
    })
}

/// Remove a local image.
pub async fn image_remove(docker: &Docker, id: &str, force: bool) -> Result<(), AppError> {
    let options = Some(RemoveImageOptions {
        force,
        noprune: false,
    });
    let _ = tokio::time::timeout(
        DOCKER_IMAGE_REMOVE_TIMEOUT,
        docker.remove_image(id, options, None),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker image remove timed out after {:?}",
            DOCKER_IMAGE_REMOVE_TIMEOUT
        ))
    })??;
    Ok(())
}

/// Tag an existing local image with a new repository:tag reference.
pub async fn image_tag(docker: &Docker, source: &str, target: &str) -> Result<(), AppError> {
    let source = source.trim();
    let target = target.trim();

    if source.is_empty() || target.is_empty() {
        return Err(AppError::Validation(
            "Image source and target must not be empty".to_string(),
        ));
    }
    if target.contains('@') {
        return Err(AppError::Validation(
            "Digest targets are not supported for image_tag".to_string(),
        ));
    }

    let (repo, tag) = split_repo_and_tag(target);
    let options = Some(TagImageOptions { repo, tag });
    tokio::time::timeout(DOCKER_IMAGE_TAG_TIMEOUT, docker.tag_image(source, options))
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker image tag timed out after {:?}",
                DOCKER_IMAGE_TAG_TIMEOUT
            ))
        })??;
    Ok(())
}

/// Progress callback for image pull operations.
pub type PullProgressCallback = Arc<dyn Fn(PullProgress) + Send + Sync + 'static>;

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
    let mut last_status = String::new();

    // Per-layer byte tracking
    let mut layer_current: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let mut layer_total: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    // Once we see the first "Downloading" event, freeze total (no new layers after that)
    let mut frozen_total: Option<u64> = None;
    // Track the highest sum_current we've ever sent (never go backwards)
    let mut max_current_sent: u64 = 0;

    let chunk_timeout_secs = 60;

    loop {
        let chunk_timeout = tokio::time::timeout(
            std::time::Duration::from_secs(chunk_timeout_secs),
            stream.next(),
        )
        .await;

        match chunk_timeout {
            Ok(Some(Ok(info))) => {
                if let Some(ref cb) = on_progress {
                    let status = info.status.unwrap_or_default();
                    let progress_text = info.progress.unwrap_or_default();
                    let layer_id = info.id.clone().unwrap_or_default();

                    let (cur, tot) = if let Some(detail) = &info.progress_detail {
                        (
                            detail.current.unwrap_or(0) as u64,
                            detail.total.unwrap_or(0) as u64,
                        )
                    } else {
                        (0u64, 0u64)
                    };

                    if !layer_id.is_empty() {
                        // Always record layer total when we first see it
                        if tot > 0 {
                            layer_total.entry(layer_id.clone()).or_insert(tot);
                        }

                        // Update current bytes based on status
                        match status.as_str() {
                            "Downloading" => {
                                // Freeze total on first download event
                                if frozen_total.is_none() {
                                    frozen_total = Some(layer_total.values().sum());
                                }
                                if cur > 0 {
                                    layer_current.insert(layer_id.clone(), cur);
                                }
                            }
                            "Download complete" | "Pull complete" | "Already exists"
                            | "Verifying Checksum" | "Extracting" => {
                                // Lock current = total for completed layers
                                if let Some(&t) = layer_total.get(&layer_id) {
                                    layer_current.insert(layer_id.clone(), t);
                                }
                            }
                            _ => {}
                        }
                    }

                    // Compute sums
                    let sum_current: u64 = layer_current.values().sum();
                    let sum_total: u64 = layer_total.values().sum();

                    // Use frozen total if available; otherwise use live sum
                    let report_total = frozen_total.unwrap_or(sum_total);
                    // Ensure total never < current, and if new layers added after freeze, expand
                    let report_total = report_total.max(sum_total).max(sum_current);
                    // Current never goes backwards
                    if sum_current > max_current_sent {
                        max_current_sent = sum_current;
                    }

                    let status_changed = status != last_status;
                    let throttle_ok =
                        last_progress_time.elapsed() > std::time::Duration::from_millis(200);

                    if status_changed || throttle_ok {
                        last_status.clone_from(&status);
                        cb(PullProgress {
                            status,
                            progress_detail: if progress_text.is_empty() {
                                None
                            } else {
                                Some(progress_text)
                            },
                            current_bytes: max_current_sent,
                            total_bytes: report_total,
                        });
                        last_progress_time = std::time::Instant::now();
                    }
                }
            }
            Ok(Some(Err(e))) => {
                return Err(e.into());
            }
            Ok(None) => {
                break;
            }
            Err(_) => {
                return Err(AppError::Runtime(format!(
                    "Image pull stalled (no data for {}s) for '{}'",
                    chunk_timeout_secs, pull_image
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

        // Notify progress: trying mirror
        if let Some(ref cb) = on_progress {
            cb(PullProgress {
                status: format!("尝试镜像站 {}/{}: {}", i + 1, mirrors.len(), mirror),
                progress_detail: None,
                current_bytes: 0,
                total_bytes: 0,
            });
        }

        match image_pull(docker, image, Some(mirror), on_progress.clone()).await {
            Ok(()) => {
                tracing::info!("Successfully pulled '{}' via mirror '{}'", image, mirror);
                // Re-tag the mirror image to the original name and remove the mirror tag.
                let mirror_ref = rewrite_image_for_mirror(image, mirror);
                if mirror_ref != image {
                    if let Err(e) = image_tag(docker, &mirror_ref, image).await {
                        tracing::warn!("Failed to re-tag '{}' → '{}': {}", mirror_ref, image, e);
                    } else {
                        // Remove the mirror-specific tag (best-effort)
                        let _ = image_remove(docker, &mirror_ref, false).await;
                        tracing::info!(
                            "Re-tagged '{}' → '{}' and removed mirror tag",
                            mirror_ref,
                            image
                        );
                    }
                }
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("Mirror '{}' failed for '{}': {}", mirror, image, e);
                if let Some(ref cb) = on_progress {
                    cb(PullProgress {
                        status: format!("镜像站 {} 失败，尝试下一个...", mirror),
                        progress_detail: None,
                        current_bytes: 0,
                        total_bytes: 0,
                    });
                }
                continue;
            }
        }
    }

    // Fallback: direct pull without mirror (with progress callback)
    tracing::info!("All mirrors failed, attempting direct pull for '{}'", image);
    if let Some(ref cb) = on_progress {
        cb(PullProgress {
            status: "所有镜像站失败，尝试直连...".to_string(),
            progress_detail: None,
            current_bytes: 0,
            total_bytes: 0,
        });
    }
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

fn split_repo_and_tag(reference: &str) -> (String, String) {
    if let Some((repo, tag)) = reference.rsplit_once(':') {
        // Keep `registry:port/repo` valid; only treat as tag if suffix is after last `/`.
        if !tag.contains('/') {
            return (repo.to_string(), tag.to_string());
        }
    }
    (reference.to_string(), "latest".to_string())
}

fn format_bytes_human(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let value = bytes as f64;
    if value < KB {
        format!("{} B", bytes)
    } else if value < MB {
        format!("{:.1} KB", value / KB)
    } else if value < GB {
        format!("{:.1} MB", value / MB)
    } else {
        format!("{:.2} GB", value / GB)
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
