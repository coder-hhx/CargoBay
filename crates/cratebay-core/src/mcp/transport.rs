//! MCP transport layer: stdio and SSE implementations.

use crate::error::AppError;
use crate::mcp::jsonrpc::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};

// ---------------------------------------------------------------------------
// Transport trait
// ---------------------------------------------------------------------------

/// Common interface for MCP transports.
#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC request and wait for the response.
    async fn send_request(
        &self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse, AppError>;

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(
        &self,
        notification: &JsonRpcNotification,
    ) -> Result<(), AppError>;

    /// Shut down the transport.
    async fn shutdown(&self) -> Result<(), AppError>;

    /// Get the PID of the underlying process (stdio only).
    fn pid(&self) -> Option<u32>;
}

// ---------------------------------------------------------------------------
// Stdio Transport
// ---------------------------------------------------------------------------

/// Manages communication state for a single request.
struct PendingRequest {
    sender: oneshot::Sender<JsonRpcResponse>,
}

/// Stdio transport: communicates with a child process via stdin/stdout.
pub struct StdioTransport {
    /// Channel to send outgoing messages (requests/notifications serialized as JSON lines).
    write_tx: mpsc::Sender<Vec<u8>>,
    /// Pending requests waiting for responses, keyed by request ID.
    pending: std::sync::Arc<Mutex<HashMap<u64, PendingRequest>>>,
    /// Handle to the spawned child process.
    child_pid: Option<u32>,
    /// Shutdown signal.
    shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
}

impl StdioTransport {
    /// Spawn a child process and set up stdin/stdout communication.
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: &[String],
        working_dir: Option<&str>,
    ) -> Result<Self, AppError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Set environment variables
        for env_var in env {
            if let Some(eq_pos) = env_var.find('=') {
                let key = &env_var[..eq_pos];
                let val = &env_var[eq_pos + 1..];
                cmd.env(key, val);
            }
        }

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().map_err(|e| {
            AppError::Mcp(format!(
                "Failed to spawn MCP server '{}': {}",
                command, e
            ))
        })?;

        let child_pid = child.id();

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::Mcp("Failed to capture child stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Mcp("Failed to capture child stdout".into()))?;

        let pending: std::sync::Arc<Mutex<HashMap<u64, PendingRequest>>> =
            std::sync::Arc::new(Mutex::new(HashMap::new()));

        // Writer task: sends data to stdin
        let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(64);
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        {
            let mut stdin = stdin;
            let mut shutdown_rx_writer = shutdown_rx.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        msg = write_rx.recv() => {
                            match msg {
                                Some(data) => {
                                    if let Err(e) = stdin.write_all(&data).await {
                                        tracing::error!("MCP stdin write error: {}", e);
                                        break;
                                    }
                                    if let Err(e) = stdin.flush().await {
                                        tracing::error!("MCP stdin flush error: {}", e);
                                        break;
                                    }
                                }
                                None => break,
                            }
                        }
                        _ = shutdown_rx_writer.changed() => {
                            break;
                        }
                    }
                }
            });
        }

        // Reader task: reads lines from stdout and dispatches to pending requests
        {
            let pending_clone = pending.clone();
            let mut shutdown_rx_reader = shutdown_rx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                loop {
                    tokio::select! {
                        line = lines.next_line() => {
                            match line {
                                Ok(Some(line)) => {
                                    Self::handle_stdout_line(&line, &pending_clone).await;
                                }
                                Ok(None) => {
                                    tracing::debug!("MCP server stdout closed");
                                    break;
                                }
                                Err(e) => {
                                    tracing::error!("MCP stdout read error: {}", e);
                                    break;
                                }
                            }
                        }
                        _ = shutdown_rx_reader.changed() => {
                            break;
                        }
                    }
                }
            });
        }

        // Background task: wait for child exit and log it
        {
            let shutdown_rx_child = shutdown_rx;
            tokio::spawn(async move {
                Self::monitor_child(child, shutdown_rx_child).await;
            });
        }

        Ok(StdioTransport {
            write_tx,
            pending,
            child_pid,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    async fn handle_stdout_line(
        line: &str,
        pending: &std::sync::Arc<Mutex<HashMap<u64, PendingRequest>>>,
    ) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }

        match serde_json::from_str::<JsonRpcResponse>(trimmed) {
            Ok(response) => {
                if let Some(id) = response.id {
                    let mut map = pending.lock().await;
                    if let Some(req) = map.remove(&id) {
                        let _ = req.sender.send(response);
                    } else {
                        tracing::warn!(
                            "Received JSON-RPC response for unknown id {}",
                            id
                        );
                    }
                }
                // Notifications from server (no id) are ignored for now
            }
            Err(e) => {
                tracing::trace!(
                    "Non-JSON-RPC line from MCP server: {} (parse error: {})",
                    trimmed,
                    e
                );
            }
        }
    }

    async fn monitor_child(mut child: Child, mut shutdown_rx: tokio::sync::watch::Receiver<bool>) {
        tokio::select! {
            status = child.wait() => {
                match status {
                    Ok(s) => tracing::info!("MCP server process exited with: {}", s),
                    Err(e) => tracing::error!("Error waiting for MCP server process: {}", e),
                }
            }
            _ = shutdown_rx.changed() => {
                tracing::debug!("Shutting down MCP child process");
                let _ = child.kill().await;
            }
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    async fn send_request(
        &self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse, AppError> {
        let mut data =
            serde_json::to_vec(request).map_err(|e| AppError::Mcp(format!("Serialize: {}", e)))?;
        data.push(b'\n');

        // Register pending request before sending
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().await;
            map.insert(request.id, PendingRequest { sender: tx });
        }

        self.write_tx.send(data).await.map_err(|_| {
            AppError::Mcp("MCP transport write channel closed".into())
        })?;

        // Wait for response with timeout
        let response = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .map_err(|_| AppError::Mcp("MCP request timed out after 30s".into()))?
            .map_err(|_| AppError::Mcp("MCP response channel dropped".into()))?;

        Ok(response)
    }

    async fn send_notification(
        &self,
        notification: &JsonRpcNotification,
    ) -> Result<(), AppError> {
        let mut data = serde_json::to_vec(notification)
            .map_err(|e| AppError::Mcp(format!("Serialize notification: {}", e)))?;
        data.push(b'\n');

        self.write_tx.send(data).await.map_err(|_| {
            AppError::Mcp("MCP transport write channel closed".into())
        })?;

        Ok(())
    }

    async fn shutdown(&self) -> Result<(), AppError> {
        if let Some(ref tx) = self.shutdown_tx {
            let _ = tx.send(true);
        }
        Ok(())
    }

    fn pid(&self) -> Option<u32> {
        self.child_pid
    }
}

// ---------------------------------------------------------------------------
// SSE Transport
// ---------------------------------------------------------------------------

/// SSE transport: communicates with a remote MCP server via HTTP SSE.
pub struct SseTransport {
    /// The SSE endpoint URL.
    url: String,
    /// HTTP headers for requests.
    headers: HashMap<String, String>,
    /// HTTP client.
    client: reqwest::Client,
    /// Endpoint URL for sending JSON-RPC requests (discovered from SSE).
    /// In the standard MCP SSE flow, the server sends an `endpoint` event
    /// that tells the client where to POST JSON-RPC messages.
    post_url: Mutex<Option<String>>,
}

impl SseTransport {
    /// Create a new SSE transport.
    pub fn new(url: &str, headers: &HashMap<String, String>) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| AppError::Mcp(format!("Failed to create HTTP client: {}", e)))?;

        Ok(SseTransport {
            url: url.to_string(),
            headers: headers.clone(),
            client,
            post_url: Mutex::new(None),
        })
    }

    /// Connect to the SSE endpoint and discover the POST URL.
    pub async fn connect(&self) -> Result<(), AppError> {
        let mut request = self.client.get(&self.url);
        for (key, value) in &self.headers {
            request = request.header(key.as_str(), value.as_str());
        }
        request = request.header("Accept", "text/event-stream");

        let response = request.send().await.map_err(|e| {
            AppError::Mcp(format!("SSE connection failed to {}: {}", self.url, e))
        })?;

        if !response.status().is_success() {
            return Err(AppError::Mcp(format!(
                "SSE server returned status {}",
                response.status()
            )));
        }

        // Read the initial SSE events to discover the POST endpoint.
        // The server should send an `endpoint` event with the URL.
        let body = response.text().await.map_err(|e| {
            AppError::Mcp(format!("Failed to read SSE response: {}", e))
        })?;

        for line in body.lines() {
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data: ") {
                // Try to parse as endpoint discovery
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(endpoint) = value.get("endpoint").and_then(|v| v.as_str()) {
                        let full_url = if endpoint.starts_with("http") {
                            endpoint.to_string()
                        } else {
                            // Relative URL: resolve against the SSE URL
                            format!(
                                "{}{}",
                                self.url.trim_end_matches(|c: char| c != '/'),
                                endpoint
                            )
                        };
                        let mut post_url = self.post_url.lock().await;
                        *post_url = Some(full_url);
                        break;
                    }
                }
            }
        }

        // If no endpoint event was received, fall back to the same URL
        let mut post_url = self.post_url.lock().await;
        if post_url.is_none() {
            *post_url = Some(self.url.clone());
        }

        Ok(())
    }

    /// Get the POST URL (either discovered via SSE or the original URL).
    async fn get_post_url(&self) -> Result<String, AppError> {
        let post_url = self.post_url.lock().await;
        post_url.clone().ok_or_else(|| {
            AppError::Mcp("SSE transport not connected: no POST URL available".into())
        })
    }
}

#[async_trait::async_trait]
impl McpTransport for SseTransport {
    async fn send_request(
        &self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse, AppError> {
        let post_url = self.get_post_url().await?;

        let mut http_request = self.client.post(&post_url);
        for (key, value) in &self.headers {
            http_request = http_request.header(key.as_str(), value.as_str());
        }
        http_request = http_request.header("Content-Type", "application/json");

        let body = serde_json::to_string(request)
            .map_err(|e| AppError::Mcp(format!("Serialize request: {}", e)))?;

        let response = http_request.body(body).send().await.map_err(|e| {
            AppError::Mcp(format!("SSE POST request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Mcp(format!(
                "SSE server returned status {}: {}",
                status, body
            )));
        }

        let text = response.text().await.map_err(|e| {
            AppError::Mcp(format!("Failed to read SSE response: {}", e))
        })?;

        let rpc_response: JsonRpcResponse = serde_json::from_str(&text).map_err(|e| {
            AppError::Mcp(format!("Failed to parse SSE JSON-RPC response: {}", e))
        })?;

        Ok(rpc_response)
    }

    async fn send_notification(
        &self,
        notification: &JsonRpcNotification,
    ) -> Result<(), AppError> {
        let post_url = self.get_post_url().await?;

        let mut http_request = self.client.post(&post_url);
        for (key, value) in &self.headers {
            http_request = http_request.header(key.as_str(), value.as_str());
        }
        http_request = http_request.header("Content-Type", "application/json");

        let body = serde_json::to_string(notification)
            .map_err(|e| AppError::Mcp(format!("Serialize notification: {}", e)))?;

        let _ = http_request.body(body).send().await.map_err(|e| {
            AppError::Mcp(format!("SSE POST notification failed: {}", e))
        })?;

        Ok(())
    }

    async fn shutdown(&self) -> Result<(), AppError> {
        // SSE transport doesn't own a process, just drop connections
        Ok(())
    }

    fn pid(&self) -> Option<u32> {
        None
    }
}
