//! CrateBay MCP Server — exposes container sandbox operations via Model Context Protocol.
//!
//! Communicates via stdio (stdin/stdout) using JSON-RPC 2.0 protocol.
//! Per mcp-spec.md §2.1.

mod audit;
mod error;
mod protocol;
mod sandbox;
mod security;
mod templates;
mod tools;

use std::sync::Arc;

use bollard::Docker;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use protocol::{
    InitializeResult, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse, ServerCapabilities,
    ServerInfo, ToolsCapability, ToolsListResult, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND,
    PARSE_ERROR,
};

/// MCP protocol version we support.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server name for MCP identification.
const SERVER_NAME: &str = "CrateBay";

/// Shared state for the MCP server.
struct McpState {
    docker: Docker,
    /// MCP client identifier (set during initialize).
    client_name: String,
    /// Whether the server has been initialized.
    initialized: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing — write to stderr so stdout is reserved for JSON-RPC
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("CrateBay MCP Server starting...");

    // Connect to Docker
    let docker = cratebay_core::docker::connect().await.map_err(|e| {
        tracing::error!("Failed to connect to Docker: {}", e);
        anyhow::anyhow!("Docker connection failed: {}", e)
    })?;

    tracing::info!("Docker connection established");

    let state = Arc::new(tokio::sync::Mutex::new(McpState {
        docker,
        client_name: String::new(),
        initialized: false,
    }));

    // stdio JSON-RPC loop
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut writer = stdout;

    tracing::info!("MCP Server ready (stdio transport)");

    let mut line_buf = String::new();
    loop {
        line_buf.clear();
        let bytes_read = reader.read_line(&mut line_buf).await?;

        // EOF — client closed stdin
        if bytes_read == 0 {
            tracing::info!("stdin closed, shutting down");
            break;
        }

        let trimmed = line_buf.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse the JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(e) => {
                let err_response =
                    JsonRpcErrorResponse::new(None, PARSE_ERROR, format!("Parse error: {}", e));
                write_response(&mut writer, &serde_json::to_string(&err_response)?).await?;
                continue;
            }
        };

        let response = handle_request(&state, request).await;

        if let Some(response_str) = response {
            write_response(&mut writer, &response_str).await?;
        }
        // No response for notifications (no id)
    }

    Ok(())
}

/// Write a JSON-RPC response line to stdout.
async fn write_response(
    writer: &mut tokio::io::Stdout,
    response: &str,
) -> Result<(), anyhow::Error> {
    writer.write_all(response.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

/// Handle a single JSON-RPC request and return the response string.
///
/// Returns None for notifications (requests without an id).
async fn handle_request(
    state: &Arc<tokio::sync::Mutex<McpState>>,
    request: JsonRpcRequest,
) -> Option<String> {
    let id = request.id.clone();
    let method = request.method.as_str();

    // Notifications (no id) — process but don't respond
    if id.is_none() {
        match method {
            "notifications/initialized" => {
                let mut s = state.lock().await;
                s.initialized = true;
                tracing::info!("Client sent initialized notification");
            }
            "notifications/cancelled" => {
                tracing::debug!("Client cancelled a request");
            }
            other => {
                tracing::debug!("Unknown notification: {}", other);
            }
        }
        return None;
    }

    // Requests (have id) — must respond
    let response_value = match method {
        "initialize" => handle_initialize(state, &request.params).await,
        "tools/list" => handle_tools_list().await,
        "tools/call" => handle_tools_call(state, &request.params).await,
        "ping" => Ok(serde_json::json!({})),
        _ => Err(JsonRpcErrorResponse::new(
            id.clone(),
            METHOD_NOT_FOUND,
            format!("Method not found: {}", method),
        )),
    };

    let response_str = match response_value {
        Ok(result) => {
            let response = JsonRpcResponse::new(id, result);
            serde_json::to_string(&response).ok()
        }
        Err(err_response) => serde_json::to_string(&err_response).ok(),
    };

    response_str
}

/// Handle the `initialize` request.
async fn handle_initialize(
    state: &Arc<tokio::sync::Mutex<McpState>>,
    params: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcErrorResponse> {
    // Extract client info
    let client_name = params
        .get("clientInfo")
        .and_then(|ci| ci.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown")
        .to_string();

    tracing::info!("Initialize request from client: {}", client_name);

    {
        let mut s = state.lock().await;
        s.client_name = client_name;
    }

    let result = InitializeResult {
        protocol_version: PROTOCOL_VERSION.to_string(),
        capabilities: ServerCapabilities {
            tools: ToolsCapability {
                list_changed: false,
            },
        },
        server_info: ServerInfo {
            name: SERVER_NAME.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
    };

    serde_json::to_value(&result).map_err(|e| {
        JsonRpcErrorResponse::new(None, INTERNAL_ERROR, format!("Serialization error: {}", e))
    })
}

/// Handle the `tools/list` request.
async fn handle_tools_list() -> Result<serde_json::Value, JsonRpcErrorResponse> {
    let catalog = tools::tool_catalog();
    let result = ToolsListResult { tools: catalog };

    serde_json::to_value(&result).map_err(|e| {
        JsonRpcErrorResponse::new(None, INTERNAL_ERROR, format!("Serialization error: {}", e))
    })
}

/// Handle the `tools/call` request.
async fn handle_tools_call(
    state: &Arc<tokio::sync::Mutex<McpState>>,
    params: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcErrorResponse> {
    let tool_name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
        JsonRpcErrorResponse::new(None, INVALID_PARAMS, "Missing 'name' parameter".to_string())
    })?;

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let s = state.lock().await;
    let caller = s.client_name.clone();
    let docker = s.docker.clone();
    drop(s);

    let result = tools::dispatch_tool_call(&docker, tool_name, &arguments, &caller).await;

    serde_json::to_value(&result).map_err(|e| {
        JsonRpcErrorResponse::new(None, INTERNAL_ERROR, format!("Serialization error: {}", e))
    })
}
