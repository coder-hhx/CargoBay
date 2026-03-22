//! CrateBay Desktop App — Tauri v2 entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod state;

use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{Emitter, Manager};

use state::AppState;

/// Try to (re)connect to Docker via any available path.
///
/// Called as a fallback when the built-in runtime fails to start or
/// when we need to retry after runtime startup.
async fn try_reconnect_docker(app_handle: &tauri::AppHandle) {
    if let Some(docker) = cratebay_core::docker::try_connect().await {
        tracing::info!("Docker reconnected via fallback path");
        let state = app_handle.state::<AppState>();
        state.set_docker(Some(Arc::new(docker)));
        let _ = app_handle.emit("docker:connected", true);
    } else {
        tracing::warn!("Docker still not available after reconnection attempt");
    }
}

fn main() {
    // Initialize tracing
    let env_filter = tracing_subscriber::EnvFilter::from_default_env();
    let env_filter = match "cratebay=info".parse() {
        Ok(directive) => env_filter.add_directive(directive),
        Err(_) => env_filter,
    };
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    // Initialize database
    let db_path = match cratebay_core::storage::default_db_path() {
        Ok(path) => path,
        Err(e) => {
            tracing::error!("Failed to determine database path: {}", e);
            eprintln!("Fatal: Failed to determine database path: {}", e);
            std::process::exit(1);
        }
    };
    let conn = match cratebay_core::storage::init(&db_path) {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to initialize database: {}", e);
            eprintln!(
                "Fatal: Failed to initialize database at {}: {}",
                db_path.display(),
                e
            );
            std::process::exit(1);
        }
    };
    tracing::info!("Database initialized at {}", db_path.display());

    // Create platform-specific runtime manager
    let runtime: Arc<dyn cratebay_core::runtime::RuntimeManager> =
        Arc::from(cratebay_core::runtime::create_runtime_manager());
    tracing::info!("Runtime manager initialized for {}", std::env::consts::OS);

    // Attempt Docker connection (non-blocking, optional)
    // Try external Docker first; if unavailable, the runtime auto-start
    // in Tauri setup will handle it.
    let docker = {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("Failed to create tokio runtime for Docker: {}", e);
                eprintln!("Fatal: Failed to create tokio runtime: {}", e);
                std::process::exit(1);
            }
        };
        match rt.block_on(cratebay_core::docker::try_connect()) {
            Some(d) => {
                tracing::info!("Docker connected (external or existing runtime)");
                Some(Arc::new(d))
            }
            None => {
                tracing::info!(
                    "Docker not available yet — runtime auto-start will attempt connection"
                );
                None
            }
        }
    };

    let data_dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();

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
        let db_rows: Vec<McpServerDbRow> = (|| -> Vec<McpServerDbRow> {
            let mut stmt = match conn.prepare(
                "SELECT id, name, command, args, env, working_dir, enabled, notes, auto_start \
                 FROM mcp_servers ORDER BY name",
            ) {
                Ok(stmt) => stmt,
                Err(e) => {
                    tracing::warn!("Failed to prepare MCP servers query: {}", e);
                    return Vec::new();
                }
            };

            stmt.query_map([], |row| {
                let args_json: String = row.get(3)?;
                let env_json: String = row.get(4)?;
                let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();
                let env: Vec<String> = serde_json::from_str(&env_json).unwrap_or_default();

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
            .unwrap_or_default()
        })();

        let configs = merge_server_configs(mcp_json.as_ref(), &db_rows);
        let manager = Arc::new(cratebay_core::mcp::McpManager::new());

        // Load configs and auto-start in a blocking tokio context
        let mgr_clone = manager.clone();
        let mcp_rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("Failed to create tokio runtime for MCP: {}", e);
                eprintln!("Fatal: Failed to create tokio runtime for MCP: {}", e);
                std::process::exit(1);
            }
        };
        mcp_rt.block_on(async {
            mgr_clone.load_configs(configs).await;
            mgr_clone.auto_start_servers().await;
        });

        tracing::info!("MCP manager initialized with {} servers", db_rows.len());
        manager
    };

    let app_state = AppState {
        docker: Arc::new(Mutex::new(docker)),
        db: Arc::new(Mutex::new(conn)),
        data_dir,
        llm_cancel_tokens: Arc::new(Mutex::new(std::collections::HashMap::new())),
        runtime: runtime.clone(),
        mcp_manager,
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            // macOS: hide title text, show overlay traffic light buttons
            #[cfg(target_os = "macos")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_title("");
                    let _ = window.set_title_bar_style(TitleBarStyle::Overlay);
                }
            }

            // Start periodic health monitor (every 30s)
            let app_handle = app.handle().clone();
            let health_runtime = runtime.clone();
            cratebay_core::runtime::start_health_monitor(
                health_runtime,
                move |health| {
                    let _ = app_handle.emit(events::event_names::RUNTIME_HEALTH, &health);
                },
            );
            tracing::info!("Runtime health monitor started");

            // ── Runtime auto-start (background, non-blocking) ────────
            // If Docker is not yet connected, try to start the built-in
            // runtime and then reconnect Docker through the runtime socket.
            let auto_start_handle = app.handle().clone();
            let auto_start_runtime = runtime.clone();
            std::thread::Builder::new()
                .name("runtime-auto-start".to_string())
                .spawn(move || {
                    let rt = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt,
                        Err(e) => {
                            tracing::error!("Failed to create runtime auto-start tokio runtime: {}", e);
                            return;
                        }
                    };

                    rt.block_on(async {
                        // Check if Docker is already available
                        {
                            let state = auto_start_handle.state::<AppState>();
                            if state.has_docker() {
                                tracing::info!("Docker already connected, skipping runtime auto-start");
                                return;
                            }
                        }

                        tracing::info!("Starting built-in runtime auto-start sequence...");

                        // Step 1: Detect current state
                        let current_state = match auto_start_runtime.detect().await {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!("Runtime detect failed: {}", e);
                                try_reconnect_docker(&auto_start_handle).await;
                                return;
                            }
                        };
                        tracing::info!("Runtime detected state: {:?}", current_state);

                        // Step 2: Provision if needed, then start
                        match &current_state {
                            cratebay_core::runtime::RuntimeState::None => {
                                tracing::info!("Runtime needs provisioning, starting provision...");
                                let handle_clone = auto_start_handle.clone();
                                if let Err(e) = auto_start_runtime.provision(
                                    Box::new(move |progress| {
                                        tracing::info!(
                                            "Provision progress: {} - {:.1}% - {}",
                                            progress.stage,
                                            progress.percent,
                                            progress.message
                                        );
                                        let _ = handle_clone.emit("runtime:provision-progress", &progress);
                                    }),
                                ).await {
                                    tracing::warn!("Runtime provisioning failed: {}", e);
                                    tracing::info!("Falling back to external Docker detection");
                                    try_reconnect_docker(&auto_start_handle).await;
                                    return;
                                }
                                tracing::info!("Runtime provisioning complete");
                            }
                            cratebay_core::runtime::RuntimeState::Ready => {
                                tracing::info!("Runtime already ready, reconnecting Docker...");
                                try_reconnect_docker(&auto_start_handle).await;
                                return;
                            }
                            cratebay_core::runtime::RuntimeState::Error(msg) => {
                                tracing::warn!("Runtime in error state: {}", msg);
                                try_reconnect_docker(&auto_start_handle).await;
                                return;
                            }
                            _ => {
                                // Provisioned, Starting, Stopping, Stopped — try to start
                            }
                        }

                        // Step 3: Start the runtime
                        tracing::info!("Starting built-in runtime...");
                        if let Err(e) = auto_start_runtime.start().await {
                            tracing::warn!("Runtime start failed: {}", e);
                            tracing::info!("Falling back to external Docker detection");
                            try_reconnect_docker(&auto_start_handle).await;
                            return;
                        }
                        tracing::info!("Built-in runtime started successfully");

                        // Step 4: Wait for Docker to become responsive via runtime socket
                        let socket_path = auto_start_runtime.docker_socket_path();
                        tracing::info!("Waiting for Docker at {}...", socket_path.display());

                        let deadline = std::time::Instant::now() + Duration::from_secs(45);
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
                                            tracing::info!("Docker is responsive via built-in runtime!");
                                            let state = auto_start_handle.state::<AppState>();
                                            state.set_docker(Some(Arc::new(docker)));
                                            // Emit an event so the frontend knows Docker is now available
                                            let _ = auto_start_handle.emit("docker:connected", true);
                                            return;
                                        }
                                    }
                                }
                            }
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }

                        tracing::warn!("Docker did not become responsive via runtime socket within 45s");
                        // Final fallback: try external Docker
                        try_reconnect_docker(&auto_start_handle).await;
                    });
                })
                .ok(); // JoinHandle is dropped — the thread runs independently.

            // Debug: check WebView status
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").expect("main window not found");
                let window_clone = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));

                    // Read WebView URL
                    match window_clone.url() {
                        Ok(url) => tracing::info!("WebView URL: {}", url),
                        Err(e) => tracing::warn!("Failed to get URL: {}", e),
                    }
                    match window_clone.title() {
                        Ok(title) => tracing::info!("WebView title: {}", title),
                        Err(e) => tracing::warn!("Failed to get title: {}", e),
                    }
                    match window_clone.inner_size() {
                        Ok(size) => tracing::info!("WebView inner size: {:?}", size),
                        Err(e) => tracing::warn!("Failed to get size: {}", e),
                    }

                    // Inject JS that calls our debug command via __TAURI_INTERNALS__
                    let _ = window_clone.eval(r#"
                        (function() {
                            try {
                                var rootEl = document.getElementById('root');
                                var rootLen = rootEl ? rootEl.innerHTML.length : -1;
                                var rootSnippet = rootEl ? rootEl.innerHTML.substring(0, 2000) : 'NO_ROOT';
                                var errs = window.__CRATEBAY_ERRORS || [];
                                var hasTauri = typeof window.__TAURI_INTERNALS__ !== 'undefined';
                                var scripts = Array.from(document.scripts).map(function(s) { return (s.src || 'inline').substring(0, 100); });
                                var stylesheets = Array.from(document.styleSheets).length;
                                
                                var info = 'TAURI=' + hasTauri + 
                                    ' | READY=' + document.readyState + 
                                    ' | ROOT_LEN=' + rootLen + 
                                    ' | ERRORS=' + errs.length + 
                                    ' | SCRIPTS=' + scripts.length + 
                                    ' | STYLES=' + stylesheets +
                                    '\nSCRIPT_SRCS=' + JSON.stringify(scripts) +
                                    '\nERRORS=' + JSON.stringify(errs) +
                                    '\nROOT_FULL=' + rootSnippet;
                                
                                if (hasTauri) {
                                    window.__TAURI_INTERNALS__.invoke('webview_debug_report', { info: info });
                                }
                            } catch(ex) {
                                if (window.__TAURI_INTERNALS__) {
                                    window.__TAURI_INTERNALS__.invoke('webview_debug_report', { info: 'EXCEPTION: ' + ex.message + '\n' + ex.stack });
                                }
                            }
                        })();
                    "#);
                });
            }

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
            commands::container::image_list,
            commands::container::image_pull,
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
            commands::system::runtime_start,
            commands::system::runtime_stop,
            // Debug
            #[cfg(debug_assertions)]
            commands::system::webview_debug_report,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!("Failed to run CrateBay: {}", e);
            eprintln!("Fatal: Failed to run CrateBay: {}", e);
            std::process::exit(1);
        });
}
