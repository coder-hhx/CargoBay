//! Application state managed by Tauri.

use bollard::Docker;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;

use cratebay_core::engine::EnsureOptions;
use cratebay_core::error::AppError;
use cratebay_core::mcp::McpManager;
use cratebay_core::models::DockerSource;
use cratebay_core::runtime::RuntimeManager;
use cratebay_core::MutexExt;

/// Shared application state accessible from all Tauri commands.
pub struct AppState {
    /// Docker client (optional — Docker may not be available).
    /// Wrapped in Mutex so it can be updated after runtime starts.
    pub docker: Arc<Mutex<Option<Arc<Docker>>>>,

    /// Which backend the current Docker client is connected through.
    pub docker_source: Arc<Mutex<Option<DockerSource>>>,

    /// In-process single-flight guard for Docker initialisation.
    ///
    /// The first call to `ensure_docker_once()` that finds Docker unavailable
    /// will execute `engine::ensure_docker()` exactly once; all concurrent
    /// callers await that same future instead of each spawning a separate
    /// start sequence. Once resolved (Ok or Err) the result is cached so
    /// subsequent calls for the same session return immediately.
    pub docker_init: Arc<OnceCell<Result<Arc<Docker>, String>>>,

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
    /// Get a clone of the Docker client Arc, or error if unavailable.
    pub fn require_docker(&self) -> Result<Arc<Docker>, AppError> {
        let guard = self
            .docker
            .lock()
            .map_err(|e| AppError::Runtime(format!("Docker state mutex poisoned: {}", e)))?;
        guard.clone().ok_or_else(|| {
            AppError::Docker(bollard::errors::Error::DockerResponseServerError {
                status_code: 503,
                message: "Docker is not available. Please start CrateBay Runtime first."
                    .to_string(),
            })
        })
    }

    /// Ensure Docker is available, with in-process single-flight deduplication.
    ///
    /// Reads the `allowExternalDocker` setting from the database to decide
    /// whether to allow fallback to Colima / Docker Desktop.
    ///
    /// When Docker is not yet connected, only the **first** concurrent caller
    /// runs the full `engine::ensure_docker()` start sequence. All other
    /// concurrent callers await that same future.
    ///
    /// # Fast path
    /// If the shared Docker client is already present, it is returned
    /// **immediately without a ping** to avoid per-command serialisation.
    pub async fn ensure_docker_once(&self) -> Result<Arc<Docker>, AppError> {
        // Fast path: a client is already stored — return it immediately.
        if let Ok(docker) = self.require_docker() {
            return Ok(docker);
        }

        // Read the allow_external_docker setting.
        let allow_external = self
            .db
            .lock_or_recover()
            .ok()
            .and_then(|db| {
                cratebay_core::storage::get_setting(&db, "allowExternalDocker")
                    .ok()
                    .flatten()
            })
            .map(|v| v.trim().eq_ignore_ascii_case("true") || v.trim() == "1")
            .unwrap_or(false);

        // Single-flight init: exactly one concurrent caller runs ensure_docker.
        let result = self
            .docker_init
            .get_or_init(|| async {
                let options = EnsureOptions {
                    lock_wait_timeout: Duration::from_secs(60),
                    allow_external_docker: allow_external,
                    ..Default::default()
                };
                match cratebay_core::engine::ensure_docker(self.runtime.as_ref(), options).await {
                    Ok(docker) => {
                        // Infer source from the runtime socket path.
                        let source = if allow_external {
                            // When external is allowed we can't know which socket won,
                            // so record as External for now (health monitor will refine).
                            DockerSource::External
                        } else {
                            DockerSource::BuiltinRuntime
                        };
                        self.set_docker(Some(docker.clone()));
                        self.set_docker_source(Some(source));
                        Ok(docker)
                    }
                    Err(e) => Err(e.to_string()),
                }
            })
            .await;

        match result {
            Ok(docker) => Ok(docker.clone()),
            Err(msg) => Err(AppError::Runtime(msg.clone())),
        }
    }

    /// Update the Docker client (e.g., after runtime starts).
    pub fn set_docker(&self, docker: Option<Arc<Docker>>) {
        if let Ok(mut guard) = self.docker.lock() {
            *guard = docker;
        }
    }

    /// Get the current Docker connection source.
    pub fn get_docker_source(&self) -> Option<DockerSource> {
        self.docker_source.lock().ok()?.clone()
    }

    /// Update the Docker connection source.
    pub fn set_docker_source(&self, source: Option<DockerSource>) {
        if let Ok(mut guard) = self.docker_source.lock() {
            *guard = source;
        }
    }

    /// Check if Docker is currently available.
    pub fn has_docker(&self) -> bool {
        self.docker.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Returns true if connected to the CrateBay built-in runtime.
    pub fn is_builtin_runtime(&self) -> bool {
        matches!(self.get_docker_source(), Some(DockerSource::BuiltinRuntime))
    }
}
