fn ollama_base_url() -> String {
    std::env::var("CRATEBAY_OLLAMA_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OLLAMA_BASE_URL.to_string())
}

fn ollama_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(900))
        .user_agent(concat!(
            "CrateBay/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/coder-hhx/CrateBay)"
        ))
        .build()
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct GpuDeviceDto {
    index: u32,
    name: String,
    utilization_percent: Option<f64>,
    memory_used_bytes: Option<u64>,
    memory_total_bytes: Option<u64>,
    memory_used_human: Option<String>,
    memory_total_human: Option<String>,
    temperature_celsius: Option<f64>,
    power_watts: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct GpuStatusDto {
    available: bool,
    utilization_supported: bool,
    backend: String,
    message: String,
    devices: Vec<GpuDeviceDto>,
}

#[derive(Debug, Serialize)]
pub struct OllamaStatusDto {
    installed: bool,
    running: bool,
    version: String,
    base_url: String,
}

#[derive(Debug, Serialize)]
pub struct OllamaModelDto {
    name: String,
    size_bytes: u64,
    size_human: String,
    modified_at: String,
    digest: String,
    family: String,
    parameter_size: String,
    quantization_level: String,
}

#[derive(Debug, Deserialize)]
struct OllamaVersionResponse {
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaTagModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagModel {
    name: String,
    #[serde(default)]
    modified_at: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: String,
    #[serde(default)]
    details: OllamaModelDetails,
}

#[derive(Debug, Default, Deserialize)]
struct OllamaModelDetails {
    #[serde(default)]
    family: String,
    #[serde(default)]
    parameter_size: String,
    #[serde(default)]
    quantization_level: String,
}

fn parse_optional_metric_f64(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("n/a") || trimmed == "-" {
        None
    } else {
        trimmed.parse::<f64>().ok()
    }
}

fn parse_optional_metric_u64(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("n/a") || trimmed == "-" {
        None
    } else {
        trimmed
            .parse::<u64>()
            .ok()
            .or_else(|| trimmed.parse::<f64>().ok().map(|item| item.round() as u64))
    }
}

fn gpu_status_unavailable(message: impl Into<String>) -> GpuStatusDto {
    GpuStatusDto {
        available: false,
        utilization_supported: false,
        backend: String::new(),
        message: message.into(),
        devices: Vec::new(),
    }
}

fn query_nvidia_gpu_status() -> Result<GpuStatusDto, String> {
    let output = runtime_setup_run(
        "nvidia-smi",
        &[
            "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw",
            "--format=csv,noheader,nounits",
        ],
    )?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("nvidia-smi exited with status {}", output.status)
        } else {
            detail
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    for (line_index, line) in stdout.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cols = trimmed
            .split(',')
            .map(|item| item.trim())
            .collect::<Vec<_>>();
        if cols.len() < 7 {
            continue;
        }
        let index = cols[0].parse::<u32>().unwrap_or(line_index as u32);
        let memory_used_bytes = parse_optional_metric_u64(cols[3]).map(|mb| mb * 1024 * 1024);
        let memory_total_bytes = parse_optional_metric_u64(cols[4]).map(|mb| mb * 1024 * 1024);
        devices.push(GpuDeviceDto {
            index,
            name: cols[1].to_string(),
            utilization_percent: parse_optional_metric_f64(cols[2]),
            memory_used_bytes,
            memory_total_bytes,
            memory_used_human: memory_used_bytes.map(format_bytes_human),
            memory_total_human: memory_total_bytes.map(format_bytes_human),
            temperature_celsius: parse_optional_metric_f64(cols[5]),
            power_watts: parse_optional_metric_f64(cols[6]),
        });
    }

    if devices.is_empty() {
        return Ok(gpu_status_unavailable(
            "nvidia-smi is installed, but no GPU devices were reported.",
        ));
    }

    Ok(GpuStatusDto {
        available: true,
        utilization_supported: true,
        backend: "nvidia-smi".to_string(),
        message: format!(
            "Live GPU telemetry is available for {} device(s).",
            devices.len()
        ),
        devices,
    })
}

#[derive(Debug)]
struct NvidiaGpuInventory {
    index: u32,
    uuid: String,
    name: String,
}

#[derive(Debug)]
struct NvidiaGpuProcessSample {
    gpu_uuid: String,
    pid: u32,
    process_name: String,
    memory_used_bytes: Option<u64>,
}

fn query_nvidia_gpu_inventory() -> Result<Vec<NvidiaGpuInventory>, String> {
    let output = runtime_setup_run(
        "nvidia-smi",
        &[
            "--query-gpu=index,uuid,name",
            "--format=csv,noheader,nounits",
        ],
    )?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("nvidia-smi exited with status {}", output.status)
        } else {
            detail
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    for (line_index, line) in stdout.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cols = trimmed
            .split(',')
            .map(|item| item.trim())
            .collect::<Vec<_>>();
        if cols.len() < 3 {
            continue;
        }
        devices.push(NvidiaGpuInventory {
            index: cols[0].parse::<u32>().unwrap_or(line_index as u32),
            uuid: cols[1].to_string(),
            name: cols[2].to_string(),
        });
    }
    Ok(devices)
}

fn query_nvidia_compute_processes() -> Result<Vec<NvidiaGpuProcessSample>, String> {
    let output = runtime_setup_run(
        "nvidia-smi",
        &[
            "--query-compute-apps=gpu_uuid,pid,process_name,used_gpu_memory",
            "--format=csv,noheader,nounits",
        ],
    )?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!(
        "{}
{}",
        stdout.trim(),
        stderr.trim()
    )
    .to_lowercase();
    if combined.contains("no running compute processes found") {
        return Ok(Vec::new());
    }

    if !output.status.success() {
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            format!("nvidia-smi exited with status {}", output.status)
        } else {
            detail.to_string()
        });
    }

    let mut processes = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cols = trimmed
            .split(',')
            .map(|item| item.trim())
            .collect::<Vec<_>>();
        if cols.len() < 4 {
            continue;
        }
        let Ok(pid) = cols[1].parse::<u32>() else {
            continue;
        };
        processes.push(NvidiaGpuProcessSample {
            gpu_uuid: cols[0].to_string(),
            pid,
            process_name: cols[2].to_string(),
            memory_used_bytes: parse_optional_metric_u64(cols[3]).map(|mb| mb * 1024 * 1024),
        });
    }

    Ok(processes)
}

#[cfg(target_os = "linux")]
fn linux_process_belongs_to_container(pid: u32, container_id: &str, short_id: &str) -> bool {
    let path = PathBuf::from("/proc").join(pid.to_string()).join("cgroup");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    raw.lines()
        .any(|line| line.contains(container_id) || line.contains(short_id))
}

#[cfg(not(target_os = "linux"))]
fn linux_process_belongs_to_container(_pid: u32, _container_id: &str, _short_id: &str) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn query_macos_gpu_status() -> Result<GpuStatusDto, String> {
    let output = runtime_setup_run("system_profiler", &["SPDisplaysDataType", "-json"])?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("system_profiler exited with status {}", output.status)
        } else {
            detail
        });
    }

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("Invalid GPU JSON: {}", e))?;
    let items = value
        .get("SPDisplaysDataType")
        .and_then(|entry| entry.as_array())
        .cloned()
        .unwrap_or_default();

    let mut devices = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let name = obj
            .get("sppci_model")
            .and_then(|entry| entry.as_str())
            .or_else(|| obj.get("_name").and_then(|entry| entry.as_str()))
            .or_else(|| {
                obj.get("spdisplays_vendor")
                    .and_then(|entry| entry.as_str())
            })
            .unwrap_or("GPU")
            .trim()
            .to_string();
        let memory_total_human = obj
            .get("spdisplays_vram")
            .and_then(|entry| entry.as_str())
            .or_else(|| {
                obj.get("spdisplays_vram_shared")
                    .and_then(|entry| entry.as_str())
            })
            .map(|entry| entry.trim().to_string());
        devices.push(GpuDeviceDto {
            index: index as u32,
            name,
            utilization_percent: None,
            memory_used_bytes: None,
            memory_total_bytes: None,
            memory_used_human: None,
            memory_total_human,
            temperature_celsius: None,
            power_watts: None,
        });
    }

    if devices.is_empty() {
        return Ok(gpu_status_unavailable(
            "No GPU devices were detected by system_profiler.",
        ));
    }

    Ok(GpuStatusDto {
        available: true,
        utilization_supported: false,
        backend: "system_profiler".to_string(),
        message: "GPU devices are detected, but live utilization is unavailable on this platform."
            .to_string(),
        devices,
    })
}

#[tauri::command]
async fn gpu_status() -> Result<GpuStatusDto, String> {
    tokio::task::spawn_blocking(|| {
        if runtime_setup_command_exists("nvidia-smi") {
            return query_nvidia_gpu_status().or_else(|error| {
                Ok(gpu_status_unavailable(format!(
                    "GPU telemetry probe failed: {}",
                    error
                )))
            });
        }

        #[cfg(target_os = "macos")]
        {
            if runtime_setup_command_exists("system_profiler") {
                return query_macos_gpu_status().or_else(|error| {
                    Ok(gpu_status_unavailable(format!(
                        "GPU telemetry probe failed: {}",
                        error
                    )))
                });
            }
        }

        Ok(gpu_status_unavailable(
            "GPU telemetry is unavailable on this machine. Install NVIDIA tooling or use a supported local runtime.",
        ))
    })
    .await
    .map_err(|e| format!("gpu_status task failed: {}", e))?
}

async fn ollama_check_installed() -> bool {
    tokio::task::spawn_blocking(|| Command::new("ollama").arg("--version").output().is_ok())
        .await
        .unwrap_or(false)
}

async fn ollama_check_running() -> Result<String, String> {
    let client = ollama_http_client()?;
    let base_url = ollama_base_url();
    let url = format!("{}/api/version", base_url.trim_end_matches('/'));
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "ollama version endpoint returned {}",
            resp.status()
        ));
    }
    let body: OllamaVersionResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body.version.unwrap_or_default())
}

#[tauri::command]
async fn ollama_status() -> Result<OllamaStatusDto, String> {
    let installed = ollama_check_installed().await;
    let base_url = ollama_base_url();
    match ollama_check_running().await {
        Ok(version) => Ok(OllamaStatusDto {
            installed,
            running: true,
            version,
            base_url,
        }),
        Err(_) => Ok(OllamaStatusDto {
            installed,
            running: false,
            version: String::new(),
            base_url,
        }),
    }
}

#[tauri::command]
async fn ollama_list_models() -> Result<Vec<OllamaModelDto>, String> {
    let client = ollama_http_client()?;
    let base_url = ollama_base_url();
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("ollama tags endpoint returned {}", resp.status()));
    }
    let body: OllamaTagsResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut out = body
        .models
        .into_iter()
        .map(|m| {
            let size = m.size;
            OllamaModelDto {
                name: m.name,
                size_bytes: size,
                size_human: format_bytes_human(size),
                modified_at: m.modified_at,
                digest: m.digest,
                family: m.details.family,
                parameter_size: m.details.parameter_size,
                quantization_level: m.details.quantization_level,
            }
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct AiHubActionResultDto {
    ok: bool,
    message: String,
}

#[derive(Debug, Serialize)]
pub struct OllamaStorageInfoDto {
    path: String,
    exists: bool,
    model_count: usize,
    total_size_bytes: u64,
    total_size_human: String,
}

fn ollama_models_path() -> PathBuf {
    if let Ok(value) = std::env::var("OLLAMA_MODELS") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    PathBuf::from(home).join(".ollama").join("models")
}

async fn run_local_cli(command: &str, args: Vec<String>) -> Result<String, String> {
    let cmd = command.to_string();
    tokio::task::spawn_blocking(move || {
        let output = Command::new(&cmd)
            .args(&args)
            .env("PATH", runtime_setup_path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if output.status.success() {
            Ok(if stdout.is_empty() { stderr } else { stdout })
        } else {
            let detail = if stderr.is_empty() { stdout } else { stderr };
            Err(if detail.is_empty() {
                format!("{} exited with status {}", cmd, output.status)
            } else {
                detail
            })
        }
    })
    .await
    .map_err(|e| format!("{} task failed: {}", command, e))?
}

#[tauri::command]
async fn ollama_storage_info() -> Result<OllamaStorageInfoDto, String> {
    let models = ollama_list_models().await.unwrap_or_default();
    let path = ollama_models_path();
    let total_size_bytes = models.iter().map(|item| item.size_bytes).sum::<u64>();
    Ok(OllamaStorageInfoDto {
        path: path.display().to_string(),
        exists: path.exists(),
        model_count: models.len(),
        total_size_bytes,
        total_size_human: format_bytes_human(total_size_bytes),
    })
}

#[tauri::command]
async fn ollama_pull_model(name: String) -> Result<AiHubActionResultDto, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Model name is required".to_string());
    }
    let output = run_local_cli("ollama", vec!["pull".to_string(), trimmed.to_string()]).await?;
    Ok(AiHubActionResultDto {
        ok: true,
        message: if output.is_empty() {
            format!("Pulled model {}", trimmed)
        } else {
            output
        },
    })
}

#[tauri::command]
async fn ollama_delete_model(name: String) -> Result<AiHubActionResultDto, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Model name is required".to_string());
    }
    let output = run_local_cli("ollama", vec!["rm".to_string(), trimmed.to_string()]).await?;
    Ok(AiHubActionResultDto {
        ok: true,
        message: if output.is_empty() {
            format!("Removed model {}", trimmed)
        } else {
            output
        },
    })
}

// ── Agent sandboxes (MVP) ──────────────────────────────────────────

// ── AI settings commands ───────────────────────────────────────────

const AI_SECRET_SERVICE: &str = "com.cratebay.app.ai";
const AI_SETTINGS_SCHEMA_VERSION: u32 = 1;
static AI_REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);
static SANDBOX_SEQ: AtomicU64 = AtomicU64::new(1);

fn default_true() -> bool {
    true
}

fn default_skill_input_schema() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderProfile {
    id: String,
    provider_id: String,
    display_name: String,
    model: String,
    base_url: String,
    api_key_ref: String,
    #[serde(default)]
    headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSecurityPolicy {
    destructive_action_confirmation: bool,
    mcp_remote_enabled: bool,
    #[serde(default)]
    mcp_allowed_actions: Vec<String>,
    #[serde(default)]
    mcp_auth_token_ref: String,
    #[serde(default = "default_true")]
    mcp_audit_enabled: bool,
    #[serde(default)]
    cli_command_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSkillDefinition {
    id: String,
    display_name: String,
    description: String,
    #[serde(default)]
    tags: Vec<String>,
    executor: String,
    target: String,
    #[serde(default = "default_skill_input_schema")]
    input_schema: serde_json::Value,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    id: String,
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: Vec<String>,
    #[serde(default)]
    working_dir: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    notes: String,
}

fn default_opensandbox_base_url() -> String {
    "http://127.0.0.1:8080".to_string()
}

fn default_opensandbox_config_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
    format!("{}/.cratebay/opensandbox.toml", home)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSandboxConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_opensandbox_base_url")]
    base_url: String,
    #[serde(default = "default_opensandbox_config_path")]
    config_path: String,
}

impl Default for OpenSandboxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_opensandbox_base_url(),
            config_path: default_opensandbox_config_path(),
        }
    }
}

impl Default for AiSecurityPolicy {
    fn default() -> Self {
        Self {
            destructive_action_confirmation: true,
            mcp_remote_enabled: false,
            mcp_allowed_actions: vec![
                "list_containers".to_string(),
                "vm_list".to_string(),
                "k8s_list_pods".to_string(),
            ],
            mcp_auth_token_ref: "MCP_AUTH_TOKEN".to_string(),
            mcp_audit_enabled: true,
            cli_command_allowlist: vec![
                "codex".to_string(),
                "claude".to_string(),
                "openclaw".to_string(),
                "gemini".to_string(),
                "qwen".to_string(),
                "aider".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    #[serde(default)]
    schema_version: u32,
    #[serde(default = "default_ai_profiles")]
    profiles: Vec<AiProviderProfile>,
    #[serde(default)]
    active_profile_id: String,
    #[serde(default = "default_ai_skills")]
    skills: Vec<AiSkillDefinition>,
    #[serde(default)]
    security_policy: AiSecurityPolicy,
    #[serde(default)]
    mcp_servers: Vec<McpServerEntry>,
    #[serde(default)]
    opensandbox: OpenSandboxConfig,
}

impl Default for AiSettings {
    fn default() -> Self {
        default_ai_settings()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProfileValidationResult {
    ok: bool,
    message: String,
}

fn ai_profile(
    id: &str,
    provider_id: &str,
    display_name: &str,
    model: &str,
    base_url: &str,
    api_key_ref: &str,
) -> AiProviderProfile {
    AiProviderProfile {
        id: id.to_string(),
        provider_id: provider_id.to_string(),
        display_name: display_name.to_string(),
        model: model.to_string(),
        base_url: base_url.to_string(),
        api_key_ref: api_key_ref.to_string(),
        headers: HashMap::new(),
    }
}

fn default_ai_profiles() -> Vec<AiProviderProfile> {
    vec![
        ai_profile(
            "openai-default",
            "openai",
            "OpenAI (GPT-4.1-mini)",
            "gpt-4.1-mini",
            "https://api.openai.com/v1",
            "OPENAI_API_KEY",
        ),
        ai_profile(
            "anthropic-default",
            "anthropic",
            "Anthropic (Claude 3.7 Sonnet)",
            "claude-3-7-sonnet-latest",
            "https://api.anthropic.com/v1",
            "ANTHROPIC_API_KEY",
        ),
        ai_profile(
            "gemini-default",
            "gemini",
            "Gemini (2.5 Pro)",
            "gemini-2.5-pro",
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "GEMINI_API_KEY",
        ),
        ai_profile(
            "openrouter-default",
            "openrouter",
            "OpenRouter (GPT-4.1-mini)",
            "openai/gpt-4.1-mini",
            "https://openrouter.ai/api/v1",
            "OPENROUTER_API_KEY",
        ),
        ai_profile(
            "deepseek-default",
            "deepseek",
            "DeepSeek (chat)",
            "deepseek-chat",
            "https://api.deepseek.com/v1",
            "DEEPSEEK_API_KEY",
        ),
        ai_profile(
            "minimax-default",
            "minimax",
            "MiniMax (Text-01)",
            "MiniMax-Text-01",
            "https://api.minimax.chat/v1",
            "MINIMAX_API_KEY",
        ),
        ai_profile(
            "kimi-default",
            "kimi",
            "Kimi (Moonshot)",
            "moonshot-v1-8k",
            "https://api.moonshot.cn/v1",
            "KIMI_API_KEY",
        ),
        ai_profile(
            "glm-default",
            "glm",
            "GLM (4 Plus)",
            "glm-4-plus",
            "https://open.bigmodel.cn/api/paas/v4",
            "GLM_API_KEY",
        ),
        ai_profile(
            "ollama-default",
            "ollama",
            "Ollama Local",
            "qwen2.5:7b",
            "http://127.0.0.1:11434/v1",
            "",
        ),
        ai_profile(
            "custom-default",
            "custom",
            "Custom Provider",
            "model-name",
            "https://api.example.com/v1",
            "CUSTOM_LLM_API_KEY",
        ),
    ]
}

fn ai_skill(
    id: &str,
    display_name: &str,
    description: &str,
    tags: &[&str],
    executor: &str,
    target: &str,
    input_schema: serde_json::Value,
) -> AiSkillDefinition {
    AiSkillDefinition {
        id: id.to_string(),
        display_name: display_name.to_string(),
        description: description.to_string(),
        tags: tags.iter().map(|item| item.to_string()).collect(),
        executor: executor.to_string(),
        target: target.to_string(),
        input_schema,
        enabled: true,
    }
}

fn default_ai_skills() -> Vec<AiSkillDefinition> {
    vec![
        ai_skill(
            "assistant-container-diagnose",
            "Container Diagnose",
            "Run safe assistant read flow for container status and baseline diagnosis.",
            &["assistant", "containers", "read"],
            "assistant_step",
            "list_containers",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        ),
        ai_skill(
            "mcp-k8s-pods-read",
            "Kubernetes Pods Read",
            "Use MCP allowlisted action to fetch pod status for troubleshooting.",
            &["mcp", "k8s", "read"],
            "mcp_action",
            "k8s_list_pods",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "namespace": { "type": "string" }
                },
                "additionalProperties": false
            }),
        ),
        ai_skill(
            "managed-sandbox-list",
            "Managed Sandbox List",
            "List CrateBay-managed sandboxes and their current lifecycle state.",
            &["sandbox", "managed", "read"],
            "assistant_step",
            "sandbox_list",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        ),
        ai_skill(
            "managed-sandbox-command",
            "Managed Sandbox Command",
            "Run a command inside a CrateBay-managed sandbox.",
            &["sandbox", "managed", "command"],
            "sandbox_action",
            "sandbox_exec",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "command": { "type": "string", "minLength": 1 }
                },
                "required": ["id", "command"],
                "additionalProperties": false
            }),
        ),
        ai_skill(
            "agent-cli-codex-prompt",
            "Codex CLI Prompt",
            "Invoke the Codex CLI preset directly from the skills runtime.",
            &["agent-cli", "codex", "prompting"],
            "agent_cli_preset",
            "codex",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "minLength": 1 }
                },
                "required": ["prompt"],
                "additionalProperties": false
            }),
        ),
        ai_skill(
            "agent-cli-claude-prompt",
            "Claude Code Prompt",
            "Invoke the Claude Code CLI preset directly from the skills runtime.",
            &["agent-cli", "claude", "prompting"],
            "agent_cli_preset",
            "claude",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "minLength": 1 }
                },
                "required": ["prompt"],
                "additionalProperties": false
            }),
        ),
        ai_skill(
            "agent-cli-openclaw-plan",
            "OpenClaw CLI Plan",
            "Invoke OpenClaw CLI preset to generate multi-step task plans.",
            &["agent-cli", "openclaw", "planning"],
            "agent_cli_preset",
            "openclaw",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "minLength": 1 }
                },
                "required": ["prompt"],
                "additionalProperties": false
            }),
        ),
    ]
}

fn default_ai_settings() -> AiSettings {
    let profiles = default_ai_profiles();
    let active_profile_id = profiles
        .first()
        .map(|p| p.id.clone())
        .unwrap_or_else(|| "openai-default".to_string());
    AiSettings {
        schema_version: AI_SETTINGS_SCHEMA_VERSION,
        profiles,
        active_profile_id,
        skills: default_ai_skills(),
        security_policy: AiSecurityPolicy::default(),
        mcp_servers: Vec::new(),
        opensandbox: OpenSandboxConfig::default(),
    }
}

fn ai_settings_path() -> PathBuf {
    cratebay_core::config_dir().join("ai-settings.json")
}

fn validate_ai_profile_inner(profile: &AiProviderProfile) -> AiProfileValidationResult {
    if profile.id.trim().is_empty() {
        return AiProfileValidationResult {
            ok: false,
            message: "Profile id is required".to_string(),
        };
    }
    if profile.provider_id.trim().is_empty() {
        return AiProfileValidationResult {
            ok: false,
            message: "Provider id is required".to_string(),
        };
    }
    if profile.display_name.trim().is_empty() {
        return AiProfileValidationResult {
            ok: false,
            message: "Display name is required".to_string(),
        };
    }
    if profile.model.trim().is_empty() {
        return AiProfileValidationResult {
            ok: false,
            message: "Model is required".to_string(),
        };
    }

    let base_url = profile.base_url.trim();
    if base_url.is_empty() {
        return AiProfileValidationResult {
            ok: false,
            message: "Base URL is required".to_string(),
        };
    }
    if !(base_url.starts_with("https://") || base_url.starts_with("http://")) {
        return AiProfileValidationResult {
            ok: false,
            message: "Base URL must start with http:// or https://".to_string(),
        };
    }

    for (key, value) in &profile.headers {
        if key.trim().is_empty() {
            return AiProfileValidationResult {
                ok: false,
                message: "Header key cannot be empty".to_string(),
            };
        }
        if value.trim().is_empty() {
            return AiProfileValidationResult {
                ok: false,
                message: format!("Header value for '{}' cannot be empty", key),
            };
        }
    }

    let base_lower = base_url.to_ascii_lowercase();
    let is_local_endpoint = base_lower.contains("127.0.0.1") || base_lower.contains("localhost");
    if !is_local_endpoint && profile.api_key_ref.trim().is_empty() {
        return AiProfileValidationResult {
            ok: false,
            message: "API key reference is required for non-local endpoints".to_string(),
        };
    }

    AiProfileValidationResult {
        ok: true,
        message: "Profile is valid".to_string(),
    }
}

fn normalize_ai_settings(mut settings: AiSettings) -> AiSettings {
    settings.schema_version = settings.schema_version.max(AI_SETTINGS_SCHEMA_VERSION);

    if settings.profiles.is_empty() {
        return default_ai_settings();
    }

    let mut seen = std::collections::HashSet::new();
    settings.profiles.retain(|profile| {
        let id = profile.id.trim();
        !id.is_empty() && seen.insert(id.to_string())
    });

    if settings.profiles.is_empty() {
        return default_ai_settings();
    }

    if !settings
        .profiles
        .iter()
        .any(|p| p.id == settings.active_profile_id)
    {
        if let Some(profile) = settings.profiles.first() {
            settings.active_profile_id = profile.id.clone();
        }
    }

    let mut skill_seen = std::collections::HashSet::new();
    settings.skills.retain(|skill| {
        let id = skill.id.trim();
        !id.is_empty() && skill_seen.insert(id.to_string())
    });
    for skill in &mut settings.skills {
        skill.id = skill.id.trim().to_string();
        skill.display_name = skill.display_name.trim().to_string();
        skill.description = skill.description.trim().to_string();
        skill.executor = skill.executor.trim().to_string();
        skill.target = skill.target.trim().to_string();
        skill.tags.retain(|tag| !tag.trim().is_empty());
        if skill.display_name.is_empty() {
            skill.display_name = skill.id.clone();
        }
        if skill.description.is_empty() {
            skill.description = "Skill runtime entry".to_string();
        }
        if skill.executor.is_empty() {
            skill.executor = "assistant_step".to_string();
        }
        if skill.input_schema.is_null() {
            skill.input_schema = default_skill_input_schema();
        }
    }
    if settings.skills.is_empty() {
        settings.skills = default_ai_skills();
    } else {
        let mut existing_ids = settings
            .skills
            .iter()
            .map(|skill| skill.id.clone())
            .collect::<std::collections::HashSet<_>>();
        for default_skill in default_ai_skills() {
            if existing_ids.insert(default_skill.id.clone()) {
                settings.skills.push(default_skill);
            }
        }
    }

    let mut mcp_seen = std::collections::HashSet::new();
    settings.mcp_servers.retain(|server| {
        let id = server.id.trim();
        !id.is_empty() && mcp_seen.insert(id.to_string())
    });
    for server in &mut settings.mcp_servers {
        server.id = server.id.trim().to_string();
        server.name = server.name.trim().to_string();
        server.command = server.command.trim().to_string();
        server.working_dir = server.working_dir.trim().to_string();
        server.notes = server.notes.trim().to_string();
        server.args.retain(|arg| !arg.trim().is_empty());
        if server.name.is_empty() {
            server.name = server.id.clone();
        }
    }

    settings.opensandbox.base_url = settings.opensandbox.base_url.trim().to_string();
    if settings.opensandbox.base_url.is_empty() {
        settings.opensandbox.base_url = default_opensandbox_base_url();
    }
    settings.opensandbox.config_path = settings.opensandbox.config_path.trim().to_string();
    if settings.opensandbox.config_path.is_empty() {
        settings.opensandbox.config_path = default_opensandbox_config_path();
    }

    settings
}

fn persist_ai_settings(path: &Path, settings: &AiSettings) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("Invalid settings path: {}", path.display()));
    };
    std::fs::create_dir_all(parent).map_err(|e| {
        format!(
            "Failed to create config directory {}: {}",
            parent.display(),
            e
        )
    })?;

    let body = serde_json::to_vec_pretty(settings)
        .map_err(|e| format!("Failed to encode settings: {}", e))?;
    std::fs::write(path, body)
        .map_err(|e| format!("Failed to write settings file {}: {}", path.display(), e))
}

#[tauri::command]
fn load_ai_settings() -> Result<AiSettings, String> {
    let path = ai_settings_path();
    if !path.exists() {
        let defaults = default_ai_settings();
        persist_ai_settings(&path, &defaults)?;
        return Ok(defaults);
    }

    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read settings file {}: {}", path.display(), e))?;
    let parsed: AiSettings = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                path = %path.display(),
                error = %e,
                "Failed to parse AI settings JSON; backing up and resetting to defaults"
            );
            let backup_path = path.with_file_name(format!(
                "ai-settings.broken-{}.json",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            ));
            if let Err(backup_err) = std::fs::write(&backup_path, raw.as_bytes()) {
                warn!(
                    backup = %backup_path.display(),
                    error = %backup_err,
                    "Failed to write backup AI settings file"
                );
            }

            let defaults = default_ai_settings();
            persist_ai_settings(&path, &defaults)?;
            return Ok(defaults);
        }
    };
    let original_version = parsed.schema_version;
    let normalized = normalize_ai_settings(parsed);
    if original_version < AI_SETTINGS_SCHEMA_VERSION {
        persist_ai_settings(&path, &normalized)?;
    }
    Ok(normalized)
}

#[tauri::command]
fn save_ai_settings(settings: AiSettings) -> Result<AiSettings, String> {
    let normalized = normalize_ai_settings(settings);
    for profile in &normalized.profiles {
        let result = validate_ai_profile_inner(profile);
        if !result.ok {
            return Err(format!(
                "Profile '{}' validation failed: {}",
                profile.display_name, result.message
            ));
        }
    }

    for server in &normalized.mcp_servers {
        if server.id.trim().is_empty() {
            return Err("MCP server id is required".to_string());
        }
        if server.command.trim().is_empty() {
            return Err(format!("MCP server '{}' command is required", server.id));
        }
        sandbox_normalize_env(Some(server.env.clone()))?;
    }

    let path = ai_settings_path();
    persist_ai_settings(&path, &normalized)?;
    Ok(normalized)
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerStatusDto {
    id: String,
    name: String,
    command: String,
    args: Vec<String>,
    enabled: bool,
    running: bool,
    status: String,
    pid: Option<u32>,
    started_at: String,
    exit_code: Option<i32>,
    notes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenSandboxStatusDto {
    installed: bool,
    enabled: bool,
    configured: bool,
    reachable: bool,
    base_url: String,
    config_path: String,
}

fn mcp_runtime_logs() -> Arc<Mutex<VecDeque<String>>> {
    Arc::new(Mutex::new(VecDeque::new()))
}

fn mcp_push_log(logs: &Arc<Mutex<VecDeque<String>>>, line: String) {
    if let Ok(mut guard) = logs.lock() {
        guard.push_back(format!(
            "{} {}",
            chrono::Local::now().format("%H:%M:%S"),
            line
        ));
        while guard.len() > 400 {
            guard.pop_front();
        }
    }
}

fn mcp_env_map(entries: &[String]) -> HashMap<String, String> {
    entries
        .iter()
        .filter_map(|item| item.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn mcp_refresh_runtime(runtime: &mut McpServerRuntime) {
    if let Some(child) = runtime.child.as_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                runtime.exit_code = status.code();
                runtime.child = None;
                mcp_push_log(
                    &runtime.logs,
                    format!("[runtime] process exited with {:?}", runtime.exit_code),
                );
            }
            Ok(None) => {}
            Err(err) => {
                mcp_push_log(
                    &runtime.logs,
                    format!("[runtime] status check failed: {}", err),
                );
            }
        }
    }
}

fn mcp_spawn_log_reader<R: std::io::Read + Send + 'static>(
    logs: Arc<Mutex<VecDeque<String>>>,
    stream_name: &'static str,
    reader: R,
) {
    std::thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines() {
            match line {
                Ok(line) => mcp_push_log(&logs, format!("[{}] {}", stream_name, line)),
                Err(err) => {
                    mcp_push_log(&logs, format!("[{}] read error: {}", stream_name, err));
                    break;
                }
            }
        }
    });
}

fn mcp_runtime_status_from_settings(
    state: &AppState,
    settings: &AiSettings,
) -> Vec<McpServerStatusDto> {
    let mut runtimes = state.mcp_runtimes.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = Vec::new();
    for server in &settings.mcp_servers {
        let runtime = runtimes
            .entry(server.id.clone())
            .or_insert_with(|| McpServerRuntime {
                child: None,
                logs: mcp_runtime_logs(),
                started_at: None,
                exit_code: None,
            });
        mcp_refresh_runtime(runtime);
        let pid = runtime.child.as_ref().map(|child| child.id());
        let running = runtime.child.is_some();
        out.push(McpServerStatusDto {
            id: server.id.clone(),
            name: server.name.clone(),
            command: server.command.clone(),
            args: server.args.clone(),
            enabled: server.enabled,
            running,
            status: if running {
                "running".to_string()
            } else if runtime.exit_code.is_some() {
                "exited".to_string()
            } else {
                "stopped".to_string()
            },
            pid,
            started_at: runtime.started_at.clone().unwrap_or_default(),
            exit_code: runtime.exit_code,
            notes: server.notes.clone(),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn mcp_list_servers_inner(state: &AppState) -> Result<Vec<McpServerStatusDto>, String> {
    let settings = load_ai_settings()?;
    Ok(mcp_runtime_status_from_settings(state, &settings))
}

#[tauri::command]
fn mcp_list_servers(state: State<'_, AppState>) -> Result<Vec<McpServerStatusDto>, String> {
    mcp_list_servers_inner(state.inner())
}

#[tauri::command]
fn mcp_save_servers(servers: Vec<McpServerEntry>) -> Result<Vec<McpServerEntry>, String> {
    let mut settings = load_ai_settings()?;
    settings.mcp_servers = servers;
    let saved = save_ai_settings(settings)?;
    Ok(saved.mcp_servers)
}

async fn mcp_start_server_inner(
    state: &AppState,
    id: String,
) -> Result<AiHubActionResultDto, String> {
    let settings = load_ai_settings()?;
    let server = settings
        .mcp_servers
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("Unknown MCP server '{}'", id))?;
    if !server.enabled {
        return Err(format!("MCP server '{}' is disabled", server.name));
    }
    sandbox_normalize_env(Some(server.env.clone()))?;

    let mut runtimes = state.mcp_runtimes.lock().unwrap_or_else(|e| e.into_inner());
    let runtime = runtimes
        .entry(server.id.clone())
        .or_insert_with(|| McpServerRuntime {
            child: None,
            logs: mcp_runtime_logs(),
            started_at: None,
            exit_code: None,
        });
    mcp_refresh_runtime(runtime);
    if runtime.child.is_some() {
        return Ok(AiHubActionResultDto {
            ok: true,
            message: format!("{} is already running", server.name),
        });
    }

    let mut command = Command::new(&server.command);
    command.args(&server.args);
    command.env("PATH", runtime_setup_path());
    for (key, value) in mcp_env_map(&server.env) {
        command.env(key, value);
    }
    if !server.working_dir.trim().is_empty() {
        command.current_dir(server.working_dir.trim());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start MCP server {}: {}", server.name, e))?;
    let pid = child.id();
    if let Some(stdout) = child.stdout.take() {
        mcp_spawn_log_reader(runtime.logs.clone(), "stdout", stdout);
    }
    if let Some(stderr) = child.stderr.take() {
        mcp_spawn_log_reader(runtime.logs.clone(), "stderr", stderr);
    }
    runtime.started_at = Some(chrono::Utc::now().to_rfc3339());
    runtime.exit_code = None;
    runtime.child = Some(child);
    mcp_push_log(
        &runtime.logs,
        format!(
            "[runtime] started pid={} cmd={} {}",
            pid,
            server.command,
            server.args.join(" ")
        ),
    );
    Ok(AiHubActionResultDto {
        ok: true,
        message: format!("Started {} (pid {})", server.name, pid),
    })
}

#[tauri::command]
async fn mcp_start_server(
    state: State<'_, AppState>,
    id: String,
) -> Result<AiHubActionResultDto, String> {
    mcp_start_server_inner(state.inner(), id).await
}

async fn mcp_stop_server_inner(
    state: &AppState,
    id: String,
) -> Result<AiHubActionResultDto, String> {
    let mut runtimes = state.mcp_runtimes.lock().unwrap_or_else(|e| e.into_inner());
    let Some(runtime) = runtimes.get_mut(&id) else {
        return Ok(AiHubActionResultDto {
            ok: true,
            message: format!("MCP server {} is not running", id),
        });
    };
    mcp_refresh_runtime(runtime);
    let Some(mut child) = runtime.child.take() else {
        return Ok(AiHubActionResultDto {
            ok: true,
            message: format!("MCP server {} is already stopped", id),
        });
    };
    child
        .kill()
        .map_err(|e| format!("Failed to stop MCP server {}: {}", id, e))?;
    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for MCP server {}: {}", id, e))?;
    runtime.exit_code = status.code();
    mcp_push_log(
        &runtime.logs,
        format!("[runtime] stopped with {:?}", runtime.exit_code),
    );
    Ok(AiHubActionResultDto {
        ok: true,
        message: format!("Stopped MCP server {}", id),
    })
}

#[tauri::command]
async fn mcp_stop_server(
    state: State<'_, AppState>,
    id: String,
) -> Result<AiHubActionResultDto, String> {
    mcp_stop_server_inner(state.inner(), id).await
}

fn mcp_server_logs_inner(
    state: &AppState,
    id: String,
    limit: Option<usize>,
) -> Result<Vec<String>, String> {
    let runtimes = state.mcp_runtimes.lock().unwrap_or_else(|e| e.into_inner());
    let Some(runtime) = runtimes.get(&id) else {
        return Ok(Vec::new());
    };
    let logs = runtime.logs.lock().unwrap_or_else(|e| e.into_inner());
    let limit = limit.unwrap_or(80).clamp(1, 400);
    let len = logs.len();
    let start = len.saturating_sub(limit);
    Ok(logs.iter().skip(start).cloned().collect())
}

#[tauri::command]
fn mcp_server_logs(
    state: State<'_, AppState>,
    id: String,
    limit: Option<usize>,
) -> Result<Vec<String>, String> {
    mcp_server_logs_inner(state.inner(), id, limit)
}

#[tauri::command]
fn mcp_export_client_config(client: String) -> Result<String, String> {
    let normalized_client = client.trim().to_ascii_lowercase();
    if !matches!(normalized_client.as_str(), "codex" | "claude" | "cursor") {
        return Err(format!("Unsupported MCP client '{}'", client));
    }
    let settings = load_ai_settings()?;
    let mut enabled = settings
        .mcp_servers
        .into_iter()
        .filter(|item| item.enabled)
        .collect::<Vec<_>>();
    enabled.sort_by(|a, b| a.id.cmp(&b.id));

    match normalized_client.as_str() {
        "claude" | "cursor" => {
            let mut servers = serde_json::Map::new();
            for server in enabled {
                let mut entry = serde_json::Map::new();
                entry.insert(
                    "command".to_string(),
                    serde_json::Value::String(server.command),
                );
                entry.insert(
                    "args".to_string(),
                    serde_json::to_value(server.args).map_err(|e| e.to_string())?,
                );

                let env_map = mcp_env_map(&server.env);
                entry.insert(
                    "env".to_string(),
                    serde_json::to_value(env_map).map_err(|e| e.to_string())?,
                );
                if !server.working_dir.trim().is_empty() {
                    entry.insert(
                        "cwd".to_string(),
                        serde_json::Value::String(server.working_dir.trim().to_string()),
                    );
                }

                servers.insert(server.id, serde_json::Value::Object(entry));
            }

            serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": servers }))
                .map_err(|e| format!("Failed to encode MCP export config: {}", e))
        }
        "codex" => export_codex_mcp_config(&enabled),
        _ => unreachable!("client validated"),
    }
}

fn export_codex_mcp_config(servers: &[McpServerEntry]) -> Result<String, String> {
    if servers.is_empty() {
        return Ok(
            "# No enabled MCP servers in CrateBay.\n# Add one in AI Hub → MCP, then export again.\n"
                .to_string(),
        );
    }

    fn toml_quote(value: &str) -> String {
        let mut out = String::with_capacity(value.len() + 2);
        out.push('"');
        for ch in value.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ => out.push(ch),
            }
        }
        out.push('"');
        out
    }

    fn toml_key_segment(key: &str) -> String {
        let is_bare = !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if is_bare {
            key.to_string()
        } else {
            toml_quote(key)
        }
    }

    fn toml_string_array(values: &[String]) -> String {
        let items = values
            .iter()
            .map(|value| toml_quote(value))
            .collect::<Vec<_>>();
        format!("[{}]", items.join(", "))
    }

    let mut out = String::new();
    out.push_str("# Add to ~/.codex/config.toml\n");
    out.push_str("# Docs: docs/MCP_SERVER.md\n\n");

    for server in servers {
        let id_key = toml_key_segment(&server.id);
        out.push_str(&format!("[mcp_servers.{}]\n", id_key));
        out.push_str(&format!("command = {}\n", toml_quote(&server.command)));
        out.push_str(&format!("args = {}\n", toml_string_array(&server.args)));

        if !server.working_dir.trim().is_empty() {
            out.push_str(&format!(
                "cwd = {}\n",
                toml_quote(server.working_dir.trim())
            ));
        }

        let env_map = mcp_env_map(&server.env);
        if !env_map.is_empty() {
            out.push('\n');
            out.push_str(&format!("[mcp_servers.{}.env]\n", id_key));
            let mut env_items = env_map.into_iter().collect::<Vec<_>>();
            env_items.sort_by(|a, b| a.0.cmp(&b.0));
            for (key, value) in env_items {
                out.push_str(&format!(
                    "{} = {}\n",
                    toml_key_segment(&key),
                    toml_quote(&value)
                ));
            }
        }

        out.push('\n');
    }

    Ok(out.trim_end().to_string() + "\n")
}

#[tauri::command]
async fn opensandbox_status() -> Result<OpenSandboxStatusDto, String> {
    let settings = load_ai_settings()?;
    let config = settings.opensandbox;
    let installed = tokio::task::spawn_blocking(|| {
        Command::new("opensandbox-server")
            .arg("--help")
            .env("PATH", runtime_setup_path())
            .output()
            .is_ok()
    })
    .await
    .unwrap_or(false);
    let configured = Path::new(&config.config_path).exists();
    let reachable = if config.base_url.trim().is_empty() {
        false
    } else {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()
            .map_err(|e| format!("Failed to build OpenSandbox client: {}", e))?;
        let url = format!("{}/docs", config.base_url.trim_end_matches('/'));
        match client.get(&url).send().await {
            Ok(resp) => resp.status().is_success() || resp.status().is_redirection(),
            Err(_) => false,
        }
    };
    Ok(OpenSandboxStatusDto {
        installed,
        enabled: config.enabled,
        configured,
        reachable,
        base_url: config.base_url,
        config_path: config.config_path,
    })
}

#[tauri::command]
fn validate_ai_profile(profile: AiProviderProfile) -> AiProfileValidationResult {
    validate_ai_profile_inner(&profile)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolCall {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatResponse {
    request_id: String,
    provider_id: String,
    model: String,
    text: String,
    #[serde(default)]
    usage: Option<AiUsage>,
    #[serde(default)]
    tool_calls: Vec<AiToolCall>,
    #[serde(default)]
    error_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConnectionTestResult {
    ok: bool,
    request_id: String,
    message: String,
    #[serde(default)]
    error_type: Option<String>,
    latency_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantPlanStep {
    id: String,
    title: String,
    command: String,
    args: serde_json::Value,
    risk_level: String,
    requires_confirmation: bool,
    explain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantPlanResult {
    request_id: String,
    strategy: String,
    notes: String,
    fallback_used: bool,
    steps: Vec<AssistantPlanStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpAccessCheckResult {
    allowed: bool,
    request_id: String,
    message: String,
    risk_level: String,
    requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerRuntimeSetupResult {
    ok: bool,
    request_id: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCliPreset {
    id: String,
    name: String,
    description: String,
    command: String,
    args_template: Vec<String>,
    timeout_sec: u64,
    dangerous: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCliRunResult {
    ok: bool,
    request_id: String,
    command_line: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantStepExecutionResult {
    ok: bool,
    request_id: String,
    command: String,
    risk_level: String,
    output: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSkillExecutionResult {
    ok: bool,
    skill_id: String,
    executor: String,
    target: String,
    request_id: String,
    output: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AssistantCommandPolicy {
    risk_level: &'static str,
    always_confirm: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct McpActionPolicy {
    risk_level: &'static str,
    requires_confirmation: bool,
}

fn assistant_command_policy(command: &str) -> Option<AssistantCommandPolicy> {
    if let Some(policy) = sandbox_action_policy(command) {
        return Some(AssistantCommandPolicy {
            risk_level: policy.risk_level,
            always_confirm: policy.requires_confirmation,
        });
    }

    match command {
        "list_containers"
        | "vm_list"
        | "k8s_list_pods"
        | "ollama_list_models"
        | "mcp_list_servers"
        | "mcp_export_client_config" => Some(AssistantCommandPolicy {
            risk_level: "read",
            always_confirm: false,
        }),
        "start_container"
        | "stop_container"
        | "vm_start"
        | "vm_stop"
        | "docker_runtime_quick_setup"
        | "ollama_pull_model"
        | "mcp_start_server"
        | "mcp_stop_server" => Some(AssistantCommandPolicy {
            risk_level: "write",
            always_confirm: false,
        }),
        "remove_container" | "vm_delete" | "ollama_delete_model" => Some(AssistantCommandPolicy {
            risk_level: "destructive",
            always_confirm: true,
        }),
        _ => None,
    }
}

fn mcp_action_has_keyword(action_lower: &str, keyword: &str) -> bool {
    action_lower == keyword
        || action_lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|segment| segment == keyword)
        || action_lower.contains(keyword)
}

fn mcp_action_policy(action: &str) -> McpActionPolicy {
    let action_lower = action.trim().to_ascii_lowercase();
    const DESTRUCTIVE_KEYWORDS: &[&str] = &[
        "delete",
        "remove",
        "destroy",
        "drop",
        "wipe",
        "prune",
        "terminate",
        "kill",
        "uninstall",
        "purge",
    ];
    const WRITE_KEYWORDS: &[&str] = &[
        "create", "apply", "update", "patch", "set", "start", "stop", "restart", "scale", "run",
        "exec", "install",
    ];

    if DESTRUCTIVE_KEYWORDS
        .iter()
        .any(|kw| mcp_action_has_keyword(&action_lower, kw))
    {
        return McpActionPolicy {
            risk_level: "destructive",
            requires_confirmation: true,
        };
    }

    if WRITE_KEYWORDS
        .iter()
        .any(|kw| mcp_action_has_keyword(&action_lower, kw))
    {
        return McpActionPolicy {
            risk_level: "write",
            requires_confirmation: false,
        };
    }

    McpActionPolicy {
        risk_level: "read",
        requires_confirmation: false,
    }
}

fn mcp_confirmation_satisfied(
    action_policy: McpActionPolicy,
    destructive_action_confirmation: bool,
    requires_confirmation: Option<bool>,
    confirmed: Option<bool>,
) -> bool {
    if !destructive_action_confirmation || !action_policy.requires_confirmation {
        return true;
    }

    requires_confirmation.unwrap_or(false) && confirmed.unwrap_or(false)
}

fn assistant_arg_map(
    args: &serde_json::Value,
) -> Result<&serde_json::Map<String, serde_json::Value>, String> {
    args.as_object()
        .ok_or_else(|| "assistant step args must be a JSON object".to_string())
}

fn assistant_arg_string(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("assistant step missing required string arg '{}'", key))
}

fn assistant_arg_optional_string(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match args.get(key) {
        Some(v) if v.is_null() => Ok(None),
        Some(v) => v
            .as_str()
            .map(|s| Some(s.trim().to_string()))
            .ok_or_else(|| format!("assistant step arg '{}' must be a string or null", key)),
        None => Ok(None),
    }
}

fn next_ai_request_id() -> String {
    let seq = AI_REQUEST_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("ai-{}-{}", chrono::Utc::now().timestamp_millis(), seq)
}

fn redact_sensitive(mut text: String) -> String {
    let lower = text.to_ascii_lowercase();
    if lower.contains("authorization") {
        text = text.replace("Authorization", "Authorization[redacted]");
        text = text.replace("authorization", "authorization[redacted]");
    }
    if lower.contains("bearer ") {
        text = text.replace("Bearer ", "Bearer [redacted]");
        text = text.replace("bearer ", "bearer [redacted]");
    }
    if lower.contains("api_key") {
        text = text.replace("api_key", "api_key[redacted]");
    }
    text
}

fn ai_audit_log(action: &str, level: &str, request_id: &str, details: &str) {
    let path = cratebay_core::config_dir()
        .join("audit")
        .join("ai-actions.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let sanitized = cratebay_core::validation::sanitize_log_string(details);
        let redacted = redact_sensitive(sanitized);
        let _ = writeln!(
            file,
            "{} request_id={} action={} level={} {}",
            chrono::Utc::now().to_rfc3339(),
            request_id,
            action,
            level,
            redacted
        );
    }
}

fn file_secret_path(key_ref: &str) -> Option<PathBuf> {
    let base = std::env::var("CRATEBAY_TEST_SECRET_DIR").ok()?;
    let mut name = key_ref
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if name.trim_matches('_').is_empty() {
        name = "secret".to_string();
    }
    Some(PathBuf::from(base).join(format!("{}.secret", name)))
}

fn secret_entry(key_ref: &str) -> Result<Entry, String> {
    Entry::new(AI_SECRET_SERVICE, key_ref)
        .map_err(|e| format!("Failed to create secret entry: {e}"))
}

fn secret_set(key_ref: &str, value: &str) -> Result<(), String> {
    if let Some(path) = file_secret_path(key_ref) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create test secret directory {}: {}",
                    parent.display(),
                    e
                )
            })?;
        }
        std::fs::write(&path, value).map_err(|e| {
            format!(
                "Failed to write test secret '{}' at {}: {}",
                key_ref,
                path.display(),
                e
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path)
                .map_err(|e| format!("Failed to stat test secret '{}': {}", key_ref, e))?
                .permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&path, perms).map_err(|e| {
                format!(
                    "Failed to set permissions on test secret '{}': {}",
                    key_ref, e
                )
            })?;
        }
        return Ok(());
    }

    let entry = secret_entry(key_ref)?;
    entry
        .set_password(value)
        .map_err(|e| format!("Failed to save secret '{}': {}", key_ref, e))
}

fn secret_get(key_ref: &str) -> Result<Option<String>, String> {
    if let Some(path) = file_secret_path(key_ref) {
        if !path.exists() {
            return Ok(None);
        }
        let value = std::fs::read_to_string(&path).map_err(|e| {
            format!(
                "Failed to read test secret '{}' at {}: {}",
                key_ref,
                path.display(),
                e
            )
        })?;
        if value.trim().is_empty() {
            return Ok(None);
        }
        return Ok(Some(value));
    }

    let entry = secret_entry(key_ref)?;
    match entry.get_password() {
        Ok(v) => {
            if v.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(v))
            }
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("NoEntry") || msg.contains("No entry") {
                Ok(None)
            } else {
                Err(format!("Failed to read secret '{}': {}", key_ref, e))
            }
        }
    }
}

fn secret_delete(key_ref: &str) -> Result<(), String> {
    if let Some(path) = file_secret_path(key_ref) {
        match std::fs::remove_file(&path) {
            Ok(_) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(format!(
                    "Failed to delete test secret '{}' at {}: {}",
                    key_ref,
                    path.display(),
                    e
                ))
            }
        }
    }

    let entry = secret_entry(key_ref)?;
    match entry.delete_password() {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("NoEntry") || msg.contains("No entry") {
                Ok(())
            } else {
                Err(format!("Failed to delete secret '{}': {}", key_ref, e))
            }
        }
    }
}

#[tauri::command]
fn ai_secret_set(api_key_ref: String, api_key: String) -> Result<(), String> {
    if api_key_ref.trim().is_empty() {
        return Err("api_key_ref is required".to_string());
    }
    if api_key.trim().is_empty() {
        return Err("api_key is required".to_string());
    }
    let request_id = next_ai_request_id();
    let out = secret_set(api_key_ref.trim(), api_key.trim());
    let status = if out.is_ok() { "ok" } else { "error" };
    ai_audit_log(
        "ai_secret_set",
        "write",
        &request_id,
        &format!("key_ref={} status={}", api_key_ref.trim(), status),
    );
    out
}

#[tauri::command]
fn ai_secret_delete(api_key_ref: String) -> Result<(), String> {
    if api_key_ref.trim().is_empty() {
        return Err("api_key_ref is required".to_string());
    }
    let request_id = next_ai_request_id();
    let out = secret_delete(api_key_ref.trim());
    let status = if out.is_ok() { "ok" } else { "error" };
    ai_audit_log(
        "ai_secret_delete",
        "write",
        &request_id,
        &format!("key_ref={} status={}", api_key_ref.trim(), status),
    );
    out
}

#[tauri::command]
fn ai_secret_exists(api_key_ref: String) -> Result<bool, String> {
    if api_key_ref.trim().is_empty() {
        return Err("api_key_ref is required".to_string());
    }
    let exists = secret_get(api_key_ref.trim())?.is_some();
    Ok(exists)
}

fn resolve_ai_profile(
    settings: &AiSettings,
    profile_id: Option<&str>,
) -> Result<AiProviderProfile, String> {
    if let Some(pid) = profile_id {
        return settings
            .profiles
            .iter()
            .find(|p| p.id == pid)
            .cloned()
            .ok_or_else(|| format!("Profile not found: {}", pid));
    }
    settings
        .profiles
        .iter()
        .find(|p| p.id == settings.active_profile_id)
        .cloned()
        .ok_or_else(|| "Active AI profile not found".to_string())
}

fn classify_provider_error(status: reqwest::StatusCode) -> String {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        "auth_error".to_string()
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        "rate_limit".to_string()
    } else if status.is_client_error() {
        "invalid_request".to_string()
    } else if status.is_server_error() {
        "provider_unavailable".to_string()
    } else {
        "unknown_error".to_string()
    }
}

fn normalized_base_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

fn join_endpoint(base_url: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        normalized_base_url(base_url),
        suffix.trim_start_matches('/')
    )
}

fn parse_openai_text(content: &serde_json::Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(parts) = content.as_array() {
        let mut out = String::new();
        for item in parts {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                out.push_str(text);
            }
        }
        return out;
    }
    String::new()
}

fn parse_openai_tool_calls(value: &serde_json::Value) -> Vec<AiToolCall> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let function = item.get("function")?;
                    let name = function.get("name")?.as_str()?.to_string();
                    let args_value = function
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                        .unwrap_or_else(|| serde_json::json!({}));
                    Some(AiToolCall {
                        name,
                        arguments: args_value,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_usage(value: &serde_json::Value) -> Option<AiUsage> {
    let prompt_tokens = value.get("prompt_tokens").and_then(|v| v.as_u64());
    let completion_tokens = value.get("completion_tokens").and_then(|v| v.as_u64());
    let total_tokens = value.get("total_tokens").and_then(|v| v.as_u64());
    if prompt_tokens.is_none() && completion_tokens.is_none() && total_tokens.is_none() {
        None
    } else {
        Some(AiUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        })
    }
}

async fn call_anthropic(
    client: &reqwest::Client,
    profile: &AiProviderProfile,
    messages: &[AiChatMessage],
    timeout_ms: u64,
    api_key: &str,
    request_id: &str,
) -> Result<AiChatResponse, String> {
    let url = join_endpoint(&profile.base_url, "/messages");
    let body = serde_json::json!({
        "model": profile.model,
        "max_tokens": 512u32,
        "messages": messages.iter().map(|m| {
            serde_json::json!({
                "role": if m.role == "assistant" { "assistant" } else { "user" },
                "content": m.content
            })
        }).collect::<Vec<_>>()
    });

    let mut req = client
        .post(url)
        .timeout(Duration::from_millis(timeout_ms))
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body);

    for (k, v) in &profile.headers {
        req = req.header(k, v);
    }

    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() {
            "network_error: request timeout".to_string()
        } else {
            format!("network_error: {}", e)
        }
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "{}: HTTP {} {}",
            classify_provider_error(status),
            status,
            text
        ));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("provider_unavailable: invalid JSON response: {}", e))?;

    let text = json
        .get("content")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.get("text").and_then(|v| v.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();

    Ok(AiChatResponse {
        request_id: request_id.to_string(),
        provider_id: profile.provider_id.clone(),
        model: profile.model.clone(),
        text,
        usage: json.get("usage").and_then(parse_usage),
        tool_calls: vec![],
        error_type: None,
    })
}

async fn call_openai_compatible(
    client: &reqwest::Client,
    profile: &AiProviderProfile,
    messages: &[AiChatMessage],
    timeout_ms: u64,
    api_key: Option<&str>,
    request_id: &str,
) -> Result<AiChatResponse, String> {
    let url = join_endpoint(&profile.base_url, "/chat/completions");
    let body = serde_json::json!({
        "model": profile.model,
        "stream": false,
        "messages": messages.iter().map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content
            })
        }).collect::<Vec<_>>()
    });

    let mut req = client
        .post(url)
        .timeout(Duration::from_millis(timeout_ms))
        .json(&body);

    if let Some(key) = api_key {
        if !key.trim().is_empty() {
            req = req.bearer_auth(key);
        }
    }
    for (k, v) in &profile.headers {
        req = req.header(k, v);
    }

    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() {
            "network_error: request timeout".to_string()
        } else {
            format!("network_error: {}", e)
        }
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "{}: HTTP {} {}",
            classify_provider_error(status),
            status,
            text
        ));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("provider_unavailable: invalid JSON response: {}", e))?;

    let choice = json
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    let message = choice
        .get("message")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let text = message
        .get("content")
        .map(parse_openai_text)
        .unwrap_or_default();
    let tool_calls = message
        .get("tool_calls")
        .map(parse_openai_tool_calls)
        .unwrap_or_default();

    Ok(AiChatResponse {
        request_id: request_id.to_string(),
        provider_id: profile.provider_id.clone(),
        model: profile.model.clone(),
        text,
        usage: json.get("usage").and_then(parse_usage),
        tool_calls,
        error_type: None,
    })
}

async fn ai_chat_inner(
    profile: &AiProviderProfile,
    messages: &[AiChatMessage],
    timeout_ms: u64,
    request_id: &str,
) -> Result<AiChatResponse, String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("CrateBay-AI/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let api_key = if profile.api_key_ref.trim().is_empty() {
        None
    } else {
        secret_get(profile.api_key_ref.trim())?
    };

    if profile.provider_id != "ollama" && profile.provider_id != "custom" {
        let local_endpoint =
            profile.base_url.contains("127.0.0.1") || profile.base_url.contains("localhost");
        if !local_endpoint && api_key.as_deref().unwrap_or("").trim().is_empty() {
            return Err(format!(
                "auth_error: API key not found in secure store for key ref '{}'",
                profile.api_key_ref
            ));
        }
    }

    if profile.provider_id == "anthropic" {
        let key = api_key.unwrap_or_default();
        if key.trim().is_empty() {
            return Err("auth_error: anthropic profile requires API key".to_string());
        }
        call_anthropic(&client, profile, messages, timeout_ms, &key, request_id).await
    } else {
        call_openai_compatible(
            &client,
            profile,
            messages,
            timeout_ms,
            api_key.as_deref(),
            request_id,
        )
        .await
    }
}

#[tauri::command]
async fn ai_chat(
    profile_id: Option<String>,
    messages: Vec<AiChatMessage>,
    timeout_ms: Option<u64>,
) -> Result<AiChatResponse, String> {
    if messages.is_empty() {
        return Err("messages cannot be empty".to_string());
    }
    let settings = load_ai_settings()?;
    let profile = resolve_ai_profile(&settings, profile_id.as_deref())?;
    let request_id = next_ai_request_id();
    let out = ai_chat_inner(
        &profile,
        &messages,
        timeout_ms.unwrap_or(30_000),
        &request_id,
    )
    .await;
    let level = if out.is_ok() { "read" } else { "error" };
    ai_audit_log(
        "ai_chat",
        level,
        &request_id,
        &format!(
            "provider={} profile={} messages={}",
            profile.provider_id,
            profile.id,
            messages.len()
        ),
    );
    out
}

#[tauri::command]
async fn ai_test_connection(
    profile_id: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<AiConnectionTestResult, String> {
    let settings = load_ai_settings()?;
    let profile = resolve_ai_profile(&settings, profile_id.as_deref())?;
    let request_id = next_ai_request_id();
    let started = std::time::Instant::now();
    let messages = vec![AiChatMessage {
        role: "user".to_string(),
        content: "Reply with the single word PONG.".to_string(),
    }];

    let out = ai_chat_inner(
        &profile,
        &messages,
        timeout_ms.unwrap_or(20_000),
        &request_id,
    )
    .await;
    let latency_ms = started.elapsed().as_millis();

    match out {
        Ok(resp) => {
            ai_audit_log(
                "ai_test_connection",
                "read",
                &request_id,
                &format!(
                    "provider={} profile={} ok=true",
                    profile.provider_id, profile.id
                ),
            );
            Ok(AiConnectionTestResult {
                ok: true,
                request_id: resp.request_id,
                message: if resp.text.trim().is_empty() {
                    "Connection succeeded but provider returned empty text".to_string()
                } else {
                    format!("Connection succeeded: {}", resp.text.trim())
                },
                error_type: None,
                latency_ms,
            })
        }
        Err(err) => {
            let error_type = err.split(':').next().map(|s| s.trim().to_string());
            ai_audit_log(
                "ai_test_connection",
                "error",
                &request_id,
                &format!(
                    "provider={} profile={} ok=false error={}",
                    profile.provider_id, profile.id, err
                ),
            );
            Ok(AiConnectionTestResult {
                ok: false,
                request_id,
                message: err,
                error_type,
                latency_ms,
            })
        }
    }
}
