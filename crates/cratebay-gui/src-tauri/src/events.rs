//! Tauri event definitions for streaming and real-time updates.

/// Event names used for Tauri event system.
pub mod event_names {
    /// Prefix for LLM streaming events. The full event name is
    /// `llm:stream:{channel_id}`.
    pub const LLM_STREAM_PREFIX: &str = "llm:stream";

    pub const CONTAINER_STATUS_CHANGE: &str = "container:status-change";
    pub const MCP_CONNECTION_CHANGE: &str = "mcp:connection-change";
    pub const RUNTIME_HEALTH: &str = "runtime:health";
}

/// Build a scoped LLM stream event name.
pub fn llm_stream_event(channel_id: &str) -> String {
    format!("{}:{}", event_names::LLM_STREAM_PREFIX, channel_id)
}
