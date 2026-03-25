use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Container models
// ---------------------------------------------------------------------------

/// Container information returned from Docker API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub state: String,
    pub created_at: String,
    pub ports: Vec<PortMapping>,
    pub labels: HashMap<String, String>,
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u64>,
}

/// Port mapping between host and container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
}

/// Volume mount configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeMount {
    pub host_path: String,
    pub container_path: String,
    pub read_only: Option<bool>,
}

/// Container filter criteria for listing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContainerListFilters {
    pub status: Option<Vec<ContainerStatus>>,
    pub name: Option<String>,
    pub image: Option<String>,
    pub label: Option<HashMap<String, String>>,
    /// When `true`, only return containers with the
    /// `com.cratebay.sandbox.managed=true` label (created by CrateBay).
    /// When `false` or `None`, return all containers.
    pub managed_only: Option<bool>,
}

/// Identifies which Docker backend a connection was established through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DockerSource {
    /// CrateBay built-in VM runtime (VZ / KVM / WSL2).
    BuiltinRuntime,
    /// Colima — a popular macOS/Linux Docker daemon wrapper.
    Colima,
    /// Any other external Docker daemon (Docker Desktop, Podman socket, etc.).
    External,
}

impl DockerSource {
    /// Return the canonical string tag sent in Tauri events.
    pub fn as_str(&self) -> &'static str {
        match self {
            DockerSource::BuiltinRuntime => "builtin",
            DockerSource::Colima => "colima",
            DockerSource::External => "other",
        }
    }
}

/// Request to create a new container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerCreateRequest {
    pub name: String,
    pub image: String,
    pub command: Option<String>,
    pub env: Option<Vec<String>>,
    pub ports: Option<Vec<PortMapping>>,
    pub volumes: Option<Vec<VolumeMount>>,
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u64>,
    pub working_dir: Option<String>,
    pub auto_start: Option<bool>,
    pub labels: Option<HashMap<String, String>>,
    pub template_id: Option<String>,
}

/// Result of a container exec command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResult {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
}

/// Exec streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExecStreamChunk {
    Stdout { data: String },
    Stderr { data: String },
    Done { exit_code: i64 },
    Error { message: String },
}

/// Container detail (extended info from inspect).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerDetail {
    pub info: ContainerInfo,
    pub network_settings: serde_json::Value,
    pub mounts: Vec<serde_json::Value>,
    pub state: ContainerState,
}

/// Container state detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerState {
    pub status: String,
    pub running: bool,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub exit_code: Option<i64>,
    pub error: Option<String>,
    pub pid: Option<u64>,
}

/// Log retrieval options.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogOptions {
    pub tail: Option<u32>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub timestamps: Option<bool>,
}

/// Log entry from container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub stream: String,
    pub message: String,
    pub timestamp: Option<String>,
}

/// Real-time container resource usage snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStats {
    pub id: String,
    pub name: String,
    pub read_at: String,
    pub cpu_percent: f64,
    pub cpu_cores_used: f64,
    pub memory_used_mb: f64,
    pub memory_limit_mb: f64,
    pub memory_percent: f64,
}

/// Docker image information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerImageInfo {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size: i64,
    pub created: i64,
}

/// Docker local image info for the Images page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalImageInfo {
    pub id: String,
    pub repo_tags: Vec<String>,
    /// Compatibility field used by existing container dropdown UI.
    pub size: i64,
    pub size_bytes: u64,
    pub size_human: String,
    pub created: i64,
}

/// Docker registry search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSearchResult {
    pub source: String,
    pub reference: String,
    pub description: String,
    pub stars: Option<u64>,
    pub pulls: Option<u64>,
    pub official: bool,
}

/// Docker image inspection info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInspectInfo {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size_bytes: u64,
    pub created: String,
    pub architecture: String,
    pub os: String,
    pub docker_version: String,
    pub layers: u32,
}

/// Container lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContainerStatus {
    Running,
    Stopped,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Created,
}

// ---------------------------------------------------------------------------
// LLM / AI models
// ---------------------------------------------------------------------------

/// Supported LLM API format types.
///
/// Determines how the Rust proxy constructs outgoing HTTP requests and
/// parses the streaming response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    /// Anthropic Messages API (`/v1/messages`).
    Anthropic,
    /// OpenAI Responses API (`/v1/responses`) — supports reasoning effort.
    OpenAiResponses,
    /// OpenAI Chat Completions API (`/v1/chat/completions`).
    OpenAiCompletions,
}

impl ApiFormat {
    /// Convert to a string representation for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiFormat::Anthropic => "anthropic",
            ApiFormat::OpenAiResponses => "openai_responses",
            ApiFormat::OpenAiCompletions => "openai_completions",
        }
    }

    /// Parse from a database-stored string.
    pub fn parse_db(s: &str) -> Option<Self> {
        s.parse::<ApiFormat>().ok()
    }
}

impl std::str::FromStr for ApiFormat {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "anthropic" => Ok(ApiFormat::Anthropic),
            "openai_responses" => Ok(ApiFormat::OpenAiResponses),
            "openai_completions" => Ok(ApiFormat::OpenAiCompletions),
            _ => Err(()),
        }
    }
}

/// LLM provider configuration (stored in SQLite).
///
/// The `api_key` is **never** included — it is stored encrypted separately
/// and only decrypted in-memory during an outgoing request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProvider {
    pub id: String,
    pub name: String,
    pub api_base: String,
    pub api_format: ApiFormat,
    pub enabled: bool,
    /// `true` when an encrypted API key exists for this provider.
    pub has_api_key: bool,
    /// Free-form notes about this provider.
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A single chat message exchanged between frontend and Rust proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Describes a tool-call emitted by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    /// JSON-encoded arguments.
    pub arguments: String,
}

/// Tool definition sent alongside a chat request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub parameters: serde_json::Value,
}

/// Optional parameters for an LLM request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmOptions {
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub tools: Option<Vec<ToolDefinition>>,
    /// `"low"`, `"medium"`, or `"high"` — only effective when the provider
    /// uses `ApiFormat::OpenAiResponses`.
    pub reasoning_effort: Option<String>,
}

/// A chunk emitted during LLM response streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LlmStreamEvent {
    /// A content token fragment.
    Token { content: String },
    /// The model requested a tool call.
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    /// Stream completed successfully.
    Done { usage: UsageStats },
    /// An error occurred during streaming.
    Error { message: String },
}

/// Token usage statistics returned when the stream is complete.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStats {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Minimal information about a model, as returned by `/v1/models`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}

/// Extended model info stored in the local database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModelInfo {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub is_enabled: bool,
    pub supports_reasoning: bool,
}

// ---------------------------------------------------------------------------
// Conversation models
// ---------------------------------------------------------------------------

/// Summary of a conversation for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: u32,
    pub last_message_preview: Option<String>,
}

/// Full conversation with all messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDetail {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ChatMessage>,
}

/// Request to save a message to a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveMessageRequest {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Audit models
// ---------------------------------------------------------------------------

/// Auditable actions for the audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    ContainerCreate,
    ContainerStart,
    ContainerStop,
    ContainerDelete,
    ContainerExec,
    ApiKeySave,
    ApiKeyDelete,
    ProviderCreate,
    ProviderUpdate,
    ProviderDelete,
    ModelToggle,
    McpServerStart,
    McpServerStop,
    SettingsUpdate,
    ConversationCreate,
    ConversationDelete,
}

impl AuditAction {
    /// Convert to a string representation for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::ContainerCreate => "container.create",
            AuditAction::ContainerStart => "container.start",
            AuditAction::ContainerStop => "container.stop",
            AuditAction::ContainerDelete => "container.delete",
            AuditAction::ContainerExec => "container.exec",
            AuditAction::ApiKeySave => "api_key.save",
            AuditAction::ApiKeyDelete => "api_key.delete",
            AuditAction::ProviderCreate => "provider.create",
            AuditAction::ProviderUpdate => "provider.update",
            AuditAction::ProviderDelete => "provider.delete",
            AuditAction::ModelToggle => "model.toggle",
            AuditAction::McpServerStart => "mcp_server.start",
            AuditAction::McpServerStop => "mcp_server.stop",
            AuditAction::SettingsUpdate => "settings.update",
            AuditAction::ConversationCreate => "conversation.create",
            AuditAction::ConversationDelete => "conversation.delete",
        }
    }
}

/// Request to create a new LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderCreateRequest {
    pub name: String,
    pub api_base: String,
    pub api_key: String,
    pub api_format: ApiFormat,
}

/// Request to update an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderUpdateRequest {
    pub name: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub api_format: Option<ApiFormat>,
    pub enabled: Option<bool>,
}

/// Result from testing provider connectivity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestResult {
    pub success: bool,
    pub latency_ms: u64,
    pub model: String,
    pub error: Option<String>,
}

/// Docker connection status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerStatus {
    pub connected: bool,
    pub version: Option<String>,
    pub api_version: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub source: String,
    pub socket_path: Option<String>,
}

/// MCP server status with runtime info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatus {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
    pub enabled: bool,
    pub running: bool,
    pub pid: Option<u32>,
    pub last_started_at: Option<String>,
    pub last_exit_code: Option<i32>,
    pub tools: Vec<McpToolInfo>,
}

/// MCP tool information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub server_id: String,
    pub server_name: String,
}

// ---------------------------------------------------------------------------
// MCP models
// ---------------------------------------------------------------------------

/// MCP server connection information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub id: String,
    pub name: String,
    pub transport: McpTransport,
    pub status: McpConnectionStatus,
}

/// MCP server configuration for adding/updating.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub enabled: Option<bool>,
    pub notes: Option<String>,
    pub auto_start: Option<bool>,
}

/// MCP transport type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    Stdio,
    Sse,
}

/// MCP connection status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpConnectionStatus {
    Connected,
    Disconnected,
    Error,
}

// ---------------------------------------------------------------------------
// Runtime / System models
// ---------------------------------------------------------------------------

/// System-level information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// Operating system: "macos", "linux", "windows".
    pub os: String,
    /// OS version string.
    pub os_version: String,
    /// CPU architecture: "x86_64", "aarch64".
    pub arch: String,
    /// CrateBay application version.
    pub app_version: String,
    /// Application data directory (~/.cratebay/).
    pub data_dir: String,
    /// Database file path (~/.cratebay/cratebay.db).
    pub db_path: String,
    /// Database file size in bytes.
    pub db_size_bytes: u64,
    /// Log file path.
    pub log_path: String,
}

/// Built-in runtime status information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusInfo {
    /// Runtime state: "none", "provisioned", "starting", "ready", "stopped", "error".
    pub state: String,
    /// Platform identifier: "macos-vz", "linux-kvm", "windows-wsl2".
    pub platform: String,
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub disk_gb: f32,
    /// Whether Docker inside the runtime is responsive.
    pub docker_responsive: bool,
    pub uptime_seconds: Option<u64>,
    pub resource_usage: Option<ResourceUsage>,
}

/// Resource usage stats for the runtime VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUsage {
    pub cpu_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_used_gb: f32,
    pub disk_total_gb: f32,
    pub container_count: u32,
}
