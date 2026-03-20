//! Application state managed by Tauri.

use bollard::Docker;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

use cratebay_core::error::AppError;
use cratebay_core::mcp::McpManager;
use cratebay_core::runtime::RuntimeManager;

/// Shared application state accessible from all Tauri commands.
pub struct AppState {
    /// Docker client (optional — Docker may not be available).
    pub docker: Option<Arc<Docker>>,

    /// SQLite database connection.
    pub db: Arc<Mutex<Connection>>,

    /// Application data directory (~/.cratebay/).
    pub data_dir: PathBuf,

    /// Active LLM streaming session cancellation tokens.
    /// Key is the channel_id used to identify the streaming session.
    pub llm_cancel_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,

    /// Built-in container runtime manager (platform-specific).
    pub runtime: Arc<dyn RuntimeManager>,

    /// MCP server connection manager.
    pub mcp_manager: Arc<McpManager>,
}

impl AppState {
    /// Get a reference to the Docker client, or error if unavailable.
    pub fn require_docker(&self) -> Result<&Docker, AppError> {
        self.docker
            .as_deref()
            .ok_or_else(|| AppError::Docker(bollard::errors::Error::DockerResponseServerError {
                status_code: 503,
                message: "Docker is not available. Please install and start Docker.".to_string(),
            }))
    }
}
