//! MCP server and client management Tauri commands.
//!
//! These commands allow the frontend to manage MCP server connections
//! and call tools on connected servers. The commands delegate to
//! `McpManager` (in cratebay-core) for runtime operations and to
//! SQLite storage for persistent configuration.

use std::collections::HashMap;

use tauri::State;

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::mcp::{McpTransportType, ResolvedMcpServer};
use cratebay_core::models::{AuditAction, McpServerConfig, McpServerStatus, McpToolInfo};
use cratebay_core::{audit, storage, MutexExt};

/// List all configured MCP servers with their current status.
///
/// Merges runtime status from `McpManager` with persistent config from SQLite.
/// Servers loaded from `.mcp.json` that are not in the database are included
/// with their live runtime state (running, pid, tools, etc.).
#[tauri::command]
pub async fn mcp_server_list(
    state: State<'_, AppState>,
) -> Result<Vec<McpServerStatus>, AppError> {
    Ok(state.mcp_manager.list_servers().await)
}

/// Add a new MCP server configuration.
///
/// Persists the configuration to SQLite and registers it with the
/// `McpManager` so it can be started immediately.
#[tauri::command]
pub async fn mcp_server_add(
    state: State<'_, AppState>,
    config: McpServerConfig,
) -> Result<McpServerStatus, AppError> {
    let id = uuid::Uuid::new_v4().to_string();

    // Persist to SQLite
    let db_status = {
        let db = state.db.lock_or_recover()?;
        let result = storage::add_mcp_server(&db, &id, &config)?;
        audit::log_action(
            &db,
            &AuditAction::McpServerStart,
            &id,
            Some(&config.name),
            "user",
        )?;
        result
    };

    // Register with McpManager for runtime management
    let resolved = ResolvedMcpServer {
        id: id.clone(),
        name: config.name.clone(),
        transport: McpTransportType::Stdio,
        command: Some(config.command.clone()),
        args: config.args.clone().unwrap_or_default(),
        env: config.env.clone().unwrap_or_default(),
        working_dir: config.working_dir.clone(),
        url: None,
        headers: HashMap::new(),
        enabled: config.enabled.unwrap_or(true),
        notes: config.notes.clone().unwrap_or_default(),
        auto_start: config.auto_start.unwrap_or(false),
    };
    state.mcp_manager.register_server(resolved).await;

    // Return the status from McpManager (has correct runtime defaults)
    match state.mcp_manager.get_server_status(&id).await {
        Ok(status) => Ok(status),
        Err(_) => Ok(db_status),
    }
}

/// Remove an MCP server configuration.
///
/// Stops the server if it is running, removes it from `McpManager`,
/// and deletes the persistent configuration from SQLite.
#[tauri::command]
pub async fn mcp_server_remove(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    // Stop and remove from McpManager (best-effort)
    state.mcp_manager.remove_server(&id).await;

    // Remove from SQLite
    let db = state.db.lock_or_recover()?;
    storage::remove_mcp_server(&db, &id)?;
    audit::log_action(
        &db,
        &AuditAction::McpServerStop,
        &id,
        None,
        "user",
    )?;
    Ok(())
}

/// Start an MCP server by its ID.
///
/// Connects to the server using the configured transport (stdio/SSE),
/// performs the MCP initialize handshake, and discovers available tools.
#[tauri::command]
pub async fn mcp_server_start(
    state: State<'_, AppState>,
    id: String,
) -> Result<McpServerStatus, AppError> {
    let status = state.mcp_manager.start_server(&id).await?;

    // Audit log
    let db = state.db.lock_or_recover()?;
    audit::log_action(
        &db,
        &AuditAction::McpServerStart,
        &id,
        Some(&status.name),
        "user",
    )?;

    Ok(status)
}

/// Stop a running MCP server.
///
/// Disconnects from the server and cleans up the transport.
#[tauri::command]
pub async fn mcp_server_stop(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    state.mcp_manager.stop_server(&id).await?;

    // Audit log
    let db = state.db.lock_or_recover()?;
    audit::log_action(
        &db,
        &AuditAction::McpServerStop,
        &id,
        None,
        "user",
    )?;

    Ok(())
}

/// Call a tool on a connected MCP server.
///
/// Forwards the call through the MCP JSON-RPC protocol to the target
/// server and returns the tool execution result.
#[tauri::command]
pub async fn mcp_client_call_tool(
    state: State<'_, AppState>,
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    state
        .mcp_manager
        .call_tool(&server_id, &tool_name, arguments)
        .await
}

/// List all available tools across connected MCP servers.
///
/// Only returns tools from servers that are currently connected.
#[tauri::command]
pub async fn mcp_client_list_tools(
    state: State<'_, AppState>,
) -> Result<Vec<McpToolInfo>, AppError> {
    Ok(state.mcp_manager.list_all_tools().await)
}

/// Export CrateBay MCP server configuration for external AI clients.
///
/// Generates a JSON configuration suitable for Claude Desktop,
/// Cursor, or other MCP-compatible clients.
#[tauri::command]
pub async fn mcp_export_client_config(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    // Find the cratebay-mcp binary path
    let mcp_binary = which_cratebay_mcp();

    let mut config = serde_json::json!({
        "mcpServers": {
            "cratebay": {
                "command": mcp_binary,
                "args": [],
                "env": {}
            }
        }
    });

    // If a workspace root is configured, include it in the env
    let workspace_root = {
        let db = state.db.lock_or_recover()?;
        storage::get_setting(&db, "mcp.workspace_root")?
    };
    if let Some(root) = workspace_root {
        config["mcpServers"]["cratebay"]["env"]["CRATEBAY_MCP_WORKSPACE_ROOT"] =
            serde_json::Value::String(root);
    }

    Ok(config)
}

/// Attempt to locate the `cratebay-mcp` binary.
///
/// Checks common installation paths and falls back to assuming
/// it is on the system PATH.
fn which_cratebay_mcp() -> String {
    // Check next to the current executable
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let sibling = parent.join("cratebay-mcp");
            if sibling.exists() {
                return sibling.to_string_lossy().to_string();
            }
        }
    }

    // Fall back to PATH
    "cratebay-mcp".to_string()
}
