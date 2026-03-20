//! System-related Tauri commands.

use tauri::State;

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::models::{DockerStatus, RuntimeStatusInfo, SystemInfo};
use cratebay_core::runtime::{RuntimeConfig, RuntimeState};
use cratebay_core::docker;

/// Get system information.
#[tauri::command]
pub async fn system_info(
    state: State<'_, AppState>,
) -> Result<SystemInfo, AppError> {
    let data_dir = state.data_dir.to_string_lossy().to_string();
    let db_path = state.data_dir.join("cratebay.db");
    let db_path_str = db_path.to_string_lossy().to_string();

    let db_size_bytes = std::fs::metadata(&db_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let log_path = state.data_dir.join("cratebay.log");
    let log_path_str = log_path.to_string_lossy().to_string();

    Ok(SystemInfo {
        os: std::env::consts::OS.to_string(),
        os_version: os_version(),
        arch: std::env::consts::ARCH.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir,
        db_path: db_path_str,
        db_size_bytes,
        log_path: log_path_str,
    })
}

/// Get Docker connection status.
#[tauri::command]
pub async fn docker_status(
    state: State<'_, AppState>,
) -> Result<DockerStatus, AppError> {
    match &state.docker {
        Some(d) => {
            let is_available = docker::is_available(d).await;
            if is_available {
                let version_info = docker::version(d).await.ok();
                Ok(DockerStatus {
                    connected: true,
                    version: version_info.as_ref().and_then(|v| v.version.clone()),
                    api_version: version_info.as_ref().and_then(|v| v.api_version.clone()),
                    os: version_info.as_ref().and_then(|v| v.os.clone()),
                    arch: version_info.as_ref().and_then(|v| v.arch.clone()),
                    source: "external".to_string(),
                    socket_path: None,
                })
            } else {
                Ok(DockerStatus {
                    connected: false,
                    version: None,
                    api_version: None,
                    os: None,
                    arch: None,
                    source: "none".to_string(),
                    socket_path: None,
                })
            }
        }
        None => Ok(DockerStatus {
            connected: false,
            version: None,
            api_version: None,
            os: None,
            arch: None,
            source: "none".to_string(),
            socket_path: None,
        }),
    }
}

/// Get built-in runtime status.
///
/// Returns the current state of the built-in container runtime (VM),
/// including health, configuration, and resource usage.
#[tauri::command]
pub async fn runtime_status(
    state: State<'_, AppState>,
) -> Result<RuntimeStatusInfo, AppError> {
    let platform = match std::env::consts::OS {
        "macos" => "macos-vz",
        "linux" => "linux-kvm",
        "windows" => "windows-wsl2",
        other => other,
    };

    // Perform a health check via the runtime manager
    let health = state.runtime.health_check().await?;
    let config = RuntimeConfig::default();

    // Try to get resource usage (non-fatal if it fails)
    let resource_usage = state.runtime.resource_usage().await.ok();

    Ok(RuntimeStatusInfo {
        state: format_runtime_state(&health.runtime_state),
        platform: platform.to_string(),
        cpu_cores: config.cpu_cores,
        memory_mb: config.memory_mb,
        disk_gb: config.disk_gb as f32,
        docker_responsive: health.docker_responsive,
        uptime_seconds: health.uptime_seconds,
        resource_usage,
    })
}

/// Convert a [`RuntimeState`] enum to its string representation for the API.
fn format_runtime_state(state: &RuntimeState) -> String {
    match state {
        RuntimeState::None => "none".to_string(),
        RuntimeState::Provisioning => "provisioning".to_string(),
        RuntimeState::Provisioned => "provisioned".to_string(),
        RuntimeState::Starting => "starting".to_string(),
        RuntimeState::Ready => "ready".to_string(),
        RuntimeState::Stopping => "stopping".to_string(),
        RuntimeState::Stopped => "stopped".to_string(),
        RuntimeState::Error(msg) => format!("error: {}", msg),
    }
}

/// Get OS version string.
fn os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("PRETTY_NAME="))
                    .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "unknown".to_string()
    }
}
