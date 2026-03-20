//! MCP Manager: manages multiple MCP server connections.
//!
//! Provides `McpManager` for coordinating connections to multiple MCP servers,
//! and `McpServerConnection` for individual server lifecycle management.

use crate::error::AppError;
use crate::mcp::config::{McpTransportType, ResolvedMcpServer};
use crate::mcp::jsonrpc::{JsonRpcNotification, JsonRpcRequest};
use crate::mcp::transport::{McpTransport, SseTransport, StdioTransport};
use crate::models::{McpServerStatus, McpToolInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

/// Connection lifecycle state (matches mcp-spec.md §3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Configuration loaded but not yet connected.
    ConfigLoaded,
    /// Currently spawning/connecting.
    Connecting,
    /// Successfully connected and initialized.
    Connected,
    /// Explicitly disconnected.
    Disconnected,
    /// Connection failed after retries.
    Error(String),
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::ConfigLoaded => write!(f, "config_loaded"),
            ConnectionState::Connecting => write!(f, "connecting"),
            ConnectionState::Connected => write!(f, "connected"),
            ConnectionState::Disconnected => write!(f, "disconnected"),
            ConnectionState::Error(msg) => write!(f, "error: {}", msg),
        }
    }
}

// ---------------------------------------------------------------------------
// McpServerConnection
// ---------------------------------------------------------------------------

/// Manages a single MCP server connection.
pub struct McpServerConnection {
    /// Server configuration.
    config: ResolvedMcpServer,
    /// Current connection state.
    state: RwLock<ConnectionState>,
    /// Active transport (None if not connected).
    transport: RwLock<Option<Box<dyn McpTransport>>>,
    /// Discovered tools from this server.
    tools: RwLock<Vec<McpToolInfo>>,
    /// Timestamp when the server was last started.
    last_started_at: RwLock<Option<String>>,
    /// Last exit code of the server process.
    last_exit_code: RwLock<Option<i32>>,
}

impl McpServerConnection {
    /// Create a new connection from configuration.
    pub fn new(config: ResolvedMcpServer) -> Self {
        Self {
            config,
            state: RwLock::new(ConnectionState::ConfigLoaded),
            transport: RwLock::new(None),
            tools: RwLock::new(Vec::new()),
            last_started_at: RwLock::new(None),
            last_exit_code: RwLock::new(None),
        }
    }

    /// Connect to the MCP server with retry logic.
    ///
    /// Implements 3 retries with exponential backoff (1s, 2s, 4s).
    pub async fn connect(&self) -> Result<(), AppError> {
        {
            let mut state = self.state.write().await;
            *state = ConnectionState::Connecting;
        }

        let mut last_error = None;
        let retry_delays = [
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(4),
        ];

        for (attempt, _delay) in retry_delays.iter().enumerate() {
            match self.try_connect().await {
                Ok(()) => {
                    let now = chrono::Utc::now().to_rfc3339();
                    {
                        let mut started = self.last_started_at.write().await;
                        *started = Some(now);
                    }
                    {
                        let mut state = self.state.write().await;
                        *state = ConnectionState::Connected;
                    }
                    tracing::info!(
                        "MCP server '{}' connected successfully",
                        self.config.name
                    );
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(
                        "MCP server '{}' connection attempt {} failed: {}",
                        self.config.name,
                        attempt + 1,
                        e
                    );
                    last_error = Some(e);
                    if attempt < retry_delays.len() - 1 {
                        tokio::time::sleep(retry_delays[attempt]).await;
                    }
                }
            }
        }

        let error_msg = last_error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "Unknown error".to_string());
        {
            let mut state = self.state.write().await;
            *state = ConnectionState::Error(error_msg.clone());
        }

        Err(AppError::Mcp(format!(
            "Failed to connect to MCP server '{}' after 3 retries: {}",
            self.config.name, error_msg
        )))
    }

    /// Attempt a single connection (no retry).
    async fn try_connect(&self) -> Result<(), AppError> {
        let transport: Box<dyn McpTransport> = match self.config.transport {
            McpTransportType::Stdio => {
                let command = self.config.command.as_deref().ok_or_else(|| {
                    AppError::Mcp(format!(
                        "No command specified for stdio server '{}'",
                        self.config.name
                    ))
                })?;

                Box::new(
                    StdioTransport::spawn(
                        command,
                        &self.config.args,
                        &self.config.env,
                        self.config.working_dir.as_deref(),
                    )
                    .await?,
                )
            }
            McpTransportType::Sse => {
                let url = self.config.url.as_deref().ok_or_else(|| {
                    AppError::Mcp(format!(
                        "No URL specified for SSE server '{}'",
                        self.config.name
                    ))
                })?;

                let sse = SseTransport::new(url, &self.config.headers)?;
                sse.connect().await?;
                Box::new(sse)
            }
        };

        // Step 1: Send `initialize` request
        let init_request = JsonRpcRequest::new(
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "CrateBay",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
        );

        let init_response = transport.send_request(&init_request).await?;

        if let Some(error) = init_response.error {
            return Err(AppError::Mcp(format!(
                "MCP initialize failed: {}",
                error
            )));
        }

        // Step 2: Send `notifications/initialized`
        let initialized_notification =
            JsonRpcNotification::new("notifications/initialized", Some(serde_json::json!({})));
        transport.send_notification(&initialized_notification).await?;

        // Step 3: Discover tools via `tools/list`
        let tools_request =
            JsonRpcRequest::new("tools/list", Some(serde_json::json!({})));
        let tools_response = transport.send_request(&tools_request).await?;

        let discovered_tools = if let Some(result) = tools_response.result {
            Self::parse_tools_response(&self.config.id, &self.config.name, &result)
        } else if let Some(error) = tools_response.error {
            tracing::warn!(
                "MCP tools/list failed for '{}': {}",
                self.config.name,
                error
            );
            Vec::new()
        } else {
            Vec::new()
        };

        tracing::info!(
            "MCP server '{}' discovered {} tools",
            self.config.name,
            discovered_tools.len()
        );

        // Store transport and tools
        {
            let mut t = self.transport.write().await;
            *t = Some(transport);
        }
        {
            let mut tools = self.tools.write().await;
            *tools = discovered_tools;
        }

        Ok(())
    }

    /// Parse the `tools/list` response into `McpToolInfo` objects.
    fn parse_tools_response(
        server_id: &str,
        server_name: &str,
        result: &serde_json::Value,
    ) -> Vec<McpToolInfo> {
        let tools_array = result
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        tools_array
            .iter()
            .filter_map(|tool| {
                let name = tool.get("name")?.as_str()?.to_string();
                let description = tool
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input_schema = tool
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                Some(McpToolInfo {
                    name,
                    description,
                    input_schema,
                    server_id: server_id.to_string(),
                    server_name: server_name.to_string(),
                })
            })
            .collect()
    }

    /// Call a tool on this server.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let transport = self.transport.read().await;
        let transport = transport.as_ref().ok_or_else(|| {
            AppError::Mcp(format!(
                "MCP server '{}' is not connected",
                self.config.name
            ))
        })?;

        let request = JsonRpcRequest::new(
            "tools/call",
            Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments,
            })),
        );

        let response = transport.send_request(&request).await?;

        if let Some(error) = response.error {
            return Err(AppError::Mcp(format!(
                "Tool '{}' on server '{}' failed: {}",
                tool_name, self.config.name, error
            )));
        }

        Ok(response.result.unwrap_or(serde_json::Value::Null))
    }

    /// Disconnect from the server.
    pub async fn disconnect(&self) -> Result<(), AppError> {
        let mut transport = self.transport.write().await;
        if let Some(ref t) = *transport {
            t.shutdown().await?;
        }
        *transport = None;

        {
            let mut tools = self.tools.write().await;
            tools.clear();
        }
        {
            let mut state = self.state.write().await;
            *state = ConnectionState::Disconnected;
        }

        tracing::info!("MCP server '{}' disconnected", self.config.name);
        Ok(())
    }

    /// Get the current connection state.
    pub async fn connection_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Get the list of discovered tools.
    pub async fn tools(&self) -> Vec<McpToolInfo> {
        self.tools.read().await.clone()
    }

    /// Check if the server is currently connected.
    pub async fn is_connected(&self) -> bool {
        *self.state.read().await == ConnectionState::Connected
    }

    /// Get the full server status.
    pub async fn status(&self) -> McpServerStatus {
        let state = self.state.read().await;
        let transport = self.transport.read().await;
        let tools = self.tools.read().await;
        let last_started = self.last_started_at.read().await;
        let last_exit = self.last_exit_code.read().await;

        let running = *state == ConnectionState::Connected;
        let pid = transport.as_ref().and_then(|t| t.pid());

        McpServerStatus {
            id: self.config.id.clone(),
            name: self.config.name.clone(),
            command: self.config.command.clone().unwrap_or_default(),
            args: self.config.args.clone(),
            env: self.config.env.clone(),
            enabled: self.config.enabled,
            running,
            pid,
            last_started_at: last_started.clone(),
            last_exit_code: *last_exit,
            tools: tools.clone(),
        }
    }

    /// Get the server ID.
    pub fn id(&self) -> &str {
        &self.config.id
    }

    /// Get the server name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Whether this server is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Whether this server should auto-start.
    pub fn auto_start(&self) -> bool {
        self.config.auto_start
    }
}

// ---------------------------------------------------------------------------
// McpManager
// ---------------------------------------------------------------------------

/// Manages multiple MCP server connections.
///
/// This is the main entry point for the MCP client subsystem. It holds
/// all configured servers and provides methods to start, stop, and
/// query them.
pub struct McpManager {
    /// Server connections, keyed by server ID.
    servers: RwLock<HashMap<String, Arc<McpServerConnection>>>,
}

impl McpManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
        }
    }

    /// Load server configurations and register them (without connecting).
    pub async fn load_configs(&self, configs: Vec<ResolvedMcpServer>) {
        let mut servers = self.servers.write().await;
        for config in configs {
            let id = config.id.clone();
            servers.insert(id, Arc::new(McpServerConnection::new(config)));
        }
    }

    /// Start (connect to) a specific server by ID.
    pub async fn start_server(&self, id: &str) -> Result<McpServerStatus, AppError> {
        let server = {
            let servers = self.servers.read().await;
            servers.get(id).cloned().ok_or_else(|| AppError::NotFound {
                entity: "MCP server".to_string(),
                id: id.to_string(),
            })?
        };

        server.connect().await?;
        Ok(server.status().await)
    }

    /// Stop (disconnect from) a specific server by ID.
    pub async fn stop_server(&self, id: &str) -> Result<(), AppError> {
        let server = {
            let servers = self.servers.read().await;
            servers.get(id).cloned().ok_or_else(|| AppError::NotFound {
                entity: "MCP server".to_string(),
                id: id.to_string(),
            })?
        };

        server.disconnect().await
    }

    /// Start all enabled servers that are configured for auto-start.
    pub async fn auto_start_servers(&self) {
        let servers: Vec<Arc<McpServerConnection>> = {
            let map = self.servers.read().await;
            map.values()
                .filter(|s| s.is_enabled() && s.auto_start())
                .cloned()
                .collect()
        };

        for server in servers {
            let name = server.name().to_string();
            tokio::spawn(async move {
                if let Err(e) = server.connect().await {
                    tracing::error!("Auto-start failed for MCP server '{}': {}", name, e);
                }
            });
        }
    }

    /// List all servers with their current status.
    pub async fn list_servers(&self) -> Vec<McpServerStatus> {
        let servers = self.servers.read().await;
        let mut statuses = Vec::with_capacity(servers.len());
        for server in servers.values() {
            statuses.push(server.status().await);
        }
        statuses
    }

    /// Get all available tools across all connected servers.
    pub async fn list_all_tools(&self) -> Vec<McpToolInfo> {
        let servers = self.servers.read().await;
        let mut all_tools = Vec::new();
        for server in servers.values() {
            if server.is_connected().await {
                all_tools.extend(server.tools().await);
            }
        }
        all_tools
    }

    /// Call a tool on a specific server.
    pub async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let server = {
            let servers = self.servers.read().await;
            servers
                .get(server_id)
                .cloned()
                .ok_or_else(|| AppError::NotFound {
                    entity: "MCP server".to_string(),
                    id: server_id.to_string(),
                })?
        };

        if !server.is_connected().await {
            return Err(AppError::Mcp(format!(
                "MCP server '{}' is not connected",
                server_id
            )));
        }

        server.call_tool(tool_name, arguments).await
    }

    /// Get status for a specific server.
    pub async fn get_server_status(
        &self,
        id: &str,
    ) -> Result<McpServerStatus, AppError> {
        let servers = self.servers.read().await;
        let server = servers.get(id).ok_or_else(|| AppError::NotFound {
            entity: "MCP server".to_string(),
            id: id.to_string(),
        })?;
        Ok(server.status().await)
    }

    /// Register a single server configuration (without connecting).
    ///
    /// If a server with the same ID already exists, it will be replaced
    /// (the old connection is disconnected first).
    pub async fn register_server(&self, config: ResolvedMcpServer) {
        let id = config.id.clone();
        let new_conn = Arc::new(McpServerConnection::new(config));

        let mut servers = self.servers.write().await;
        if let Some(old) = servers.remove(&id) {
            // Best-effort disconnect
            let _ = old.disconnect().await;
        }
        servers.insert(id, new_conn);
    }

    /// Remove a server from the manager.
    ///
    /// If the server is connected, it will be disconnected first.
    /// Returns `true` if a server was removed, `false` if not found.
    pub async fn remove_server(&self, id: &str) -> bool {
        let mut servers = self.servers.write().await;
        if let Some(server) = servers.remove(id) {
            let _ = server.disconnect().await;
            true
        } else {
            false
        }
    }

    /// Shut down all server connections.
    pub async fn shutdown_all(&self) {
        let servers = self.servers.read().await;
        for server in servers.values() {
            if let Err(e) = server.disconnect().await {
                tracing::error!(
                    "Error shutting down MCP server '{}': {}",
                    server.name(),
                    e
                );
            }
        }
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::config::McpTransportType;
    use std::collections::HashMap;

    fn make_test_config(id: &str, name: &str) -> ResolvedMcpServer {
        ResolvedMcpServer {
            id: id.to_string(),
            name: name.to_string(),
            transport: McpTransportType::Stdio,
            command: Some("echo".to_string()),
            args: vec!["test".to_string()],
            env: vec![],
            working_dir: None,
            url: None,
            headers: HashMap::new(),
            enabled: true,
            notes: String::new(),
            auto_start: false,
        }
    }

    #[test]
    fn test_manager_new_is_empty() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            let servers = manager.list_servers().await;
            assert!(servers.is_empty());
        });
    }

    #[test]
    fn test_manager_load_configs() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            let configs = vec![
                make_test_config("server1", "Server One"),
                make_test_config("server2", "Server Two"),
            ];
            manager.load_configs(configs).await;
            let servers = manager.list_servers().await;
            assert_eq!(servers.len(), 2);
        });
    }

    #[test]
    fn test_manager_register_server() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            manager.register_server(make_test_config("s1", "Server 1")).await;
            let servers = manager.list_servers().await;
            assert_eq!(servers.len(), 1);
            assert_eq!(servers[0].id, "s1");
        });
    }

    #[test]
    fn test_manager_register_replaces_existing() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            manager.register_server(make_test_config("s1", "Original")).await;
            manager.register_server(make_test_config("s1", "Replaced")).await;
            let servers = manager.list_servers().await;
            assert_eq!(servers.len(), 1);
            assert_eq!(servers[0].name, "Replaced");
        });
    }

    #[test]
    fn test_manager_remove_server() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            manager.register_server(make_test_config("s1", "Server 1")).await;
            assert!(manager.remove_server("s1").await);
            let servers = manager.list_servers().await;
            assert!(servers.is_empty());
        });
    }

    #[test]
    fn test_manager_remove_nonexistent() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            assert!(!manager.remove_server("nonexistent").await);
        });
    }

    #[test]
    fn test_manager_start_nonexistent_server() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            let result = manager.start_server("nonexistent").await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_manager_stop_nonexistent_server() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            let result = manager.stop_server("nonexistent").await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_manager_list_all_tools_empty() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            let tools = manager.list_all_tools().await;
            assert!(tools.is_empty());
        });
    }

    #[test]
    fn test_manager_get_server_status_not_found() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            let result = manager.get_server_status("nonexistent").await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_manager_get_server_status() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            manager.register_server(make_test_config("s1", "Test Server")).await;
            let status = manager.get_server_status("s1").await.unwrap();
            assert_eq!(status.id, "s1");
            assert_eq!(status.name, "Test Server");
            assert!(!status.running);
        });
    }

    #[test]
    fn test_manager_call_tool_not_connected() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            manager.register_server(make_test_config("s1", "Test")).await;
            let result = manager.call_tool("s1", "some_tool", serde_json::json!({})).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_connection_state_display() {
        assert_eq!(format!("{}", ConnectionState::ConfigLoaded), "config_loaded");
        assert_eq!(format!("{}", ConnectionState::Connecting), "connecting");
        assert_eq!(format!("{}", ConnectionState::Connected), "connected");
        assert_eq!(format!("{}", ConnectionState::Disconnected), "disconnected");
        assert_eq!(
            format!("{}", ConnectionState::Error("oops".to_string())),
            "error: oops"
        );
    }

    #[test]
    fn test_connection_initial_state() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let conn = McpServerConnection::new(make_test_config("test", "Test"));
            assert_eq!(conn.connection_state().await, ConnectionState::ConfigLoaded);
            assert!(!conn.is_connected().await);
            assert!(conn.tools().await.is_empty());
        });
    }

    #[test]
    fn test_connection_id_and_name() {
        let conn = McpServerConnection::new(make_test_config("my-id", "My Server"));
        assert_eq!(conn.id(), "my-id");
        assert_eq!(conn.name(), "My Server");
    }

    #[test]
    fn test_connection_enabled() {
        let config = make_test_config("test", "Test");
        let conn = McpServerConnection::new(config);
        assert!(conn.is_enabled());
    }

    #[test]
    fn test_connection_auto_start() {
        let mut config = make_test_config("test", "Test");
        config.auto_start = true;
        let conn = McpServerConnection::new(config);
        assert!(conn.auto_start());
    }

    #[test]
    fn test_manager_shutdown_all_empty() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::new();
            // Should not panic
            manager.shutdown_all().await;
        });
    }

    #[test]
    fn test_manager_default() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = McpManager::default();
            assert!(manager.list_servers().await.is_empty());
        });
    }
}
