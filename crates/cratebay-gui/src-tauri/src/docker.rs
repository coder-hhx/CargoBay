#[derive(Serialize)]
pub struct VolumeInfo {
    name: String,
    driver: String,
    mountpoint: String,
    created_at: String,
    labels: HashMap<String, String>,
    options: HashMap<String, String>,
    scope: String,
}

#[derive(Serialize)]
pub struct LocalImageInfo {
    id: String,
    repo_tags: Vec<String>,
    size_bytes: u64,
    size_human: String,
    created: i64,
}

#[derive(Serialize)]
pub struct ImageInspectInfo {
    id: String,
    repo_tags: Vec<String>,
    size_bytes: u64,
    created: String,
    architecture: String,
    os: String,
    docker_version: String,
    layers: usize,
}

fn format_bytes_human(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[tauri::command]
async fn list_containers() -> Result<Vec<ContainerInfo>, String> {
    let docker = connect_docker()?;

    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };

    let containers = docker
        .list_containers(Some(opts))
        .await
        .map_err(|e| e.to_string())?;

    Ok(containers
        .into_iter()
        .map(|c| {
            let published = c
                .ports
                .unwrap_or_default()
                .into_iter()
                .filter_map(|p| p.public_port.map(|public| (public, p.private_port)))
                .collect::<Vec<_>>();
            let ports = format_published_ports(published);

            let full_id = c.id.unwrap_or_default();
            let id = full_id.chars().take(12).collect::<String>();

            ContainerInfo {
                id,
                name: c
                    .names
                    .unwrap_or_default()
                    .first()
                    .unwrap_or(&String::new())
                    .trim_start_matches('/')
                    .to_string(),
                image: c.image.unwrap_or_default(),
                state: c.state.unwrap_or_default(),
                status: c.status.unwrap_or_default(),
                ports,
            }
        })
        .collect())
}

#[tauri::command]
async fn stop_container(id: String) -> Result<(), String> {
    let docker = connect_docker()?;
    docker
        .stop_container(&id, Some(StopContainerOptions { t: 10 }))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_container(id: String) -> Result<(), String> {
    let docker = connect_docker()?;
    docker
        .start_container(&id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_container(id: String) -> Result<(), String> {
    let docker = connect_docker()?;
    let _ = docker
        .stop_container(&id, Some(StopContainerOptions { t: 10 }))
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
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct RunContainerResult {
    id: String,
    name: String,
    image: String,
    login_cmd: String,
}

#[tauri::command]
async fn docker_run(
    image: String,
    name: Option<String>,
    cpus: Option<u32>,
    memory_mb: Option<u64>,
    pull: bool,
    env: Option<Vec<String>>,
) -> Result<RunContainerResult, String> {
    validation::validate_image_reference(&image)
        .map_err(|e| format!("Invalid image reference '{}': {}", image, e))?;
    if let Some(ref n) = name {
        validation::validate_container_name(n)
            .map_err(|e| format!("Invalid container name '{}': {}", n, e))?;
    }

    let docker = connect_docker()?;

    if pull {
        docker_pull_image(&docker, &image).await?;
    }

    let mut host_config = HostConfig::default();
    if let Some(c) = cpus {
        host_config.nano_cpus = Some((c as i64) * 1_000_000_000);
    }
    if let Some(mb) = memory_mb {
        let bytes = (mb as i64).saturating_mul(1024).saturating_mul(1024);
        host_config.memory = Some(bytes);
    }

    let config = Config::<String> {
        image: Some(image.clone()),
        host_config: Some(host_config),
        env: env.filter(|v| !v.is_empty()),
        ..Default::default()
    };

    let create_opts = name.as_deref().map(|n| CreateContainerOptions::<String> {
        name: n.to_string(),
        platform: None,
    });

    let result = docker
        .create_container(create_opts, config)
        .await
        .map_err(|e| e.to_string())?;

    docker
        .start_container(&result.id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| e.to_string())?;

    let id = result.id.chars().take(12).collect::<String>();
    let display = name.clone().unwrap_or_else(|| id.clone());
    let login_cmd = format!("docker exec -it {} /bin/sh", display);

    Ok(RunContainerResult {
        id,
        name: display,
        image,
        login_cmd,
    })
}

#[tauri::command]
fn container_login_cmd(container: String, shell: String) -> String {
    format!("docker exec -it {} {}", container, shell)
}

#[tauri::command]
async fn container_logs(
    id: String,
    tail: Option<String>,
    timestamps: bool,
) -> Result<String, String> {
    let docker = connect_docker()?;

    let tail_value = tail.unwrap_or_else(|| "200".to_string());

    let opts = LogsOptions::<String> {
        follow: false,
        stdout: true,
        stderr: true,
        timestamps,
        tail: tail_value,
        ..Default::default()
    };

    let mut stream = docker.logs(&id, Some(opts));
    let mut output = String::new();
    while let Some(chunk) = stream.try_next().await.map_err(|e| e.to_string())? {
        output.push_str(&chunk.to_string());
    }

    Ok(output)
}

#[tauri::command]
async fn container_logs_stream(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    timestamps: bool,
) -> Result<(), String> {
    // Stop any existing stream for this container
    if let Ok(mut handles) = state.log_stream_handles.lock() {
        if let Some(handle) = handles.remove(&id) {
            handle.abort();
        }
    }

    let docker = connect_docker()?;
    let container_id = id.clone();

    let opts = LogsOptions::<String> {
        follow: true,
        stdout: true,
        stderr: true,
        timestamps,
        tail: "100".to_string(),
        ..Default::default()
    };

    let mut stream = docker.logs(&container_id, Some(opts));
    let emit_id = id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        while let Some(chunk) = stream.try_next().await.unwrap_or(None) {
            let payload = serde_json::json!({
                "container_id": emit_id,
                "data": chunk.to_string()
            });
            if app.emit("container-log", payload).is_err() {
                break;
            }
        }
        let _ = app.emit("container-log-end", &emit_id);
    });

    if let Ok(mut handles) = state.log_stream_handles.lock() {
        handles.insert(id, handle);
    }

    Ok(())
}

#[tauri::command]
async fn container_logs_stream_stop(state: State<'_, AppState>, id: String) -> Result<(), String> {
    if let Ok(mut handles) = state.log_stream_handles.lock() {
        if let Some(handle) = handles.remove(&id) {
            handle.abort();
        }
    }
    Ok(())
}

#[tauri::command]
async fn container_exec(container_id: String, command: String) -> Result<String, String> {
    let docker = connect_docker()?;

    let cmd_parts: Vec<&str> = command.split_whitespace().collect();
    if cmd_parts.is_empty() {
        return Err("Empty command".into());
    }

    let exec = docker
        .create_exec(
            &container_id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(cmd_parts.into_iter().map(String::from).collect()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("Failed to create exec: {}", e))?;

    let output = docker
        .start_exec(&exec.id, None)
        .await
        .map_err(|e| format!("Failed to start exec: {}", e))?;

    let mut result = String::new();
    if let StartExecResults::Attached { mut output, .. } = output {
        while let Some(chunk) = output.try_next().await.map_err(|e| e.to_string())? {
            result.push_str(&chunk.to_string());
        }
    }

    Ok(result)
}

#[tauri::command]
fn container_exec_interactive_cmd(container_id: String) -> String {
    let docker_host = docker_host_for_cli();
    if let Some(host) = docker_host {
        format!(
            "DOCKER_HOST={} docker exec -it {} /bin/sh",
            host, container_id
        )
    } else {
        format!("docker exec -it {} /bin/sh", container_id)
    }
}

#[derive(Debug, Serialize)]
pub struct EnvVar {
    key: String,
    value: String,
}

#[tauri::command]
async fn container_env(id: String) -> Result<Vec<EnvVar>, String> {
    let docker = connect_docker()?;

    let inspect = docker
        .inspect_container(&id, None::<InspectContainerOptions>)
        .await
        .map_err(|e| format!("Failed to inspect container {}: {}", id, e))?;

    let env_list = inspect.config.and_then(|c| c.env).unwrap_or_default();

    Ok(env_list
        .into_iter()
        .map(|entry| {
            if let Some((k, v)) = entry.split_once('=') {
                EnvVar {
                    key: k.to_string(),
                    value: v.to_string(),
                }
            } else {
                EnvVar {
                    key: entry,
                    value: String::new(),
                }
            }
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub struct ContainerStats {
    cpu_percent: f64,
    memory_usage_mb: f64,
    memory_limit_mb: f64,
    memory_percent: f64,
    network_rx_bytes: u64,
    network_tx_bytes: u64,
}

#[tauri::command]
async fn container_stats(id: String) -> Result<ContainerStats, String> {
    let docker = connect_docker()?;

    let opts = StatsOptions {
        stream: false,
        one_shot: true,
    };

    let mut stream = docker.stats(&id, Some(opts));
    let stats = stream
        .try_next()
        .await
        .map_err(|e| format!("Failed to get stats for {}: {}", id, e))?
        .ok_or_else(|| format!("No stats returned for container {}", id))?;

    // Calculate CPU percent
    let cpu_percent = {
        let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as f64
            - stats.precpu_stats.cpu_usage.total_usage as f64;
        let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64
            - stats.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
        let num_cpus = stats.cpu_stats.online_cpus.unwrap_or(1) as f64;

        if system_delta > 0.0 && cpu_delta >= 0.0 {
            (cpu_delta / system_delta) * num_cpus * 100.0
        } else {
            0.0
        }
    };

    // Memory usage
    let memory_usage = stats.memory_stats.usage.unwrap_or(0);
    let memory_limit = stats.memory_stats.limit.unwrap_or(0);
    let memory_usage_mb = memory_usage as f64 / 1024.0 / 1024.0;
    let memory_limit_mb = memory_limit as f64 / 1024.0 / 1024.0;
    let memory_percent = if memory_limit > 0 {
        (memory_usage as f64 / memory_limit as f64) * 100.0
    } else {
        0.0
    };

    // Network stats
    let (network_rx_bytes, network_tx_bytes) = stats
        .networks
        .as_ref()
        .map(|nets| {
            nets.values().fold((0u64, 0u64), |(rx, tx), net| {
                (rx + net.rx_bytes, tx + net.tx_bytes)
            })
        })
        .unwrap_or((0, 0));

    Ok(ContainerStats {
        cpu_percent,
        memory_usage_mb,
        memory_limit_mb,
        memory_percent,
        network_rx_bytes,
        network_tx_bytes,
    })
}

#[derive(Debug, Serialize)]
pub struct ImageSearchResult {
    source: String,
    reference: String,
    description: String,
    stars: Option<u64>,
    pulls: Option<u64>,
    official: bool,
}

#[derive(Deserialize)]
struct DockerHubSearchResponse {
    results: Vec<DockerHubRepo>,
}

#[derive(Deserialize)]
struct DockerHubRepo {
    #[serde(alias = "repo_name")]
    name: String,
    #[serde(alias = "repo_owner")]
    namespace: Option<String>,
    #[serde(alias = "short_description")]
    description: Option<String>,
    star_count: Option<u64>,
    pull_count: Option<u64>,
    is_official: Option<bool>,
}

#[derive(Deserialize)]
struct RegistryTagsResponse {
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RegistryTokenResponse {
    token: Option<String>,
    access_token: Option<String>,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!(
            "CrateBay/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/coder-hhx/CrateBay)"
        ))
        .build()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn image_search(
    query: String,
    source: String,
    limit: usize,
) -> Result<Vec<ImageSearchResult>, String> {
    let client = http_client()?;
    let src = source.to_ascii_lowercase();
    let mut items: Vec<ImageSearchResult> = Vec::new();
    let mut did_any = false;

    if matches!(src.as_str(), "all" | "dockerhub" | "hub" | "docker") {
        did_any = true;
        items.extend(search_dockerhub(&client, &query, limit).await?);
    }
    if matches!(src.as_str(), "all" | "quay") {
        did_any = true;
        items.extend(search_quay(&client, &query, limit).await?);
    }

    if !did_any {
        return Err(format!("Unknown source: {}", source));
    }

    Ok(items)
}

#[tauri::command]
async fn image_tags(reference: String, limit: usize) -> Result<Vec<String>, String> {
    let client = http_client()?;
    let Some((registry, repo)) = parse_registry_reference(&reference) else {
        return Err("Invalid reference. Expected e.g. ghcr.io/org/image".into());
    };
    list_registry_tags(&client, &registry, &repo, limit).await
}

#[tauri::command]
async fn image_load(path: String) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path is required".into());
    }

    let docker = connect_docker()?;
    let archive = PathBuf::from(trimmed);
    let file = tokio::fs::File::open(&archive)
        .await
        .map_err(|e| format!("Failed to open {}: {}", archive.display(), e))?;

    let read_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let read_error_capture = read_error.clone();
    let archive_display = archive.display().to_string();

    let byte_stream = FramedRead::new(file, BytesCodec::new()).filter_map(move |result| {
        let read_error_capture = read_error_capture.clone();
        let archive_display = archive_display.clone();
        async move {
            match result {
                Ok(buf) => Some(buf.freeze()),
                Err(e) => {
                    let mut guard = read_error_capture.lock().unwrap_or_else(|e| e.into_inner());
                    *guard = Some(format!(
                        "Failed to read image archive {}: {}",
                        archive_display, e
                    ));
                    None
                }
            }
        }
    });

    let mut stream = docker.import_image_stream(ImportImageOptions::default(), byte_stream, None);
    let mut out = String::new();
    while let Some(progress) = stream.try_next().await.map_err(|e| e.to_string())? {
        if let Some(error) = progress.error {
            return Err(error);
        }
        if let Some(line) = progress.stream {
            out.push_str(&line);
            continue;
        }
        if let Some(status) = progress.status {
            if let Some(p) = progress.progress {
                out.push_str(&format!("{} {}\n", status, p));
            } else {
                out.push_str(&format!("{}\n", status));
            }
        }
    }

    if let Some(e) = read_error.lock().unwrap_or_else(|e| e.into_inner()).take() {
        return Err(e);
    }

    Ok(out.trim().to_string())
}

#[tauri::command]
async fn image_push(reference: String) -> Result<String, String> {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return Err("reference is required".into());
    }
    validation::validate_image_reference(trimmed)?;
    let docker = connect_docker()?;

    let (repo, tag) = split_image_reference(trimmed);
    let auth = cratebay_core::docker_auth::resolve_registry_auth_for_image(trimmed)?;
    let creds = auth.map(|a| DockerCredentials {
        username: a.username,
        password: a.password,
        serveraddress: Some(a.server_address),
        identitytoken: a.identity_token,
        ..Default::default()
    });

    let mut stream = docker.push_image(&repo, Some(PushImageOptions { tag }), creds);
    let mut out = String::new();
    while let Some(progress) = stream.try_next().await.map_err(|e| e.to_string())? {
        if let Some(status) = progress.status {
            if let Some(p) = progress.progress {
                out.push_str(&format!("{} {}\n", status, p));
            } else {
                out.push_str(&format!("{}\n", status));
            }
        }
    }

    Ok(out.trim().to_string())
}

#[tauri::command]
async fn image_pack_container(container: String, tag: String) -> Result<String, String> {
    let c = container.trim();
    if c.is_empty() {
        return Err("container is required".into());
    }
    let t = tag.trim();
    if t.is_empty() {
        return Err("tag is required".into());
    }

    let docker = connect_docker()?;
    let (repo, image_tag) = split_image_reference(t);
    let opts = CommitContainerOptions {
        container: c,
        repo: repo.as_str(),
        tag: image_tag.as_str(),
        pause: true,
        ..Default::default()
    };
    let result = docker
        .commit_container(opts, Config::<String>::default())
        .await
        .map_err(|e| e.to_string())?;

    let id = result
        .id
        .clone()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| result.expected.clone().filter(|v| !v.trim().is_empty()));
    if let Some(id) = id {
        Ok(id)
    } else {
        serde_json::to_string(&result).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------
// OS Image management commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct OsImageDto {
    id: String,
    name: String,
    version: String,
    arch: String,
    size_bytes: u64,
    status: String,
    default_cmdline: String,
}

#[derive(Debug, Serialize)]
pub struct OsImageDownloadProgressDto {
    image_id: String,
    current_file: String,
    bytes_downloaded: u64,
    bytes_total: u64,
    done: bool,
    error: Option<String>,
}

#[tauri::command]
fn image_catalog() -> Vec<OsImageDto> {
    cratebay_core::images::list_available_images()
        .into_iter()
        .map(|e| OsImageDto {
            id: e.id,
            name: e.name,
            version: e.version,
            arch: e.arch,
            size_bytes: e.size_bytes,
            status: match e.status {
                cratebay_core::images::ImageStatus::NotDownloaded => "not_downloaded".into(),
                cratebay_core::images::ImageStatus::Downloading => "downloading".into(),
                cratebay_core::images::ImageStatus::Ready => "ready".into(),
            },
            default_cmdline: e.default_cmdline,
        })
        .collect()
}

#[tauri::command]
async fn image_download_os(image_id: String) -> Result<(), String> {
    cratebay_core::images::download_image(&image_id, |_file, _downloaded, _total| {})
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn image_download_status(image_id: String) -> OsImageDownloadProgressDto {
    let p = cratebay_core::images::read_download_progress(&image_id);
    OsImageDownloadProgressDto {
        image_id: p.image_id,
        current_file: p.current_file,
        bytes_downloaded: p.bytes_downloaded,
        bytes_total: p.bytes_total,
        done: p.done,
        error: p.error,
    }
}

#[tauri::command]
fn image_delete_os(image_id: String) -> Result<(), String> {
    cratebay_core::images::delete_image(&image_id).map_err(|e| e.to_string())
}
