//! Container CRUD operations.
//!
//! Provides high-level container management on top of the bollard Docker SDK.

use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions, LogsOptions,
    RemoveContainerOptions, StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;

use crate::error::AppError;
use crate::models::{
    ContainerCreateRequest, ContainerDetail, ContainerInfo, ContainerListFilters, ContainerState,
    ContainerStatus, ExecResult, ExecStreamChunk, LogEntry, LogOptions, PortMapping,
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
                .map(|s| format!("{}", serde_json::to_value(s).unwrap_or_default()).trim_matches('"').to_string())
                .collect();
            list_filters.insert("status".to_string(), status_strings);
        }
        if let Some(ref name) = f.name {
            list_filters.insert("name".to_string(), vec![name.clone()]);
        }
        if let Some(ref label) = f.label {
            let label_filters: Vec<String> = label
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
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
                        protocol: p.typ.map(|t| t.to_string()).unwrap_or_else(|| "tcp".to_string()),
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
                created_at: c.created.map(|t| {
                    chrono::DateTime::from_timestamp(t, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                }).unwrap_or_default(),
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
        cmd: request
            .command
            .as_ref()
            .map(|c| vec!["/bin/sh".to_string(), "-c".to_string(), c.clone()]),
        env: request.env.clone(),
        host_config: Some(host_config),
        labels: Some(labels),
        working_dir: request.working_dir.clone(),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: request.name.as_str(),
        platform: None,
    };

    let response = docker.create_container(Some(options), config).await?;

    // Auto-start if requested (default: true)
    if request.auto_start.unwrap_or(true) {
        docker
            .start_container::<String>(&response.id, None)
            .await?;
    }

    inspect(docker, &response.id).await.map(|detail| detail.info)
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

    let image = config
        .and_then(|c| c.image.clone())
        .unwrap_or_default();

    let labels = config
        .and_then(|c| c.labels.clone())
        .unwrap_or_default();

    let cpu_cores = labels
        .get("com.cratebay.cpu_cores")
        .and_then(|v| v.parse().ok())
        .or_else(|| {
            host_config.and_then(|h| h.nano_cpus.map(|n| (n / 1_000_000_000) as u32))
        });

    let memory_mb = labels
        .get("com.cratebay.memory_mb")
        .and_then(|v| v.parse().ok())
        .or_else(|| {
            host_config.and_then(|h| h.memory.map(|m| (m / 1024 / 1024) as u64))
        });

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
        error: state.and_then(|s| s.error.clone()).filter(|e| !e.is_empty()),
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
