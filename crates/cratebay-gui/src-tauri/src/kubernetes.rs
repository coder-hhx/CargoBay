#[derive(Serialize)]
pub struct K3sStatusDto {
    installed: bool,
    running: bool,
    version: String,
    node_count: u32,
    kubeconfig_path: String,
}

#[tauri::command]
async fn k3s_status() -> Result<K3sStatusDto, String> {
    let status = cratebay_core::k3s::K3sManager::cluster_status().map_err(|e| e.to_string())?;
    let kubeconfig = cratebay_core::k3s::K3sManager::kubeconfig_path()
        .to_string_lossy()
        .to_string();
    Ok(K3sStatusDto {
        installed: status.installed,
        running: status.running,
        version: status.version,
        node_count: status.node_count,
        kubeconfig_path: kubeconfig,
    })
}

#[tauri::command]
async fn k3s_install() -> Result<(), String> {
    cratebay_core::k3s::K3sManager::install(None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn k3s_start() -> Result<(), String> {
    let config = cratebay_core::k3s::K3sConfig::default();
    cratebay_core::k3s::K3sManager::start_cluster(&config).map_err(|e| e.to_string())
}

#[tauri::command]
async fn k3s_stop() -> Result<(), String> {
    cratebay_core::k3s::K3sManager::stop_cluster().map_err(|e| e.to_string())
}

#[tauri::command]
async fn k3s_uninstall() -> Result<(), String> {
    cratebay_core::k3s::K3sManager::uninstall().map_err(|e| e.to_string())
}

// ── Kubernetes dashboard commands ────────────────────────────────────

#[derive(Serialize)]
pub struct K8sPod {
    name: String,
    namespace: String,
    status: String,
    ready: String,
    restarts: u32,
    age: String,
}

#[derive(Serialize)]
pub struct K8sService {
    name: String,
    namespace: String,
    service_type: String,
    cluster_ip: String,
    ports: String,
}

#[derive(Serialize)]
pub struct K8sDeployment {
    name: String,
    namespace: String,
    ready: String,
    up_to_date: u32,
    available: u32,
    age: String,
}

fn k3s_kubeconfig_path() -> String {
    if let Ok(p) = std::env::var("KUBECONFIG") {
        return p;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    // K3s default kubeconfig location
    let k3s_path = format!("{}/.kube/k3s.yaml", home);
    if Path::new(&k3s_path).exists() {
        return k3s_path;
    }
    // Fallback to default kubeconfig
    format!("{}/.kube/config", home)
}

fn run_kubectl(args: &[&str]) -> Result<String, String> {
    let kubeconfig = k3s_kubeconfig_path();
    let mut cmd = Command::new("kubectl");
    cmd.arg("--kubeconfig").arg(&kubeconfig);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.arg("-o").arg("json");
    let out = cmd.output().map_err(|e| format!("kubectl failed: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn k8s_age_from_timestamp(ts: &str) -> String {
    let Ok(created) = chrono::DateTime::parse_from_rfc3339(ts) else {
        return ts.to_string();
    };
    let now = chrono::Utc::now();
    let dur = now.signed_duration_since(created);
    if dur.num_days() > 0 {
        format!("{}d", dur.num_days())
    } else if dur.num_hours() > 0 {
        format!("{}h", dur.num_hours())
    } else if dur.num_minutes() > 0 {
        format!("{}m", dur.num_minutes())
    } else {
        format!("{}s", dur.num_seconds().max(0))
    }
}

#[tauri::command]
async fn k8s_list_namespaces() -> Result<Vec<String>, String> {
    let raw = run_kubectl(&["get", "namespaces"])?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {}", e))?;
    let items = json["items"].as_array().ok_or("No items in response")?;
    let mut ns: Vec<String> = items
        .iter()
        .filter_map(|item| item["metadata"]["name"].as_str().map(|s| s.to_string()))
        .collect();
    ns.sort();
    Ok(ns)
}

#[tauri::command]
async fn k8s_list_pods(namespace: Option<String>) -> Result<Vec<K8sPod>, String> {
    let mut args = vec!["get", "pods"];
    let ns_flag;
    match &namespace {
        Some(ns) if !ns.is_empty() => {
            args.push("-n");
            ns_flag = ns.clone();
            args.push(&ns_flag);
        }
        _ => {
            args.push("--all-namespaces");
        }
    }
    let raw = run_kubectl(&args)?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {}", e))?;
    let items = json["items"].as_array().ok_or("No items in response")?;

    let pods: Vec<K8sPod> = items
        .iter()
        .map(|item| {
            let name = item["metadata"]["name"].as_str().unwrap_or("").to_string();
            let namespace = item["metadata"]["namespace"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let phase = item["status"]["phase"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string();
            let containers = item["status"]["containerStatuses"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let total = containers.len();
            let ready_count = containers
                .iter()
                .filter(|c| c["ready"].as_bool().unwrap_or(false))
                .count();
            let ready = format!("{}/{}", ready_count, total);
            let restarts: u32 = containers
                .iter()
                .map(|c| c["restartCount"].as_u64().unwrap_or(0) as u32)
                .sum();
            let creation = item["metadata"]["creationTimestamp"].as_str().unwrap_or("");
            let age = k8s_age_from_timestamp(creation);

            K8sPod {
                name,
                namespace,
                status: phase,
                ready,
                restarts,
                age,
            }
        })
        .collect();
    Ok(pods)
}

#[tauri::command]
async fn k8s_list_services(namespace: Option<String>) -> Result<Vec<K8sService>, String> {
    let mut args = vec!["get", "services"];
    let ns_flag;
    match &namespace {
        Some(ns) if !ns.is_empty() => {
            args.push("-n");
            ns_flag = ns.clone();
            args.push(&ns_flag);
        }
        _ => {
            args.push("--all-namespaces");
        }
    }
    let raw = run_kubectl(&args)?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {}", e))?;
    let items = json["items"].as_array().ok_or("No items in response")?;

    let services: Vec<K8sService> = items
        .iter()
        .map(|item| {
            let name = item["metadata"]["name"].as_str().unwrap_or("").to_string();
            let namespace = item["metadata"]["namespace"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let service_type = item["spec"]["type"]
                .as_str()
                .unwrap_or("ClusterIP")
                .to_string();
            let cluster_ip = item["spec"]["clusterIP"]
                .as_str()
                .unwrap_or("None")
                .to_string();
            let ports_arr = item["spec"]["ports"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let ports = ports_arr
                .iter()
                .map(|p| {
                    let port = p["port"].as_u64().unwrap_or(0);
                    let proto = p["protocol"].as_str().unwrap_or("TCP");
                    let node_port = p["nodePort"].as_u64();
                    if let Some(np) = node_port {
                        format!("{}:{}/{}", port, np, proto)
                    } else {
                        format!("{}/{}", port, proto)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            K8sService {
                name,
                namespace,
                service_type,
                cluster_ip,
                ports,
            }
        })
        .collect();
    Ok(services)
}

#[tauri::command]
async fn k8s_list_deployments(namespace: Option<String>) -> Result<Vec<K8sDeployment>, String> {
    let mut args = vec!["get", "deployments"];
    let ns_flag;
    match &namespace {
        Some(ns) if !ns.is_empty() => {
            args.push("-n");
            ns_flag = ns.clone();
            args.push(&ns_flag);
        }
        _ => {
            args.push("--all-namespaces");
        }
    }
    let raw = run_kubectl(&args)?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {}", e))?;
    let items = json["items"].as_array().ok_or("No items in response")?;

    let deployments: Vec<K8sDeployment> = items
        .iter()
        .map(|item| {
            let name = item["metadata"]["name"].as_str().unwrap_or("").to_string();
            let namespace = item["metadata"]["namespace"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let replicas = item["status"]["replicas"].as_u64().unwrap_or(0);
            let ready_replicas = item["status"]["readyReplicas"].as_u64().unwrap_or(0);
            let ready = format!("{}/{}", ready_replicas, replicas);
            let up_to_date = item["status"]["updatedReplicas"].as_u64().unwrap_or(0) as u32;
            let available = item["status"]["availableReplicas"].as_u64().unwrap_or(0) as u32;
            let creation = item["metadata"]["creationTimestamp"].as_str().unwrap_or("");
            let age = k8s_age_from_timestamp(creation);

            K8sDeployment {
                name,
                namespace,
                ready,
                up_to_date,
                available,
                age,
            }
        })
        .collect();
    Ok(deployments)
}

#[tauri::command]
async fn k8s_pod_logs(
    name: String,
    namespace: String,
    tail: Option<u32>,
) -> Result<String, String> {
    let tail_str = tail.unwrap_or(200).to_string();
    let kubeconfig = k3s_kubeconfig_path();
    let mut cmd = Command::new("kubectl");
    cmd.arg("--kubeconfig")
        .arg(&kubeconfig)
        .arg("logs")
        .arg(&name)
        .arg("-n")
        .arg(&namespace)
        .arg("--tail")
        .arg(&tail_str);
    let out = cmd
        .output()
        .map_err(|e| format!("kubectl logs failed: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
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
        let ns = r
            .namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "library".to_string());
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
    if !scheme.eq_ignore_ascii_case("bearer") {
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
