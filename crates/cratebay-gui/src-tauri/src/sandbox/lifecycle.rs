use super::*;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SandboxCreateRequest {
    pub(crate) template_id: String,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) image: Option<String>,
    #[serde(default)]
    pub(crate) command: Option<String>,
    #[serde(default)]
    pub(crate) env: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) cpu_cores: Option<u32>,
    #[serde(default)]
    pub(crate) memory_mb: Option<u64>,
    #[serde(default)]
    pub(crate) ttl_hours: Option<u32>,
    #[serde(default)]
    pub(crate) owner: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SandboxCreateResultDto {
    pub(crate) id: String,
    pub(crate) short_id: String,
    pub(crate) name: String,
    pub(crate) image: String,
    pub(crate) login_cmd: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SandboxInfoDto {
    pub(crate) id: String,
    pub(crate) short_id: String,
    pub(crate) name: String,
    pub(crate) image: String,
    pub(crate) state: String,
    pub(crate) status: String,
    pub(crate) template_id: String,
    pub(crate) owner: String,
    pub(crate) created_at: String,
    pub(crate) expires_at: String,
    pub(crate) ttl_hours: u32,
    pub(crate) cpu_cores: u32,
    pub(crate) memory_mb: u64,
    pub(crate) is_expired: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SandboxInspectDto {
    pub(crate) id: String,
    pub(crate) short_id: String,
    pub(crate) name: String,
    pub(crate) image: String,
    pub(crate) template_id: String,
    pub(crate) owner: String,
    pub(crate) created_at: String,
    pub(crate) expires_at: String,
    pub(crate) ttl_hours: u32,
    pub(crate) cpu_cores: u32,
    pub(crate) memory_mb: u64,
    pub(crate) running: bool,
    pub(crate) command: String,
    pub(crate) env: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GpuProcessDto {
    pub(crate) gpu_index: u32,
    pub(crate) gpu_name: String,
    pub(crate) pid: u32,
    pub(crate) process_name: String,
    pub(crate) memory_used_bytes: Option<u64>,
    pub(crate) memory_used_human: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SandboxRuntimeUsageDto {
    pub(crate) running: bool,
    pub(crate) cpu_percent: f64,
    pub(crate) memory_usage_mb: f64,
    pub(crate) memory_limit_mb: f64,
    pub(crate) memory_percent: f64,
    pub(crate) network_rx_bytes: u64,
    pub(crate) network_tx_bytes: u64,
    pub(crate) gpu_attribution_supported: bool,
    pub(crate) gpu_message: String,
    pub(crate) gpu_processes: Vec<GpuProcessDto>,
    pub(crate) gpu_memory_used_bytes: u64,
    pub(crate) gpu_memory_used_human: String,
}

#[derive(Debug, Clone)]
struct SandboxMeta {
    template_id: String,
    owner: String,
    created_at: String,
    expires_at: String,
    ttl_hours: u32,
    cpu_cores: u32,
    memory_mb: u64,
}

fn sandbox_meta_from_labels(labels: &HashMap<String, String>) -> SandboxMeta {
    SandboxMeta {
        template_id: labels
            .get(SANDBOX_LABEL_TEMPLATE_ID)
            .cloned()
            .unwrap_or_else(|| "custom".to_string()),
        owner: labels
            .get(SANDBOX_LABEL_OWNER)
            .cloned()
            .unwrap_or_else(sandbox_default_owner),
        created_at: labels
            .get(SANDBOX_LABEL_CREATED_AT)
            .cloned()
            .unwrap_or_default(),
        expires_at: labels
            .get(SANDBOX_LABEL_EXPIRES_AT)
            .cloned()
            .unwrap_or_default(),
        ttl_hours: sandbox_parse_u32_label(labels, SANDBOX_LABEL_TTL_HOURS, 8),
        cpu_cores: sandbox_parse_u32_label(labels, SANDBOX_LABEL_CPU_CORES, 2),
        memory_mb: sandbox_parse_u64_label(labels, SANDBOX_LABEL_MEMORY_MB, 2048),
    }
}

pub(crate) async fn sandbox_require_managed(
    docker: &Docker,
    id: &str,
) -> Result<(HashMap<String, String>, String), String> {
    let inspect = docker
        .inspect_container(id, None::<InspectContainerOptions>)
        .await
        .map_err(|e| sandbox_docker_error("inspect sandbox", id, &e))?;

    let labels = inspect
        .config
        .as_ref()
        .and_then(|cfg| cfg.labels.clone())
        .unwrap_or_default();
    if !sandbox_is_managed(&labels) {
        return Err(sandbox_not_managed_error());
    }

    let name = inspect
        .name
        .unwrap_or_else(|| id.to_string())
        .trim_start_matches('/')
        .to_string();

    Ok((labels, name))
}

#[tauri::command]
pub(crate) fn sandbox_templates() -> Vec<SandboxTemplateDto> {
    sandbox_templates_catalog()
}

#[tauri::command]
pub(crate) async fn sandbox_list() -> Result<Vec<SandboxInfoDto>, String> {
    let docker = connect_docker().map_err(|e| sandbox_connect_error(&e))?;

    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };
    let containers = docker
        .list_containers(Some(opts))
        .await
        .map_err(|e| sandbox_docker_error("list", "sandboxes", &e))?;

    let mut sandboxes = containers
        .into_iter()
        .filter_map(|item| {
            let labels = item.labels.unwrap_or_default();
            if !sandbox_is_managed(&labels) {
                return None;
            }
            let id = item.id.unwrap_or_default();
            let short_id = sandbox_short_id(&id);
            let name = item
                .names
                .unwrap_or_default()
                .first()
                .unwrap_or(&String::new())
                .trim_start_matches('/')
                .to_string();
            let meta = sandbox_meta_from_labels(&labels);
            Some(SandboxInfoDto {
                id,
                short_id,
                name,
                image: item.image.unwrap_or_default(),
                state: item.state.unwrap_or_default(),
                status: item.status.unwrap_or_default(),
                template_id: meta.template_id,
                owner: meta.owner,
                created_at: meta.created_at.clone(),
                expires_at: meta.expires_at.clone(),
                ttl_hours: meta.ttl_hours,
                cpu_cores: meta.cpu_cores,
                memory_mb: meta.memory_mb,
                is_expired: sandbox_is_expired(&meta.expires_at),
            })
        })
        .collect::<Vec<_>>();

    sandboxes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(sandboxes)
}

#[tauri::command]
pub(crate) async fn sandbox_create(
    request: SandboxCreateRequest,
) -> Result<SandboxCreateResultDto, String> {
    let template_id = request.template_id.trim().to_string();
    let template = sandbox_find_template(&template_id).ok_or_else(|| {
        sandbox_template_error(format!("Unknown sandbox template '{}'", template_id))
    })?;

    let image = request
        .image
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| template.image.to_string());
    validation::validate_image_reference(&image).map_err(|e| {
        sandbox_validation_error(format!("Invalid sandbox image '{}': {}", image, e))
    })?;

    let command = request
        .command
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| template.default_command.to_string());

    let name = request
        .name
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| sandbox_generate_name(&template_id));
    validation::validate_container_name(&name)
        .map_err(|e| sandbox_validation_error(format!("Invalid sandbox name '{}': {}", name, e)))?;

    let owner = sandbox_normalize_owner(request.owner);
    let cpu_cores = request
        .cpu_cores
        .unwrap_or(template.cpu_default)
        .clamp(1, 16);
    let memory_mb = request
        .memory_mb
        .unwrap_or(template.memory_mb_default)
        .clamp(256, 65536);
    let ttl_hours = request
        .ttl_hours
        .unwrap_or(template.ttl_hours_default)
        .clamp(1, 168);

    let created_at = chrono::Utc::now();
    let expires_at = created_at + chrono::Duration::hours(ttl_hours as i64);

    let mut env = template
        .default_env
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>();
    let mut custom_env = sandbox_normalize_env(request.env)?;
    env.append(&mut custom_env);

    let mut labels = HashMap::new();
    labels.insert(SANDBOX_LABEL_MANAGED.to_string(), "true".to_string());
    labels.insert(SANDBOX_LABEL_TEMPLATE_ID.to_string(), template_id.clone());
    labels.insert(SANDBOX_LABEL_OWNER.to_string(), owner.clone());
    labels.insert(
        SANDBOX_LABEL_CREATED_AT.to_string(),
        created_at.to_rfc3339(),
    );
    labels.insert(
        SANDBOX_LABEL_EXPIRES_AT.to_string(),
        expires_at.to_rfc3339(),
    );
    labels.insert(SANDBOX_LABEL_TTL_HOURS.to_string(), ttl_hours.to_string());
    labels.insert(SANDBOX_LABEL_CPU_CORES.to_string(), cpu_cores.to_string());
    labels.insert(SANDBOX_LABEL_MEMORY_MB.to_string(), memory_mb.to_string());

    let host_config = HostConfig {
        nano_cpus: Some((cpu_cores as i64) * 1_000_000_000),
        memory: Some((memory_mb as i64).saturating_mul(1024).saturating_mul(1024)),
        ..Default::default()
    };

    let config = Config::<String> {
        image: Some(image.clone()),
        cmd: Some(vec![
            "/bin/sh".to_string(),
            "-lc".to_string(),
            command.clone(),
        ]),
        host_config: Some(host_config),
        labels: Some(labels),
        env: if env.is_empty() { None } else { Some(env) },
        tty: Some(true),
        open_stdin: Some(true),
        ..Default::default()
    };

    let docker = connect_docker().map_err(|e| sandbox_connect_error(&e))?;
    if docker.inspect_image(&image).await.is_err() {
        docker_pull_image(&docker, &image)
            .await
            .map_err(|e| sandbox_image_pull_error(&image, &e))?;
    }
    let created = docker
        .create_container(
            Some(CreateContainerOptions {
                name: name.clone(),
                platform: None,
            }),
            config,
        )
        .await
        .map_err(|e| sandbox_docker_error("create sandbox", &name, &e))?;

    docker
        .start_container(&created.id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| sandbox_docker_error("start sandbox", &name, &e))?;

    let short_id = sandbox_short_id(&created.id);
    sandbox_audit_log(
        "create",
        &short_id,
        &name,
        "ok",
        &format!(
            "template={} image={} ttl={}h cpu={} mem={}MB",
            template_id, image, ttl_hours, cpu_cores, memory_mb
        ),
    );

    let login_cmd = if let Some(host) = docker_host_for_cli() {
        format!("DOCKER_HOST={} docker exec -it {} /bin/sh", host, name)
    } else {
        format!("docker exec -it {} /bin/sh", name)
    };

    Ok(SandboxCreateResultDto {
        id: created.id,
        short_id,
        name,
        image,
        login_cmd,
    })
}

#[tauri::command]
pub(crate) async fn sandbox_start(id: String) -> Result<(), String> {
    let docker = connect_docker().map_err(|e| sandbox_connect_error(&e))?;
    let (_, name) = sandbox_require_managed(&docker, &id).await?;
    docker
        .start_container(&id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| sandbox_docker_error("start sandbox", &id, &e))?;
    sandbox_audit_log("start", &id, &name, "ok", "sandbox started");
    Ok(())
}

#[tauri::command]
pub(crate) async fn sandbox_stop(id: String) -> Result<(), String> {
    let docker = connect_docker().map_err(|e| sandbox_connect_error(&e))?;
    let (_, name) = sandbox_require_managed(&docker, &id).await?;
    docker
        .stop_container(&id, Some(StopContainerOptions { t: 10 }))
        .await
        .map_err(|e| sandbox_docker_error("stop sandbox", &id, &e))?;
    sandbox_audit_log("stop", &id, &name, "ok", "sandbox stopped");
    Ok(())
}

#[tauri::command]
pub(crate) async fn sandbox_delete(id: String) -> Result<(), String> {
    let docker = connect_docker().map_err(|e| sandbox_connect_error(&e))?;
    let (_, name) = sandbox_require_managed(&docker, &id).await?;
    let _ = docker
        .stop_container(&id, Some(StopContainerOptions { t: 5 }))
        .await;
    docker
        .remove_container(
            &id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
        .map_err(|e| sandbox_docker_error("delete sandbox", &id, &e))?;
    sandbox_audit_log("delete", &id, &name, "ok", "sandbox removed");
    Ok(())
}

#[tauri::command]
pub(crate) async fn sandbox_inspect(id: String) -> Result<SandboxInspectDto, String> {
    let docker = connect_docker().map_err(|e| sandbox_connect_error(&e))?;
    let inspect = docker
        .inspect_container(&id, None::<InspectContainerOptions>)
        .await
        .map_err(|e| sandbox_docker_error("inspect sandbox", &id, &e))?;

    let labels = inspect
        .config
        .as_ref()
        .and_then(|cfg| cfg.labels.clone())
        .unwrap_or_default();
    if !sandbox_is_managed(&labels) {
        return Err(sandbox_not_managed_error());
    }
    let meta = sandbox_meta_from_labels(&labels);

    let running = inspect
        .state
        .as_ref()
        .and_then(|s| s.running)
        .unwrap_or(false);
    let image = inspect
        .config
        .as_ref()
        .and_then(|cfg| cfg.image.clone())
        .unwrap_or_default();
    let command = inspect
        .config
        .as_ref()
        .and_then(|cfg| cfg.cmd.clone())
        .unwrap_or_default()
        .join(" ");
    let env = inspect
        .config
        .as_ref()
        .and_then(|cfg| cfg.env.clone())
        .unwrap_or_default();
    let name = inspect
        .name
        .unwrap_or_else(|| id.clone())
        .trim_start_matches('/')
        .to_string();

    Ok(SandboxInspectDto {
        id: id.clone(),
        short_id: sandbox_short_id(&id),
        name,
        image,
        template_id: meta.template_id,
        owner: meta.owner,
        created_at: meta.created_at,
        expires_at: meta.expires_at,
        ttl_hours: meta.ttl_hours,
        cpu_cores: meta.cpu_cores,
        memory_mb: meta.memory_mb,
        running,
        command,
        env,
    })
}

#[tauri::command]
pub(crate) async fn sandbox_runtime_usage(id: String) -> Result<SandboxRuntimeUsageDto, String> {
    let docker = connect_docker().map_err(|e| sandbox_connect_error(&e))?;
    let inspect = docker
        .inspect_container(&id, None::<InspectContainerOptions>)
        .await
        .map_err(|e| sandbox_docker_error("inspect sandbox", &id, &e))?;

    let labels = inspect
        .config
        .as_ref()
        .and_then(|cfg| cfg.labels.clone())
        .unwrap_or_default();
    if !sandbox_is_managed(&labels) {
        return Err(sandbox_not_managed_error());
    }

    let running = inspect
        .state
        .as_ref()
        .and_then(|state| state.running)
        .unwrap_or(false);
    if !running {
        return Ok(SandboxRuntimeUsageDto {
            running: false,
            cpu_percent: 0.0,
            memory_usage_mb: 0.0,
            memory_limit_mb: 0.0,
            memory_percent: 0.0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            gpu_attribution_supported: false,
            gpu_message: "Sandbox is not running.".to_string(),
            gpu_processes: Vec::new(),
            gpu_memory_used_bytes: 0,
            gpu_memory_used_human: format_bytes_human(0),
        });
    }

    let stats = container_stats(id.clone()).await?;

    let (gpu_attribution_supported, gpu_message, gpu_processes, gpu_memory_used_bytes) =
        if !cfg!(target_os = "linux") {
            (
                false,
                "Per-sandbox GPU attribution currently requires Linux with NVIDIA tooling."
                    .to_string(),
                Vec::new(),
                0,
            )
        } else if !runtime_setup_command_exists("nvidia-smi") {
            (
            false,
            "nvidia-smi was not found. Install NVIDIA tooling to attribute GPU usage to a sandbox."
                .to_string(),
            Vec::new(),
            0,
        )
        } else {
            let sandbox_id = id.clone();
            let sandbox_short_id = sandbox_short_id(&id);
            match tokio::task::spawn_blocking(move || {
                let inventory = query_nvidia_gpu_inventory()?;
                let inventory_by_uuid = inventory
                    .into_iter()
                    .map(|device| (device.uuid.clone(), device))
                    .collect::<HashMap<_, _>>();
                let mut matched_processes = query_nvidia_compute_processes()?
                    .into_iter()
                    .filter(|process| {
                        linux_process_belongs_to_container(
                            process.pid,
                            &sandbox_id,
                            &sandbox_short_id,
                        )
                    })
                    .map(|process| {
                        let (gpu_index, gpu_name) = inventory_by_uuid
                            .get(&process.gpu_uuid)
                            .map(|device| (device.index, device.name.clone()))
                            .unwrap_or((0, process.gpu_uuid.clone()));
                        GpuProcessDto {
                            gpu_index,
                            gpu_name,
                            pid: process.pid,
                            process_name: process.process_name,
                            memory_used_bytes: process.memory_used_bytes,
                            memory_used_human: process.memory_used_bytes.map(format_bytes_human),
                        }
                    })
                    .collect::<Vec<_>>();
                matched_processes.sort_by(|a, b| {
                    a.gpu_index
                        .cmp(&b.gpu_index)
                        .then(a.pid.cmp(&b.pid))
                        .then(a.process_name.cmp(&b.process_name))
                });

                let gpu_memory_used_bytes = matched_processes
                    .iter()
                    .filter_map(|process| process.memory_used_bytes)
                    .sum::<u64>();
                let gpu_count = matched_processes
                    .iter()
                    .map(|process| process.gpu_index)
                    .collect::<HashSet<_>>()
                    .len();
                let gpu_message = if matched_processes.is_empty() {
                    "No GPU compute workload from this sandbox was detected.".to_string()
                } else {
                    format!(
                        "Matched {} GPU process(es) across {} device(s).",
                        matched_processes.len(),
                        gpu_count.max(1)
                    )
                };

                Ok::<(bool, String, Vec<GpuProcessDto>, u64), String>((
                    true,
                    gpu_message,
                    matched_processes,
                    gpu_memory_used_bytes,
                ))
            })
            .await
            {
                Ok(Ok(result)) => result,
                Ok(Err(error)) => (
                    false,
                    format!("GPU attribution probe failed: {}", error),
                    Vec::new(),
                    0,
                ),
                Err(error) => (
                    false,
                    format!("GPU attribution task failed: {}", error),
                    Vec::new(),
                    0,
                ),
            }
        };

    Ok(SandboxRuntimeUsageDto {
        running: true,
        cpu_percent: stats.cpu_percent,
        memory_usage_mb: stats.memory_usage_mb,
        memory_limit_mb: stats.memory_limit_mb,
        memory_percent: stats.memory_percent,
        network_rx_bytes: stats.network_rx_bytes,
        network_tx_bytes: stats.network_tx_bytes,
        gpu_attribution_supported,
        gpu_message,
        gpu_processes,
        gpu_memory_used_bytes,
        gpu_memory_used_human: format_bytes_human(gpu_memory_used_bytes),
    })
}
