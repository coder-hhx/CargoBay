//! System-related Tauri commands.

use std::sync::Arc;
use std::time::Duration;

use tauri::State;

use crate::state::AppState;
use cratebay_core::docker;
use cratebay_core::error::AppError;
use cratebay_core::models::{DockerStatus, RuntimeStatusInfo, SystemInfo};
use cratebay_core::runtime::{RuntimeConfig, RuntimeState};
use cratebay_core::{storage, MutexExt};

const SETTINGS_KEY_RUNTIME_HTTP_PROXY: &str = "runtimeHttpProxy";
const SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE: &str = "runtimeHttpProxyBridge";
const SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST: &str = "runtimeHttpProxyBindHost";
const SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT: &str = "runtimeHttpProxyBindPort";
const SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST: &str = "runtimeHttpProxyGuestHost";

/// Get system information.
#[tauri::command]
pub async fn system_info(state: State<'_, AppState>) -> Result<SystemInfo, AppError> {
    let data_dir = state.data_dir.to_string_lossy().to_string();
    let db_path = state.data_dir.join("cratebay.db");
    let db_path_str = db_path.to_string_lossy().to_string();

    let db_size_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

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
///
/// Checks the current Docker connection in AppState (may have been
/// updated by the runtime auto-start background thread).
#[tauri::command]
pub async fn docker_status(state: State<'_, AppState>) -> Result<DockerStatus, AppError> {
    let docker_opt = {
        let guard = state
            .docker
            .lock()
            .map_err(|e| AppError::Runtime(format!("Docker state lock poisoned: {}", e)))?;
        guard.clone()
    };

    match docker_opt {
        Some(d) => {
            let is_available = docker::is_available(&d).await;
            if is_available {
                let version_info = docker::version(&d).await.ok();
                let socket_path = state.runtime.docker_socket_path();
                let source = if socket_path.exists() {
                    "built-in"
                } else {
                    "external"
                };
                Ok(DockerStatus {
                    connected: true,
                    version: version_info.as_ref().and_then(|v| v.version.clone()),
                    api_version: version_info.as_ref().and_then(|v| v.api_version.clone()),
                    os: version_info.as_ref().and_then(|v| v.os.clone()),
                    arch: version_info.as_ref().and_then(|v| v.arch.clone()),
                    source: source.to_string(),
                    socket_path: Some(socket_path.to_string_lossy().to_string()),
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
pub async fn runtime_status(state: State<'_, AppState>) -> Result<RuntimeStatusInfo, AppError> {
    let platform = match std::env::consts::OS {
        "macos" => "macos-vz",
        "linux" => "linux-kvm",
        "windows" => "windows-wsl2",
        other => other,
    };

    // Perform a health check via the runtime manager
    let mut health = state.runtime.health_check().await?;
    let config = RuntimeConfig::default();

    // Reconcile transient ping failures with the shared AppState Docker client.
    // This avoids reporting "starting" while an already-connected client is healthy.
    if !health.docker_responsive
        && matches!(
            health.runtime_state,
            RuntimeState::Starting | RuntimeState::Ready | RuntimeState::Error(_)
        )
    {
        let docker_opt = {
            let guard = state
                .docker
                .lock()
                .map_err(|e| AppError::Runtime(format!("Docker state lock poisoned: {}", e)))?;
            guard.clone()
        };

        if let Some(docker_client) = docker_opt {
            for attempt in 0..3 {
                if docker::is_available(&docker_client).await {
                    health.docker_responsive = true;
                    health.runtime_state = RuntimeState::Ready;
                    break;
                }
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
            }
        }
    }

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

/// Manually start the built-in runtime.
///
/// This command allows the frontend to trigger runtime start
/// (e.g., from Settings page or a retry button).
#[tauri::command]
pub async fn runtime_start(state: State<'_, AppState>) -> Result<String, AppError> {
    tracing::info!("Manual runtime start requested");

    apply_runtime_http_proxy_env(&state)?;

    // Step 1: Detect
    let current = state.runtime.detect().await?;
    tracing::info!("Runtime current state: {:?}", current);

    // Step 2: Provision if needed
    if current == RuntimeState::None {
        tracing::info!("Runtime needs provisioning...");
        state
            .runtime
            .provision(Box::new(|progress| {
                tracing::info!(
                    "Provision: {} - {:.1}% - {}",
                    progress.stage,
                    progress.percent,
                    progress.message
                );
            }))
            .await?;
    }

    // Step 3: Start
    state.runtime.start().await?;
    tracing::info!("Runtime started, waiting for Docker...");

    // Step 4: Wait for Docker and update AppState
    let socket_path = state.runtime.docker_socket_path();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    while std::time::Instant::now() < deadline {
        if socket_path.exists() {
            #[cfg(unix)]
            {
                if let Ok(docker) = bollard::Docker::connect_with_unix(
                    socket_path.to_str().unwrap_or_default(),
                    120,
                    bollard::API_DEFAULT_VERSION,
                ) {
                    if docker.ping().await.is_ok() {
                        tracing::info!("Docker connected via runtime socket");
                        state.set_docker(Some(Arc::new(docker)));
                        return Ok("Runtime started and Docker connected".to_string());
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    Ok("Runtime started but Docker not yet responsive".to_string())
}

/// Manually stop the built-in runtime.
#[tauri::command]
pub async fn runtime_stop(state: State<'_, AppState>) -> Result<String, AppError> {
    tracing::info!("Manual runtime stop requested");
    state.runtime.stop().await?;

    // Clear Docker connection since runtime is stopping
    state.set_docker(None);

    // Try to reconnect to external Docker if available
    if let Some(docker) = docker::try_connect().await {
        state.set_docker(Some(Arc::new(docker)));
        return Ok("Runtime stopped, reconnected to external Docker".to_string());
    }

    Ok("Runtime stopped".to_string())
}

/// Convert a [`RuntimeState`] enum to its string representation for the API.
fn format_runtime_state(state: &RuntimeState) -> String {
    match state {
        RuntimeState::None => "none".to_string(),
        RuntimeState::Provisioned => "provisioned".to_string(),
        RuntimeState::Provisioning | RuntimeState::Starting => "starting".to_string(),
        RuntimeState::Ready => "ready".to_string(),
        RuntimeState::Stopping | RuntimeState::Stopped => "stopped".to_string(),
        RuntimeState::Error(_) => "error".to_string(),
    }
}

#[derive(Debug)]
struct RuntimeHttpProxySettings {
    proxy: Option<String>,
    bridge_enabled: bool,
    bind_host: Option<String>,
    bind_port: Option<u16>,
    guest_host: Option<String>,
}

fn apply_runtime_http_proxy_env(state: &State<'_, AppState>) -> Result<(), AppError> {
    let settings = load_runtime_http_proxy_settings(state)?;

    set_or_remove_env_var("CRATEBAY_RUNTIME_HTTP_PROXY", settings.proxy.as_deref());
    std::env::set_var(
        "CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE",
        if settings.bridge_enabled { "1" } else { "0" },
    );
    set_or_remove_env_var(
        "CRATEBAY_RUNTIME_HTTP_PROXY_BIND_HOST",
        settings.bind_host.as_deref(),
    );
    set_or_remove_env_var(
        "CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT",
        settings.bind_port.map(|port| port.to_string()).as_deref(),
    );
    set_or_remove_env_var(
        "CRATEBAY_RUNTIME_HTTP_PROXY_GUEST_HOST",
        settings.guest_host.as_deref(),
    );

    tracing::info!(
        bridge_enabled = settings.bridge_enabled,
        bind_host = ?settings.bind_host,
        bind_port = ?settings.bind_port,
        guest_host = ?settings.guest_host,
        proxy_configured = settings.proxy.is_some(),
        "Applied runtime HTTP proxy settings from persisted app settings"
    );

    Ok(())
}

fn load_runtime_http_proxy_settings(
    state: &State<'_, AppState>,
) -> Result<RuntimeHttpProxySettings, AppError> {
    let db = state.db.lock_or_recover()?;
    let proxy =
        normalize_optional_setting(storage::get_setting(&db, SETTINGS_KEY_RUNTIME_HTTP_PROXY)?);
    let bridge_enabled = parse_boolish(storage::get_setting(
        &db,
        SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE,
    )?)
    .unwrap_or(false);
    let bind_host = normalize_optional_setting(storage::get_setting(
        &db,
        SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST,
    )?);
    let bind_port = storage::get_setting(&db, SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT)?
        .and_then(|raw| raw.trim().parse::<u16>().ok())
        .filter(|port| *port > 0);
    let guest_host = normalize_optional_setting(storage::get_setting(
        &db,
        SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST,
    )?);

    Ok(RuntimeHttpProxySettings {
        proxy,
        bridge_enabled,
        bind_host,
        bind_port,
        guest_host,
    })
}

fn normalize_optional_setting(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_boolish(raw: Option<String>) -> Option<bool> {
    let value = raw?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn set_or_remove_env_var(key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}

/// Debug command: frontend reports its status back to Rust.
/// Only compiled in debug builds.
#[cfg(debug_assertions)]
#[tauri::command]
pub fn webview_debug_report(info: String) {
    tracing::info!("=== WEBVIEW DEBUG REPORT ===\n{}", info);
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
                    .map(|l| {
                        l.trim_start_matches("PRETTY_NAME=")
                            .trim_matches('"')
                            .to_string()
                    })
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
