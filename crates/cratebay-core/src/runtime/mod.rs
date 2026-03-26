//! Built-in container runtime — platform dispatch.
//!
//! macOS: VZ.framework | Linux: KVM/QEMU | Windows: WSL2
//!
//! This module defines the platform-agnostic [`RuntimeManager`] trait and
//! all supporting types for managing the built-in container runtime.

pub mod common;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::ResourceUsage;

// ---------------------------------------------------------------------------
// Runtime State Machine (§3.1)
// ---------------------------------------------------------------------------

/// Runtime lifecycle state.
///
/// Follows the state machine defined in runtime-spec.md §3.1:
/// `None → Provisioned → Starting → Ready → Stopping → Stopped`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuntimeState {
    /// No runtime detected, needs provisioning.
    None,
    /// Image ready, VM not started.
    Provisioned,
    /// VM is booting or Docker initializing.
    Starting,
    /// Docker is available and responsive.
    Ready,
    /// VM is shutting down gracefully.
    Stopping,
    /// VM has been stopped.
    Stopped,
    /// Runtime error with description.
    Error(String),
}

// ---------------------------------------------------------------------------
// Provision Progress (§3.2)
// ---------------------------------------------------------------------------

/// Progress information emitted during the provisioning process.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvisionProgress {
    /// Current stage: "downloading", "extracting", "configuring", "complete".
    pub stage: String,
    /// Progress percentage (0.0 — 100.0).
    pub percent: f32,
    /// Bytes downloaded so far.
    pub bytes_downloaded: u64,
    /// Total bytes to download.
    pub bytes_total: u64,
    /// Human-readable progress message.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Health Status (§9.1)
// ---------------------------------------------------------------------------

/// Health check result for the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Current runtime state.
    pub runtime_state: RuntimeState,
    /// Whether the Docker socket is responding to pings.
    pub docker_responsive: bool,
    /// Docker engine version (if responsive).
    pub docker_version: Option<String>,
    /// VM uptime in seconds (if running).
    pub uptime_seconds: Option<u64>,
    /// Timestamp of this health check (RFC 3339).
    pub last_check: String,
    /// Which Docker backend is currently connected (always "builtin" for CrateBay runtime).
    pub docker_source: Option<String>,
}

// ---------------------------------------------------------------------------
// Runtime Configuration (§7.2)
// ---------------------------------------------------------------------------

/// Configuration for the built-in container runtime VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Number of CPU cores allocated to the VM.
    pub cpu_cores: u32,
    /// Memory allocated to the VM in MB.
    pub memory_mb: u64,
    /// Maximum disk size in GB (thin provisioned).
    pub disk_gb: u32,
    /// Whether to auto-start runtime on app launch.
    pub auto_start: bool,
    /// Shared directories (host → guest).
    pub shared_dirs: Vec<SharedDir>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            cpu_cores: 2,
            memory_mb: 2048,
            disk_gb: 20,
            auto_start: true,
            shared_dirs: vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Shared Directory (§5)
// ---------------------------------------------------------------------------

/// A host directory shared with the VM via VirtioFS / 9P.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedDir {
    /// Absolute path on the host filesystem.
    pub host_path: String,
    /// Mount tag used inside the VM.
    pub tag: String,
}

// ---------------------------------------------------------------------------
// Port Forwarding (§6)
// ---------------------------------------------------------------------------

/// A port forwarding rule between host and container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForward {
    /// Port on the host.
    pub host_port: u16,
    /// Port inside the container.
    pub container_port: u16,
    /// Transport protocol.
    pub protocol: Protocol,
}

/// Network protocol for port forwarding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
}

// ---------------------------------------------------------------------------
// RuntimeManager Trait (§3.2)
// ---------------------------------------------------------------------------

/// Platform-agnostic runtime manager trait.
///
/// Each platform (macOS, Linux, Windows) provides a concrete implementation.
/// Consumers use `Box<dyn RuntimeManager>` or `Arc<dyn RuntimeManager>` for
/// dynamic dispatch.
///
/// All async methods use `async-trait` to maintain object-safety.
#[async_trait]
pub trait RuntimeManager: Send + Sync {
    /// Query the current lifecycle state (fast local query, <100ms).
    async fn get_state(&self) -> Result<RuntimeState, AppError>;

    /// Download and prepare the VM image (first-run provisioning).
    ///
    /// The `on_progress` callback is invoked with progress updates during
    /// downloading, extraction, and configuration stages.
    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError>;

    /// Start the runtime VM.
    async fn start(&self) -> Result<(), AppError>;

    /// Stop the runtime VM gracefully.
    async fn stop(&self) -> Result<(), AppError>;

    /// Check if the runtime is healthy and Docker is responsive.
    async fn health_check(&self) -> Result<HealthStatus, AppError>;

    /// Get the Docker socket path for bollard connection.
    fn docker_socket_path(&self) -> PathBuf;

    /// Get current resource usage of the runtime VM.
    async fn resource_usage(&self) -> Result<ResourceUsage, AppError>;
}

// ---------------------------------------------------------------------------
// Factory Function
// ---------------------------------------------------------------------------

/// Create the platform-appropriate runtime manager.
///
/// Returns a boxed trait object that dispatches to:
/// - [`macos::MacOSRuntime`] on macOS
/// - [`linux::LinuxRuntime`] on Linux
/// - [`windows::WindowsRuntime`] on Windows
pub fn create_runtime_manager() -> Box<dyn RuntimeManager> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSRuntime::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxRuntime::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsRuntime::new())
    }
}

// ---------------------------------------------------------------------------
// Health Monitor (§9.2)
// ---------------------------------------------------------------------------

/// Start a periodic health monitor that checks runtime health every 30 seconds.
///
/// Uses a callback pattern because `cratebay-core` does not depend on Tauri.
/// The GUI layer wraps this callback to emit Tauri events.
///
/// Spawns a dedicated background thread with its own tokio runtime so it can
/// be called from any context (no pre-existing tokio reactor required).
///
/// # Arguments
///
/// * `runtime` — Shared runtime manager instance.
/// * `on_health` — Callback invoked with each health check result.
pub fn start_health_monitor(
    runtime: Arc<dyn RuntimeManager>,
    on_health: impl Fn(HealthStatus) + Send + 'static,
) {
    let spawn_result = std::thread::Builder::new()
        .name("health-monitor".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create health monitor runtime: {}", e);
                    return;
                }
            };
            rt.block_on(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    match runtime.health_check().await {
                        Ok(status) => {
                            on_health(status);
                        }
                        Err(e) => {
                            tracing::warn!("Health check failed: {}", e);
                            on_health(HealthStatus {
                                runtime_state: RuntimeState::Error(e.to_string()),
                                docker_responsive: false,
                                docker_version: None,
                                uptime_seconds: None,
                                last_check: chrono::Utc::now().to_rfc3339(),
                                docker_source: Some("builtin".to_string()),
                            });
                        }
                    }
                }
            });
        });

    if let Err(e) = spawn_result {
        tracing::error!("Failed to spawn health monitor thread: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_default_values() {
        let config = RuntimeConfig::default();
        assert_eq!(config.cpu_cores, 2);
        assert_eq!(config.memory_mb, 2048);
        assert_eq!(config.disk_gb, 20);
        assert!(config.auto_start);
        assert!(config.shared_dirs.is_empty());
    }

    #[test]
    fn runtime_state_serializes() {
        let state = RuntimeState::Ready;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"Ready\"");

        let error = RuntimeState::Error("test error".to_string());
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("test error"));
    }

    #[test]
    fn provision_progress_default() {
        let progress = ProvisionProgress::default();
        assert_eq!(progress.percent, 0.0);
        assert_eq!(progress.bytes_downloaded, 0);
        assert_eq!(progress.bytes_total, 0);
        assert!(progress.stage.is_empty());
    }

    #[test]
    fn protocol_serializes() {
        let tcp = Protocol::Tcp;
        let json = serde_json::to_string(&tcp).unwrap();
        assert_eq!(json, "\"tcp\"");

        let udp = Protocol::Udp;
        let json = serde_json::to_string(&udp).unwrap();
        assert_eq!(json, "\"udp\"");
    }

    #[test]
    fn runtime_state_all_variants_serialize_deserialize() {
        let variants = vec![
            (RuntimeState::None, "\"None\""),
            (RuntimeState::Provisioned, "\"Provisioned\""),
            (RuntimeState::Starting, "\"Starting\""),
            (RuntimeState::Ready, "\"Ready\""),
            (RuntimeState::Stopping, "\"Stopping\""),
            (RuntimeState::Stopped, "\"Stopped\""),
        ];
        for (state, expected_json) in variants {
            let json = serde_json::to_string(&state).unwrap();
            assert_eq!(json, expected_json, "serialize {:?}", state);
            let deserialized: RuntimeState = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, state, "deserialize {:?}", state);
        }

        // Error variant with payload
        let error = RuntimeState::Error("something went wrong".to_string());
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("something went wrong"));
        let deserialized: RuntimeState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, error);
    }

    #[test]
    fn provision_progress_serializes() {
        let progress = ProvisionProgress {
            stage: "downloading".into(),
            percent: 42.5,
            bytes_downloaded: 1024,
            bytes_total: 2048,
            message: "Downloading image...".into(),
        };
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"downloading\""));
        assert!(json.contains("42.5"));
        assert!(json.contains("1024"));
        assert!(json.contains("2048"));

        let deserialized: ProvisionProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.stage, "downloading");
        assert_eq!(deserialized.percent, 42.5);
        assert_eq!(deserialized.bytes_downloaded, 1024);
        assert_eq!(deserialized.bytes_total, 2048);
        assert_eq!(deserialized.message, "Downloading image...");
    }

    #[test]
    fn health_status_serializes() {
        let status = HealthStatus {
            runtime_state: RuntimeState::Ready,
            docker_responsive: true,
            docker_version: Some("24.0.7".into()),
            uptime_seconds: Some(3600),
            last_check: "2026-03-20T00:00:00Z".into(),
            docker_source: Some("builtin".to_string()),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"Ready\""));
        assert!(json.contains("true"));
        assert!(json.contains("24.0.7"));
        assert!(json.contains("3600"));

        let deserialized: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.runtime_state, RuntimeState::Ready);
        assert!(deserialized.docker_responsive);
        assert_eq!(deserialized.docker_version, Some("24.0.7".into()));
        assert_eq!(deserialized.uptime_seconds, Some(3600));
    }

    #[test]
    fn health_status_serializes_with_none_fields() {
        let status = HealthStatus {
            runtime_state: RuntimeState::None,
            docker_responsive: false,
            docker_version: None,
            uptime_seconds: None,
            last_check: "2026-03-20T00:00:00Z".into(),
            docker_source: Some("builtin".to_string()),
        };
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.runtime_state, RuntimeState::None);
        assert!(!deserialized.docker_responsive);
        assert!(deserialized.docker_version.is_none());
        assert!(deserialized.uptime_seconds.is_none());
    }

    #[test]
    fn port_forward_serializes() {
        let pf = PortForward {
            host_port: 8080,
            container_port: 80,
            protocol: Protocol::Tcp,
        };
        let json = serde_json::to_string(&pf).unwrap();
        assert!(json.contains("8080"));
        assert!(json.contains("80"));
        assert!(json.contains("\"tcp\""));

        let deserialized: PortForward = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host_port, 8080);
        assert_eq!(deserialized.container_port, 80);
        assert_eq!(deserialized.protocol, Protocol::Tcp);
    }

    #[test]
    fn shared_dir_serializes() {
        let sd = SharedDir {
            host_path: "/Users/test/project".into(),
            tag: "workspace".into(),
        };
        let json = serde_json::to_string(&sd).unwrap();
        assert!(json.contains("/Users/test/project"));
        assert!(json.contains("workspace"));

        let deserialized: SharedDir = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host_path, "/Users/test/project");
        assert_eq!(deserialized.tag, "workspace");
    }

    #[test]
    fn protocol_deserializes() {
        let tcp: Protocol = serde_json::from_str("\"tcp\"").unwrap();
        assert_eq!(tcp, Protocol::Tcp);

        let udp: Protocol = serde_json::from_str("\"udp\"").unwrap();
        assert_eq!(udp, Protocol::Udp);
    }

    #[test]
    fn runtime_config_serializes() {
        let config = RuntimeConfig {
            cpu_cores: 4,
            memory_mb: 4096,
            disk_gb: 50,
            auto_start: false,
            shared_dirs: vec![SharedDir {
                host_path: "/home/user/code".into(),
                tag: "code".into(),
            }],
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RuntimeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cpu_cores, 4);
        assert_eq!(deserialized.memory_mb, 4096);
        assert_eq!(deserialized.disk_gb, 50);
        assert!(!deserialized.auto_start);
        assert_eq!(deserialized.shared_dirs.len(), 1);
        assert_eq!(deserialized.shared_dirs[0].tag, "code");
    }

    #[test]
    fn create_runtime_manager_returns_valid_manager() {
        let manager = create_runtime_manager();
        // Verify the manager has the expected docker socket path pattern
        let socket_path = manager.docker_socket_path();
        assert!(
            socket_path.to_string_lossy().contains("docker.sock"),
            "Docker socket path should contain 'docker.sock': {:?}",
            socket_path
        );
    }

    // DOCKER_HOST parsing/connection is implemented in `crate::docker` so it can
    // support tcp/http/npipe endpoints across platforms.
}
