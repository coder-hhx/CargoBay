//! Container management Tauri commands.

use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::models::{
    ContainerCreateRequest, ContainerDetail, ContainerInfo, ContainerListFilters, ExecResult,
    LogEntry, LogOptions,
};
use cratebay_core::{audit, container, storage, validation};
use cratebay_core::models::AuditAction;
use cratebay_core::MutexExt;

/// List available container templates.
#[tauri::command]
pub async fn container_templates(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let db = state.db.lock_or_recover()?;
    storage::list_templates(&db)
}

/// List all containers, optionally filtered.
#[tauri::command]
pub async fn container_list(
    state: State<'_, AppState>,
    filters: Option<ContainerListFilters>,
) -> Result<Vec<ContainerInfo>, AppError> {
    let docker = state.require_docker()?;
    container::list(docker, true, filters).await
}

/// Create a new container.
#[tauri::command]
pub async fn container_create(
    state: State<'_, AppState>,
    request: ContainerCreateRequest,
) -> Result<ContainerInfo, AppError> {
    let docker = state.require_docker()?;

    // Validate input
    validation::validate_container_name(&request.name)?;
    if let (Some(cpu), Some(mem)) = (request.cpu_cores, request.memory_mb) {
        validation::validate_resource_limits(cpu, mem)?;
    }

    let result = container::create(docker, request).await?;

    // Audit
    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerCreate, &result.id, None, "user")?;

    Ok(result)
}

/// Start a stopped container.
#[tauri::command]
pub async fn container_start(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    let docker = state.require_docker()?;
    container::start(docker, &id).await?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerStart, &id, None, "user")?;
    Ok(())
}

/// Stop a running container.
#[tauri::command]
pub async fn container_stop(
    state: State<'_, AppState>,
    id: String,
    timeout: Option<u32>,
) -> Result<(), AppError> {
    let docker = state.require_docker()?;
    container::stop(docker, &id, timeout).await?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerStop, &id, None, "user")?;
    Ok(())
}

/// Remove a container.
#[tauri::command]
pub async fn container_delete(
    state: State<'_, AppState>,
    id: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    let docker = state.require_docker()?;
    container::delete(docker, &id, force.unwrap_or(false)).await?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerDelete, &id, None, "user")?;
    Ok(())
}

/// Execute a command inside a running container.
#[tauri::command]
pub async fn container_exec(
    state: State<'_, AppState>,
    id: String,
    cmd: Vec<String>,
    working_dir: Option<String>,
) -> Result<ExecResult, AppError> {
    let docker = state.require_docker()?;
    let result = container::exec(docker, &id, cmd, working_dir).await?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerExec, &id, None, "user")?;

    Ok(result)
}

/// Get container logs.
#[tauri::command]
pub async fn container_logs(
    state: State<'_, AppState>,
    id: String,
    options: Option<LogOptions>,
) -> Result<Vec<LogEntry>, AppError> {
    let docker = state.require_docker()?;
    container::logs(docker, &id, options).await
}

/// Inspect a container for detailed information.
#[tauri::command]
pub async fn container_inspect(
    state: State<'_, AppState>,
    id: String,
) -> Result<ContainerDetail, AppError> {
    let docker = state.require_docker()?;
    container::inspect(docker, &id).await
}

/// Execute a command with streaming output via Tauri Events.
///
/// Output is emitted as events on `exec:stream:{channel_id}`.
#[tauri::command]
pub async fn container_exec_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    cmd: Vec<String>,
    channel_id: String,
    working_dir: Option<String>,
) -> Result<(), AppError> {
    let docker = state.require_docker()?;

    let event_name = format!("exec:stream:{}", channel_id);
    let app_handle = app.clone();

    container::exec_stream(
        docker,
        &id,
        cmd,
        working_dir,
        move |chunk| {
            let _ = app_handle.emit(&event_name, &chunk);
        },
    )
    .await?;

    // Audit
    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerExec, &id, None, "user")?;

    Ok(())
}
