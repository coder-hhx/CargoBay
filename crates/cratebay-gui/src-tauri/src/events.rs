//! Tauri event definitions for streaming and real-time updates.

/// Event names used for Tauri event system.
pub mod event_names {
    /// Prefix for LLM streaming events. The full event name is
    /// `llm:stream:{channel_id}`.
    pub const LLM_STREAM_PREFIX: &str = "llm:stream";

    /// Prefix for image pull progress events. The full event name is
    /// `image:pull:{channel_id}` where channel_id is unique per pull operation.
    pub const IMAGE_PULL_PROGRESS_PREFIX: &str = "image:pull";

    #[allow(dead_code)]
    pub const CONTAINER_STATUS_CHANGE: &str = "container:status-change";
    #[allow(dead_code)]
    pub const MCP_CONNECTION_CHANGE: &str = "mcp:connection-change";
    pub const RUNTIME_HEALTH: &str = "runtime:health";
    pub const RUNTIME_PROVISION: &str = "runtime:provision";
}

/// Build a scoped LLM stream event name.
pub fn llm_stream_event(channel_id: &str) -> String {
    format!("{}:{}", event_names::LLM_STREAM_PREFIX, channel_id)
}

/// Build a scoped image pull progress event name.
pub fn image_pull_progress_event(channel_id: &str) -> String {
    format!("{}:{}", event_names::IMAGE_PULL_PROGRESS_PREFIX, channel_id)
}

/// Image pull progress update event payload.
#[derive(serde::Serialize, Clone, Debug)]
pub struct ImagePullProgress {
    /// Current layer being pulled.
    pub current_layer: u32,
    /// Total layers to pull.
    pub total_layers: u32,
    /// Overall progress percentage (0-100).
    pub progress_percent: u32,
    /// Human-readable status message.
    pub status: String,
    /// Whether pull is complete (either success or failure).
    pub complete: bool,
    /// Error message if pull failed. None means success (when complete=true).
    pub error: Option<String>,
}
