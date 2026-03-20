//! CrateBay Desktop App — Tauri v2 entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod state;

use std::sync::{Arc, Mutex};

use tauri::Emitter;

use state::AppState;

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cratebay=info".parse().unwrap()),
        )
        .init();

    // Initialize database
    let db_path = cratebay_core::storage::default_db_path()
        .expect("Failed to determine database path");
    let conn = cratebay_core::storage::init(&db_path)
        .expect("Failed to initialize database");
    tracing::info!("Database initialized at {}", db_path.display());

    // Create platform-specific runtime manager
    let runtime: Arc<dyn cratebay_core::runtime::RuntimeManager> =
        Arc::from(cratebay_core::runtime::create_runtime_manager());
    tracing::info!("Runtime manager initialized for {}", std::env::consts::OS);

    // Attempt Docker connection (non-blocking, optional)
    let docker = {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        match rt.block_on(cratebay_core::docker::try_connect()) {
            Some(d) => {
                tracing::info!("Docker connected");
                Some(Arc::new(d))
            }
            None => {
                tracing::warn!("Docker not available — container features disabled");
                None
            }
        }
    };

    let data_dir = db_path.parent().unwrap().to_path_buf();

    // Initialize MCP Manager
    // Load .mcp.json from project root and merge with SQLite-stored servers
    let mcp_manager = {
        use cratebay_core::mcp::{load_mcp_json, merge_server_configs, McpServerDbRow};

        let mcp_json = match load_mcp_json(std::path::Path::new(".")) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!("Failed to load .mcp.json: {}", e);
                None
            }
        };

        // Load user-configured servers from SQLite
        let db_rows = {
            let mut stmt = conn.prepare(
                "SELECT id, name, command, args, env, working_dir, enabled, notes, auto_start \
                 FROM mcp_servers ORDER BY name",
            ).unwrap_or_else(|e| {
                tracing::warn!("Failed to prepare MCP servers query: {}", e);
                // Return a dummy statement that yields no rows
                conn.prepare("SELECT 1 WHERE 0").unwrap()
            });

            let rows: Vec<McpServerDbRow> = stmt
                .query_map([], |row| {
                    let args_json: String = row.get(3)?;
                    let env_json: String = row.get(4)?;
                    let args: Vec<String> =
                        serde_json::from_str(&args_json).unwrap_or_default();
                    let env: Vec<String> =
                        serde_json::from_str(&env_json).unwrap_or_default();

                    Ok(McpServerDbRow {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        command: row.get(2)?,
                        args,
                        env,
                        working_dir: row.get(5)?,
                        enabled: row.get::<_, i32>(6)? != 0,
                        notes: row.get(7)?,
                        auto_start: row.get::<_, i32>(8)? != 0,
                    })
                })
                .map(|r| r.filter_map(|row| row.ok()).collect())
                .unwrap_or_default();
            rows
        };

        let configs = merge_server_configs(mcp_json.as_ref(), &db_rows);
        let manager = Arc::new(cratebay_core::mcp::McpManager::new());

        // Load configs and auto-start in a blocking tokio context
        let mgr_clone = manager.clone();
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime for MCP");
        rt.block_on(async {
            mgr_clone.load_configs(configs).await;
            mgr_clone.auto_start_servers().await;
        });

        tracing::info!("MCP manager initialized with {} servers", db_rows.len());
        manager
    };

    let app_state = AppState {
        docker,
        db: Arc::new(Mutex::new(conn)),
        data_dir,
        llm_cancel_tokens: Arc::new(Mutex::new(std::collections::HashMap::new())),
        runtime: runtime.clone(),
        mcp_manager,
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            // Start periodic health monitor (every 30s)
            let app_handle = app.handle().clone();
            cratebay_core::runtime::start_health_monitor(
                runtime,
                move |health| {
                    let _ = app_handle.emit(events::event_names::RUNTIME_HEALTH, &health);
                },
            );
            tracing::info!("Runtime health monitor started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Container
            commands::container::container_templates,
            commands::container::container_list,
            commands::container::container_create,
            commands::container::container_start,
            commands::container::container_stop,
            commands::container::container_delete,
            commands::container::container_exec,
            commands::container::container_exec_stream,
            commands::container::container_logs,
            commands::container::container_inspect,
            // LLM
            commands::llm::llm_proxy_stream,
            commands::llm::llm_proxy_cancel,
            commands::llm::llm_provider_list,
            commands::llm::llm_provider_create,
            commands::llm::llm_provider_update,
            commands::llm::llm_provider_delete,
            commands::llm::llm_provider_test,
            commands::llm::llm_models_fetch,
            commands::llm::llm_models_list,
            commands::llm::llm_models_toggle,
            // Storage
            commands::storage::settings_get,
            commands::storage::settings_update,
            commands::storage::api_key_save,
            commands::storage::api_key_delete,
            commands::storage::conversation_list,
            commands::storage::conversation_get_messages,
            commands::storage::conversation_create,
            commands::storage::conversation_delete,
            commands::storage::conversation_save_message,
            commands::storage::conversation_update_title,
            // MCP
            commands::mcp::mcp_server_list,
            commands::mcp::mcp_server_add,
            commands::mcp::mcp_server_remove,
            commands::mcp::mcp_server_start,
            commands::mcp::mcp_server_stop,
            commands::mcp::mcp_client_call_tool,
            commands::mcp::mcp_client_list_tools,
            commands::mcp::mcp_export_client_config,
            // System
            commands::system::system_info,
            commands::system::docker_status,
            commands::system::runtime_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CrateBay");
}
