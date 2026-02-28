#[cfg_attr(mobile, tauri::mobile_entry_point)]

use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::service::HostConfig;
use futures_util::stream::TryStreamExt;
use reqwest::header::WWW_AUTHENTICATE;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tauri::State;

pub struct AppState {
    hv: Box<dyn cargobay_core::hypervisor::Hypervisor>,
}

fn detect_docker_socket() -> Option<String> {
    // Unix socket detection (macOS / Linux)
    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates = [
            format!("{}/.colima/default/docker.sock", home),
            format!("{}/.orbstack/run/docker.sock", home),
            "/var/run/docker.sock".to_string(),
            format!("{}/.docker/run/docker.sock", home),
        ];
        if let Some(sock) = candidates.into_iter().find(|p| Path::new(p).exists()) {
            return Some(sock);
        }
    }

    None
}

fn connect_docker() -> Result<Docker, String> {
    // Check DOCKER_HOST env first
    if std::env::var("DOCKER_HOST").is_ok() {
        return Docker::connect_with_local_defaults()
            .map_err(|e| format!("Failed to connect via DOCKER_HOST: {}", e));
    }

    #[cfg(unix)]
    {
        if let Some(sock) = detect_docker_socket() {
            return Docker::connect_with_socket(&sock, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| format!("Failed to connect to Docker at {}: {}", sock, e));
        }
        return Err("No Docker socket found. Set DOCKER_HOST or install Docker/Colima/OrbStack.".into());
    }

    #[cfg(windows)]
    {
        let candidates = [
            r"//./pipe/docker_engine",
            r"//./pipe/dockerDesktopLinuxEngine",
        ];
        for pipe in &candidates {
            if let Ok(d) = Docker::connect_with_named_pipe(pipe, 120, bollard::API_DEFAULT_VERSION) {
                return Ok(d);
            }
        }
        return Err("No Docker named pipe found. Set DOCKER_HOST or install Docker Desktop.".into());
    }

    #[cfg(not(any(unix, windows)))]
    {
        Docker::connect_with_local_defaults().map_err(|e| format!("Failed to connect to Docker: {}", e))
    }
}

fn docker_host_for_cli() -> Option<String> {
    if let Ok(v) = std::env::var("DOCKER_HOST") {
        return Some(v);
    }
    #[cfg(unix)]
    {
        detect_docker_socket().map(|sock| format!("unix://{}", sock))
    }
    #[cfg(windows)]
    {
        None
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

#[derive(Serialize)]
pub struct ContainerInfo {
    id: String,
    name: String,
    image: String,
    state: String,
    status: String,
    ports: String,
}

#[tauri::command]
async fn list_containers() -> Result<Vec<ContainerInfo>, String> {
    let docker = connect_docker()?;

    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };

    let containers = docker.list_containers(Some(opts)).await.map_err(|e| e.to_string())?;

    Ok(containers.into_iter().map(|c| {
        let ports = c.ports.unwrap_or_default().iter().filter_map(|p| {
            p.public_port.map(|pub_p| format!("{}:{}", pub_p, p.private_port))
        }).collect::<Vec<_>>().join(", ");

        let full_id = c.id.unwrap_or_default();
        let id = full_id.chars().take(12).collect::<String>();

        ContainerInfo {
            id,
            name: c.names.unwrap_or_default().first()
                .unwrap_or(&String::new()).trim_start_matches('/').to_string(),
            image: c.image.unwrap_or_default(),
            state: c.state.unwrap_or_default(),
            status: c.status.unwrap_or_default(),
            ports,
        }
    }).collect())
}

#[tauri::command]
async fn stop_container(id: String) -> Result<(), String> {
    let docker = connect_docker()?;
    docker.stop_container(&id, Some(StopContainerOptions { t: 10 })).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_container(id: String) -> Result<(), String> {
    let docker = connect_docker()?;
    docker.start_container(&id, None::<StartContainerOptions<String>>).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_container(id: String) -> Result<(), String> {
    let docker = connect_docker()?;
    let _ = docker.stop_container(&id, Some(StopContainerOptions { t: 10 })).await;
    docker.remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await.map_err(|e| e.to_string())
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
) -> Result<RunContainerResult, String> {
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
    name: String,
    namespace: Option<String>,
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
        .user_agent("CargoBay/0.1.0 (+https://github.com/coder-hhx/CargoBay)")
        .build()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn image_search(query: String, source: String, limit: usize) -> Result<Vec<ImageSearchResult>, String> {
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
    let docker_host = docker_host_for_cli();
    tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new("docker");
        cmd.arg("load").arg("-i").arg(&path);
        if let Some(host) = docker_host {
            cmd.env("DOCKER_HOST", host);
        }
        let out = cmd.output().map_err(|e| format!("Failed to run docker: {}", e))?;
        if !out.status.success() {
            return Err(format!(
                "docker load failed (exit {}): {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn image_push(reference: String) -> Result<String, String> {
    let docker_host = docker_host_for_cli();
    tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new("docker");
        cmd.arg("push").arg(&reference);
        if let Some(host) = docker_host {
            cmd.env("DOCKER_HOST", host);
        }
        let out = cmd.output().map_err(|e| format!("Failed to run docker: {}", e))?;
        if !out.status.success() {
            return Err(format!(
                "docker push failed (exit {}): {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn image_pack_container(container: String, tag: String) -> Result<String, String> {
    let docker_host = docker_host_for_cli();
    tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new("docker");
        cmd.arg("commit").arg(&container).arg(&tag);
        if let Some(host) = docker_host {
            cmd.env("DOCKER_HOST", host);
        }
        let out = cmd.output().map_err(|e| format!("Failed to run docker: {}", e))?;
        if !out.status.success() {
            return Err(format!(
                "docker commit failed (exit {}): {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Serialize)]
pub struct VmInfoDto {
    id: String,
    name: String,
    state: String,
    cpus: u32,
    memory_mb: u64,
    disk_gb: u64,
    rosetta_enabled: bool,
    mounts: Vec<SharedDirectoryDto>,
}

#[derive(Debug, Serialize)]
pub struct SharedDirectoryDto {
    tag: String,
    host_path: String,
    guest_path: String,
    read_only: bool,
}

impl From<cargobay_core::hypervisor::SharedDirectory> for SharedDirectoryDto {
    fn from(value: cargobay_core::hypervisor::SharedDirectory) -> Self {
        Self {
            tag: value.tag,
            host_path: value.host_path,
            guest_path: value.guest_path,
            read_only: value.read_only,
        }
    }
}

fn vm_state_to_string(state: cargobay_core::hypervisor::VmState) -> String {
    match state {
        cargobay_core::hypervisor::VmState::Running => "running".into(),
        cargobay_core::hypervisor::VmState::Stopped => "stopped".into(),
        cargobay_core::hypervisor::VmState::Creating => "creating".into(),
    }
}

#[tauri::command]
fn vm_list(state: State<'_, AppState>) -> Result<Vec<VmInfoDto>, String> {
    let vms = state.hv.list_vms().map_err(|e| e.to_string())?;
    Ok(vms
        .into_iter()
        .map(|vm| VmInfoDto {
            id: vm.id,
            name: vm.name,
            state: vm_state_to_string(vm.state),
            cpus: vm.cpus,
            memory_mb: vm.memory_mb,
            disk_gb: 0,
            rosetta_enabled: vm.rosetta_enabled,
            mounts: vm
                .shared_dirs
                .into_iter()
                .map(SharedDirectoryDto::from)
                .collect(),
        })
        .collect())
}

#[tauri::command]
fn vm_create(
    state: State<'_, AppState>,
    name: String,
    cpus: u32,
    memory_mb: u64,
    disk_gb: u64,
    rosetta: bool,
) -> Result<String, String> {
    use cargobay_core::hypervisor::VmConfig;
    let config = VmConfig {
        name,
        cpus,
        memory_mb,
        disk_gb,
        rosetta,
        shared_dirs: vec![],
    };
    state.hv.create_vm(config).map_err(|e| e.to_string())
}

#[tauri::command]
fn vm_start(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.hv.start_vm(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn vm_stop(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.hv.stop_vm(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn vm_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.hv.delete_vm(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn vm_login_cmd(name: String, user: String, host: String, port: Option<u16>) -> Result<String, String> {
    let Some(port) = port else {
        return Err("VM login is not available yet. Specify an SSH port.".into());
    };
    Ok(format!("ssh {}@{} -p {}\n# VM: {}", user, host, port, name))
}

#[tauri::command]
fn vm_mount_add(
    state: State<'_, AppState>,
    vm: String,
    tag: String,
    host_path: String,
    guest_path: String,
    readonly: bool,
) -> Result<(), String> {
    use cargobay_core::hypervisor::SharedDirectory;
    let share = SharedDirectory {
        tag,
        host_path,
        guest_path,
        read_only: readonly,
    };
    state.hv.mount_virtiofs(&vm, &share).map_err(|e| e.to_string())
}

#[tauri::command]
fn vm_mount_remove(state: State<'_, AppState>, vm: String, tag: String) -> Result<(), String> {
    state.hv.unmount_virtiofs(&vm, &tag).map_err(|e| e.to_string())
}

#[tauri::command]
fn vm_mount_list(state: State<'_, AppState>, vm: String) -> Result<Vec<SharedDirectoryDto>, String> {
    let mounts = state.hv.list_virtiofs_mounts(&vm).map_err(|e| e.to_string())?;
    Ok(mounts.into_iter().map(SharedDirectoryDto::from).collect())
}

async fn docker_pull_image(docker: &Docker, reference: &str) -> Result<(), String> {
    let (from_image, tag) = split_image_reference(reference);
    let opts = CreateImageOptions {
        from_image,
        tag,
        ..Default::default()
    };

    let mut stream = docker.create_image(Some(opts), None, None);
    while let Some(_progress) = stream.try_next().await.map_err(|e| e.to_string())? {}
    Ok(())
}

fn split_image_reference(reference: &str) -> (String, String) {
    let no_digest = reference.split('@').next().unwrap_or(reference);
    let last_slash = no_digest.rfind('/').unwrap_or(0);
    let last_colon = no_digest.rfind(':');

    if let Some(colon_idx) = last_colon {
        if colon_idx > last_slash {
            let image = &no_digest[..colon_idx];
            let tag = &no_digest[(colon_idx + 1)..];
            if !image.is_empty() && !tag.is_empty() {
                return (image.to_string(), tag.to_string());
            }
        }
    }

    (no_digest.to_string(), "latest".to_string())
}

async fn search_dockerhub(
    client: &reqwest::Client,
    query: &str,
    limit: usize,
) -> Result<Vec<ImageSearchResult>, String> {
    let mut url = reqwest::Url::parse("https://hub.docker.com/v2/search/repositories/")
        .map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("page_size", &limit.to_string());

    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Docker Hub search failed: HTTP {}", resp.status()));
    }

    let data: DockerHubSearchResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();

    for r in data.results.into_iter().take(limit) {
        let ns = r.namespace.unwrap_or_else(|| "library".to_string());
        let name = if ns == "library" {
            r.name.clone()
        } else {
            format!("{}/{}", ns, r.name)
        };

        out.push(ImageSearchResult {
            source: "dockerhub".into(),
            reference: name,
            description: r.description.unwrap_or_default(),
            stars: r.star_count,
            pulls: r.pull_count,
            official: r.is_official.unwrap_or(false),
        });
    }

    Ok(out)
}

async fn search_quay(
    client: &reqwest::Client,
    query: &str,
    limit: usize,
) -> Result<Vec<ImageSearchResult>, String> {
    let mut url = reqwest::Url::parse("https://quay.io/api/v1/find/repositories")
        .map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("query", query);

    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Quay search failed: HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let results = json
        .get("results")
        .and_then(|v| v.as_array())
        .or_else(|| json.get("repositories").and_then(|v| v.as_array()))
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for item in results.into_iter().take(limit) {
        let full_name = item
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                let ns = item
                    .get("namespace")
                    .or_else(|| item.get("namespace_name"))
                    .and_then(|v| v.as_str())?;
                let name = item
                    .get("repo_name")
                    .or_else(|| item.get("name"))
                    .and_then(|v| v.as_str())?;
                Some(format!("{}/{}", ns, name))
            })
            .unwrap_or_else(|| "<unknown>".to_string());

        let desc = item
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let stars = item
            .get("stars")
            .or_else(|| item.get("star_count"))
            .and_then(|v| v.as_u64());

        out.push(ImageSearchResult {
            source: "quay".into(),
            reference: format!("quay.io/{}", full_name),
            description: desc,
            stars,
            pulls: None,
            official: false,
        });
    }

    Ok(out)
}

fn parse_registry_reference(reference: &str) -> Option<(String, String)> {
    let no_digest = reference.split('@').next().unwrap_or(reference);
    let no_tag = {
        let last_slash = no_digest.rfind('/').unwrap_or(0);
        if let Some(colon_idx) = no_digest.rfind(':') {
            if colon_idx > last_slash {
                &no_digest[..colon_idx]
            } else {
                no_digest
            }
        } else {
            no_digest
        }
    };

    let (first, rest) = no_tag.split_once('/')?;
    if !(first.contains('.') || first.contains(':') || first == "localhost") {
        return None;
    }
    if rest.is_empty() {
        return None;
    }
    Some((first.to_string(), rest.to_string()))
}

async fn list_registry_tags(
    client: &reqwest::Client,
    registry: &str,
    repository: &str,
    limit: usize,
) -> Result<Vec<String>, String> {
    let url = format!("https://{}/v2/{}/tags/list", registry, repository);
    let mut resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        let auth = resp
            .headers()
            .get(WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| "Registry requires auth (missing WWW-Authenticate)".to_string())?;

        let fallback_scope = format!("repository:{}:pull", repository);
        let token = fetch_bearer_token(client, auth, Some(&fallback_scope)).await?;

        resp = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
    }

    if !resp.status().is_success() {
        return Err(format!(
            "Failed to list tags for {}/{}: HTTP {}",
            registry,
            repository,
            resp.status()
        ));
    }

    let data: RegistryTagsResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut tags = data.tags.unwrap_or_default();
    tags.sort();
    tags.truncate(limit);
    Ok(tags)
}

async fn fetch_bearer_token(
    client: &reqwest::Client,
    auth_header: &str,
    fallback_scope: Option<&str>,
) -> Result<String, String> {
    let params = parse_bearer_auth_params(auth_header)
        .ok_or_else(|| format!("Unsupported WWW-Authenticate header: {}", auth_header))?;

    let realm = params
        .get("realm")
        .ok_or_else(|| "WWW-Authenticate missing realm".to_string())?;

    let service = params.get("service").map(String::as_str);
    let scope = params.get("scope").map(String::as_str).or(fallback_scope);

    let mut url = reqwest::Url::parse(realm).map_err(|e| e.to_string())?;
    {
        let mut qp = url.query_pairs_mut();
        if let Some(s) = service {
            qp.append_pair("service", s);
        }
        if let Some(s) = scope {
            qp.append_pair("scope", s);
        }
    }

    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Token request failed: HTTP {}", resp.status()));
    }

    let token: RegistryTokenResponse = resp.json().await.map_err(|e| e.to_string())?;
    token
        .token
        .or(token.access_token)
        .ok_or_else(|| "Token response missing token".to_string())
}

fn parse_bearer_auth_params(header_value: &str) -> Option<HashMap<String, String>> {
    let header_value = header_value.trim();
    let mut parts = header_value.splitn(2, ' ');
    let scheme = parts.next()?.trim();
    if scheme.to_ascii_lowercase() != "bearer" {
        return None;
    }
    let rest = parts.next()?.trim();

    let mut out = HashMap::new();
    for part in rest.split(',') {
        let part = part.trim();
        let mut kv = part.splitn(2, '=');
        let key = kv.next()?.trim();
        let val = kv.next()?.trim().trim_matches('"');
        out.insert(key.to_string(), val.to_string());
    }
    Some(out)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            hv: cargobay_core::create_hypervisor(),
        })
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_containers,
            stop_container,
            start_container,
            remove_container,
            docker_run,
            container_login_cmd,
            image_search,
            image_tags,
            image_load,
            image_push,
            image_pack_container,
            vm_list,
            vm_create,
            vm_start,
            vm_stop,
            vm_delete,
            vm_login_cmd,
            vm_mount_add,
            vm_mount_remove,
            vm_mount_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
