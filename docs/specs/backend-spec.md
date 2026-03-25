# Backend Specification

> Version: 1.3.3 | Last Updated: 2026-03-25 | Author: architect

---

## Table of Contents

1. [Crate Structure](#1-crate-structure)
2. [Rust Coding Conventions](#2-rust-coding-conventions)
3. [cratebay-core Design](#3-cratebay-core-design)
4. [Tauri Command Design Patterns](#4-tauri-command-design-patterns)
5. [CLI Design](#5-cli-design)
6. [Docker Integration](#6-docker-integration)
7. [Dependency Management](#7-dependency-management)
8. [Performance Requirements](#8-performance-requirements)

---

## 1. Crate Structure

### 1.1 Overview

The backend is organized as a Cargo workspace with 4 crates:

```
crates/
├── cratebay-core/           # Shared library — business logic
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           # Public API exports
│       ├── engine.rs        # Container engine ensure (runtime auto-start + locking)
│       ├── docker.rs        # Docker connection management
│       ├── container.rs     # Container CRUD operations
│       ├── llm_proxy.rs     # LLM request proxy with streaming
│       ├── storage.rs       # SQLite storage layer
│       ├── mcp/             # MCP Client (stdio + SSE)
│       │   ├── mod.rs       # Public API exports
│       │   ├── config.rs    # Configuration loading (.mcp.json, DB, env expansion)
│       │   ├── jsonrpc.rs   # JSON-RPC 2.0 message types and serialization
│       │   ├── manager.rs   # McpManager — multi-server lifecycle management
│       │   └── transport.rs # Transport layer (stdio + SSE)
│       ├── audit.rs         # Audit logging
│       ├── validation.rs    # Input validation
│       ├── error.rs         # Error types (AppError)
│       ├── models.rs        # Shared data models
│       └── runtime/         # Built-in runtime management
│           ├── mod.rs       # Platform dispatch
│           ├── macos.rs     # VZ.framework implementation
│           ├── linux.rs     # KVM/QEMU implementation
│           └── windows.rs   # WSL2 implementation
│
├── cratebay-gui/            # Tauri v2 desktop app
│   └── src-tauri/
│       ├── Cargo.toml
│       ├── tauri.conf.json  # Tauri configuration
│       ├── capabilities/    # Tauri v2 permission capabilities
│       └── src/
│           ├── main.rs      # App entry point, plugin registration
│           ├── state.rs     # AppState definition
│           ├── commands/    # Tauri command modules
│           │   ├── mod.rs   # Command registration
│           │   ├── container.rs
│           │   ├── llm.rs
│           │   ├── storage.rs
│           │   ├── mcp.rs
│           │   └── system.rs
│           └── events.rs    # Tauri Event definitions
│
├── cratebay-cli/            # CLI binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # Entry point
│       └── commands/        # Clap subcommand handlers
│           ├── mod.rs
│           ├── container.rs
│           ├── image.rs
│           └── system.rs
│
└── cratebay-mcp/            # MCP Server binary
    ├── Cargo.toml
    └── src/
        ├── main.rs          # MCP Server entry, tool registration
        ├── tools.rs         # Tool implementations
        └── sandbox.rs       # Sandbox-specific logic
```

### 1.2 Dependency Direction

```
cratebay-gui ──→ cratebay-core
cratebay-cli ──→ cratebay-core
cratebay-mcp ──→ cratebay-core

(No circular dependencies. Binaries depend on core only.)
```

---

## 2. Rust Coding Conventions

### 2.1 Error Handling

**Rule: Use `thiserror` + `Result<T, AppError>`. Never `unwrap()` in production code.**

```rust
// cratebay-core/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Docker error: {0}")]
    Docker(#[from] bollard::errors::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("LLM proxy error: {0}")]
    LlmProxy(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

// For Tauri command compatibility
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
```

**Usage pattern:**

```rust
// Good: Propagate with ?
pub async fn get_container(docker: &Docker, id: &str) -> Result<ContainerInfo, AppError> {
    let inspect = docker.inspect_container(id, None).await?;
    Ok(ContainerInfo::from(inspect))
}

// Good: Custom error context
pub fn validate_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::Validation("Name cannot be empty".into()));
    }
    Ok(())
}

// BAD: Never do this
let result = something.unwrap(); // PROHIBITED
```

### 2.2 Async Patterns

**Rule: Use tokio runtime. Never block an async context with synchronous operations.**

```rust
// Good: Async throughout
pub async fn list_containers(docker: &Docker) -> Result<Vec<ContainerInfo>, AppError> {
    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await?;
    Ok(containers.into_iter().map(ContainerInfo::from).collect())
}

// Good: Use spawn_blocking for CPU-heavy work
pub async fn encrypt_key(plaintext: &str, key: &[u8]) -> Result<Vec<u8>, AppError> {
    let plaintext = plaintext.to_string();
    let key = key.to_vec();
    tokio::task::spawn_blocking(move || {
        // CPU-intensive encryption
        do_encrypt(&plaintext, &key)
    })
    .await
    .map_err(|e| AppError::Runtime(e.to_string()))?
}

// BAD: Blocking in async context
pub async fn bad_example() {
    std::thread::sleep(Duration::from_secs(1)); // PROHIBITED
    std::fs::read_to_string("file.txt"); // PROHIBITED in async
}
```

### 2.3 Mutex Usage

**Rule: Use `lock_or_recover()` pattern. Never `.lock().unwrap()`.**

```rust
use std::sync::{Arc, Mutex};

/// Extension trait for safe mutex locking
pub trait MutexExt<T> {
    fn lock_or_recover(&self) -> Result<std::sync::MutexGuard<'_, T>, AppError>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_or_recover(&self) -> Result<std::sync::MutexGuard<'_, T>, AppError> {
        self.lock().map_err(|e| {
            // Attempt to recover from poisoned mutex
            tracing::error!("Mutex poisoned, attempting recovery: {}", e);
            AppError::Runtime(format!("Mutex poisoned: {}", e))
        })
    }
}

// Usage
pub fn get_setting(db: &Arc<Mutex<Connection>>, key: &str) -> Result<String, AppError> {
    let conn = db.lock_or_recover()?;
    // ... query
    Ok(value)
}
```

### 2.4 Platform-Specific Code

**Rule: All platform code gated with `#[cfg(target_os = "...")]`.**

```rust
// In mod.rs — platform dispatch
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

/// Platform-agnostic trait
pub trait RuntimeManager: Send + Sync {
    async fn detect(&self) -> Result<RuntimeState, AppError>;
    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError>;
    async fn start(&self) -> Result<(), AppError>;
    async fn stop(&self) -> Result<(), AppError>;
    async fn health_check(&self) -> Result<HealthStatus, AppError>;
    fn docker_socket_path(&self) -> PathBuf;
    async fn resource_usage(&self) -> Result<ResourceUsage, AppError>;
}

/// Factory function
pub fn create_runtime_manager() -> Box<dyn RuntimeManager> {
    #[cfg(target_os = "macos")]
    { Box::new(macos::MacOSRuntime::new()) }
    #[cfg(target_os = "linux")]
    { Box::new(linux::LinuxRuntime::new()) }
    #[cfg(target_os = "windows")]
    { Box::new(windows::WindowsRuntime::new()) }
}
```

### 2.5 Module Organization

**Rule: One responsibility per module. Public API through `lib.rs` re-exports.**

```rust
// cratebay-core/src/lib.rs
pub mod container;
pub mod docker;
pub mod llm_proxy;
pub mod storage;
pub mod mcp;
pub mod audit;
pub mod validation;
pub mod error;
pub mod models;
pub mod runtime;

// Re-export commonly used types
pub use error::AppError;
pub use models::*;
```

### 2.6 Logging

**Rule: Use `tracing` with structured fields.**

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(docker), fields(container_id = %id))]
pub async fn start_container(docker: &Docker, id: &str) -> Result<(), AppError> {
    info!("Starting container");
    docker.start_container::<String>(id, None).await?;
    info!("Container started successfully");
    Ok(())
}
```

### 2.7 Testing Conventions

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_container_list_empty() {
        // Arrange
        let docker = mock_docker();

        // Act
        let result = list_containers(&docker).await;

        // Assert
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
```

---

## 3. cratebay-core Design

### 3.1 engine.rs — Container Engine Ensure

`engine.rs` is the single entry-point used by both **GUI (Tauri)** and **CLI**
to guarantee a responsive Docker client.

**Runtime strategy for backend contributors and AI agents:**

- The **built-in runtime** is the **primary product path**.
- **Podman is fallback / escape hatch only**, not a co-equal roadmap track.
- Container and image operations MUST stay on the **Docker-compatible API boundary** (`bollard`, Docker socket/host semantics).
- When runtime, container, or image workflows break, fix the built-in runtime path first before expanding Podman-specific behavior.
- Do **not** add Podman-specific product flows or architectural branches unless a human maintainer explicitly approves that work.

Operational behavior:

- Prefer **external Docker** when available (including `DOCKER_HOST`)
- Otherwise **provision + start built-in runtime**, then wait for Docker
- Use a **cross-process lock** (`~/.cratebay/runtime/engine.lock`) so GUI + CLI
  don't start/provision concurrently
- Runtime is **not** automatically stopped when the GUI exits
- Provider override via `CRATEBAY_ENGINE_PROVIDER`:
  - `auto` (default): external Docker → built-in runtime → (best-effort) Podman fallback
  - `builtin`: force built-in runtime only (no Podman fallback)
  - `podman`: force Podman only (start Podman machine/service if needed)

`CRATEBAY_ENGINE_PROVIDER` is a compatibility, recovery, testing, and operator-override mechanism. It does **not** change the product strategy: built-in runtime remains the default roadmap path, and Podman remains a secondary fallback mode.

```rust
use bollard::Docker;
use std::sync::Arc;
use std::time::Duration;

use crate::error::AppError;
use crate::runtime::{self, RuntimeManager, RuntimeState};

pub struct EnsureOptions {
    pub lock_wait_timeout: Duration,
    pub docker_wait_timeout: Duration,
    pub runtime_detect_timeout: Duration,
    pub runtime_start_timeout: Duration,
    pub runtime_provision_timeout: Duration,
    pub podman_start_timeout: Duration,
    pub on_provision_progress: Option<Box<dyn Fn(runtime::ProvisionProgress) + Send>>,
}

impl Default for EnsureOptions {
    fn default() -> Self {
        Self {
            lock_wait_timeout: Duration::from_secs(10 * 60),
            docker_wait_timeout: Duration::from_secs(45),
            runtime_detect_timeout: Duration::from_secs(10),
            runtime_start_timeout: Duration::from_secs(90),
            runtime_provision_timeout: Duration::from_secs(30 * 60),
            podman_start_timeout: Duration::from_secs(120),
            on_provision_progress: None,
        }
    }
}

pub async fn ensure_docker(
    runtime: &dyn RuntimeManager,
    options: EnsureOptions,
) -> Result<Arc<Docker>, AppError> {
    // (implementation omitted — see crates/cratebay-core/src/engine.rs)
    unimplemented!()
}
```

### 3.2 docker.rs — Docker Connection Management

`docker.rs` manages Docker client connection with multi-platform socket detection
but does **not** start/provision the runtime. Runtime lifecycle is handled by
`engine.rs`.

```rust
use bollard::Docker;

pub async fn connect() -> Result<Docker, AppError> {
    // (implementation omitted — see crates/cratebay-core/src/docker.rs)
    unimplemented!()
}
```

### 3.3 container.rs — Container CRUD Operations

```rust
use bollard::Docker;
use bollard::container::*;

/// List all containers (optionally filtered)
pub async fn list(
    docker: &Docker,
    all: bool,
    filters: Option<ContainerListFilters>,
) -> Result<Vec<ContainerInfo>, AppError> { /* ... */ }

/// Create a new container from a template or custom config
pub async fn create(
    docker: &Docker,
    request: ContainerCreateRequest,
) -> Result<ContainerInfo, AppError> { /* ... */ }

/// Start a stopped container
pub async fn start(docker: &Docker, id: &str) -> Result<(), AppError> { /* ... */ }

/// Stop a running container
pub async fn stop(docker: &Docker, id: &str) -> Result<(), AppError> { /* ... */ }

/// Remove a container (must be stopped first)
pub async fn delete(docker: &Docker, id: &str, force: bool) -> Result<(), AppError> { /* ... */ }

/// Execute a command inside a running container
pub async fn exec(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    working_dir: Option<String>,
) -> Result<ExecResult, AppError> { /* ... */ }

/// Stream exec output via callback
pub async fn exec_stream(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    on_output: impl Fn(ExecStreamChunk) + Send + 'static,
) -> Result<i64, AppError> { /* ... */ }

/// Get container logs
pub async fn logs(
    docker: &Docker,
    id: &str,
    options: LogsOptions,
) -> Result<Vec<LogEntry>, AppError> { /* ... */ }

/// Inspect a container (detailed info)
pub async fn inspect(
    docker: &Docker,
    id: &str,
) -> Result<ContainerDetail, AppError> { /* ... */ }
```

### 3.3 llm_proxy.rs — LLM Request Proxy

The LLM proxy module handles streaming chat requests to multiple LLM providers, automatically selecting the correct API format based on the provider's `api_format` configuration.

#### 3.3.1 API Format Dispatch

```rust
use reqwest::Client;
use tokio::sync::mpsc;

/// Supported API format types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    Anthropic,              // Anthropic Messages API
    OpenaiResponses,        // OpenAI Responses API (supports reasoning effort)
    OpenaiCompletions,      // OpenAI Chat Completions API
}

/// Proxy an LLM request through the backend, streaming tokens
pub async fn stream_chat(
    client: &Client,
    db: &Arc<Mutex<Connection>>,
    provider_id: &str,
    model_id: &str,
    messages: Vec<ChatMessage>,
    options: Option<LlmOptions>,
    tx: mpsc::Sender<StreamChunk>,
) -> Result<UsageStats, AppError> {
    // 1. Load provider config
    let provider = storage::get_provider(db, provider_id)?;

    // 2. Decrypt API key
    let api_key = storage::decrypt_api_key(db, provider_id)?;

    // 3. Build dual authentication headers
    let headers = build_auth_headers(&api_key);

    // 4. Build format-specific request body
    let (url, body) = match provider.api_format {
        ApiFormat::Anthropic => build_anthropic_request(
            &provider, model_id, &messages, &options,
        )?,
        ApiFormat::OpenaiResponses => build_openai_responses_request(
            &provider, model_id, &messages, &options,
        )?,
        ApiFormat::OpenaiCompletions => build_openai_completions_request(
            &provider, model_id, &messages, &options,
        )?,
    };

    // 5. Send request and stream response
    let mut response = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::LlmProxy(e.to_string()))?;

    // 6. Parse SSE chunks based on format
    let mut usage = UsageStats::default();
    while let Some(chunk) = response.chunk().await? {
        let parsed = parse_sse_chunk(&chunk, &provider.api_format)?;
        match parsed {
            SseData::Token(token) => {
                tx.send(StreamChunk::Token(token)).await.ok();
            }
            SseData::ToolCall(tc) => {
                tx.send(StreamChunk::ToolCall(tc)).await.ok();
            }
            SseData::Done(stats) => {
                usage = stats;
            }
            SseData::Error(err) => {
                return Err(AppError::LlmProxy(err));
            }
        }
    }

    // 7. Send completion signal
    tx.send(StreamChunk::Done(usage.clone())).await.ok();
    Ok(usage)
}
```

#### 3.3.2 Dual Header Authentication

All outgoing requests include both `Authorization: Bearer` and `x-api-key` headers:

```rust
use reqwest::header::HeaderMap;

/// Build authentication headers with dual-header strategy.
/// Both headers are always sent, ensuring compatibility with:
/// - OpenAI-compatible providers (use Authorization: Bearer)
/// - Anthropic (uses x-api-key)
/// - Third-party providers (may use either)
fn build_auth_headers(api_key: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Bearer {}", api_key).parse().unwrap(),
    );
    headers.insert(
        "x-api-key",
        api_key.parse().unwrap(),
    );
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers
}
```

#### 3.3.3 Format-Specific Request Builders

**Anthropic Messages API:**
```rust
fn build_anthropic_request(
    provider: &LlmProvider,
    model_id: &str,
    messages: &[ChatMessage],
    options: &Option<LlmOptions>,
) -> Result<(String, serde_json::Value), AppError> {
    // Extract system message from messages array (Anthropic uses top-level system param)
    let system_content = messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // Filter out system messages from the messages array
    let non_system_messages: Vec<_> = messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| anthropic_message(m))
        .collect();

    let url = format!("{}/v1/messages", provider.api_base.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model_id,
        "max_tokens": options.as_ref().and_then(|o| o.max_tokens).unwrap_or(4096),
        "system": system_content,
        "messages": non_system_messages,
        "stream": true,
    });

    Ok((url, body))
}
```

**OpenAI Responses API:**
```rust
fn build_openai_responses_request(
    provider: &LlmProvider,
    model_id: &str,
    messages: &[ChatMessage],
    options: &Option<LlmOptions>,
) -> Result<(String, serde_json::Value), AppError> {
    let url = format!("{}/v1/responses", provider.api_base.trim_end_matches('/'));
    let mut body = serde_json::json!({
        "model": model_id,
        "input": messages.iter().map(|m| openai_message(m)).collect::<Vec<_>>(),
        "stream": true,
    });

    // Add reasoning effort if provided (ONLY supported in this format)
    if let Some(ref opts) = options {
        if let Some(ref effort) = opts.reasoning_effort {
            body["reasoning"] = serde_json::json!({ "effort": effort });
        }
    }

    Ok((url, body))
}
```

**OpenAI Chat Completions:**
```rust
fn build_openai_completions_request(
    provider: &LlmProvider,
    model_id: &str,
    messages: &[ChatMessage],
    options: &Option<LlmOptions>,
) -> Result<(String, serde_json::Value), AppError> {
    let url = format!("{}/v1/chat/completions", provider.api_base.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model_id,
        "messages": messages.iter().map(|m| openai_message(m)).collect::<Vec<_>>(),
        "stream": true,
    });

    Ok((url, body))
}
```

#### 3.3.4 Model List Fetching (`/v1/models`)

```rust
/// Fetch available models from a provider's /v1/models endpoint
pub async fn fetch_models(
    client: &Client,
    provider: &LlmProvider,
    api_key: &str,
) -> Result<Vec<ModelInfo>, AppError> {
    let url = format!("{}/v1/models", provider.api_base.trim_end_matches('/'));
    let headers = build_auth_headers(api_key);

    let response = client
        .get(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| AppError::LlmProxy(format!("Failed to fetch models: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::LlmProxy(format!(
            "Models endpoint returned {}: {}",
            response.status(),
            response.text().await.unwrap_or_default()
        )));
    }

    let body: serde_json::Value = response.json().await
        .map_err(|e| AppError::LlmProxy(format!("Invalid models response: {}", e)))?;

    // Parse OpenAI-compatible /v1/models response format
    let models = body["data"]
        .as_array()
        .ok_or_else(|| AppError::LlmProxy("Invalid models response: missing data array".into()))?
        .iter()
        .filter_map(|m| {
            Some(ModelInfo {
                id: m["id"].as_str()?.to_string(),
                name: m["id"].as_str()?.to_string(),
            })
        })
        .collect();

    Ok(models)
}
```

### 3.4 storage.rs — SQLite Storage Layer

```rust
use rusqlite::Connection;

/// Initialize database, run migrations
pub fn init(path: &Path) -> Result<Connection, AppError> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Run pending migrations
fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    let current_version = get_schema_version(conn)?;
    for migration in MIGRATIONS.iter().filter(|m| m.version > current_version) {
        conn.execute_batch(&migration.sql)?;
        set_schema_version(conn, migration.version)?;
        tracing::info!("Applied migration v{}", migration.version);
    }
    Ok(())
}

// --- API Key operations (encrypted) ---

pub fn save_api_key(
    conn: &Connection,
    provider_id: &str,
    plaintext_key: &str,
) -> Result<(), AppError> { /* encrypt then store */ }

pub fn decrypt_api_key(
    conn: &Connection,
    provider_id: &str,
) -> Result<String, AppError> { /* load then decrypt */ }

pub fn delete_api_key(
    conn: &Connection,
    provider_id: &str,
) -> Result<(), AppError> { /* ... */ }

// --- Conversation operations ---

pub fn list_conversations(conn: &Connection) -> Result<Vec<Conversation>, AppError> { /* ... */ }
pub fn get_conversation(conn: &Connection, id: &str) -> Result<ConversationDetail, AppError> { /* ... */ }
pub fn save_message(conn: &Connection, msg: &Message) -> Result<(), AppError> { /* ... */ }

// --- Settings operations ---

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, AppError> { /* ... */ }
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> { /* ... */ }
```

### 3.5 mcp/ — MCP Client Module

The MCP Client is organized as a multi-file module under `cratebay-core/src/mcp/`:

| File | Responsibility |
|------|----------------|
| `mod.rs` | Public API re-exports (`McpManager`, `McpServerConnection`, config types) |
| `config.rs` | Configuration loading from `.mcp.json` and SQLite, environment variable expansion, server config merging |
| `jsonrpc.rs` | JSON-RPC 2.0 message types (`JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcNotification`) and serialization |
| `manager.rs` | `McpManager` — multi-server lifecycle management (register, start, stop, remove, tool discovery, tool calls) |
| `transport.rs` | Transport layer implementations (stdio child process, SSE HTTP client) |

#### McpManager

```rust
/// Manages multiple MCP server connections.
/// Thread-safe — uses interior `RwLock` for server map.
pub struct McpManager { /* ... */ }

impl McpManager {
    /// Create a new manager (empty, no servers registered)
    pub fn new() -> Self { /* ... */ }

    /// Register a server configuration (does not start it)
    pub async fn register_server(&self, server: ResolvedMcpServer) { /* ... */ }

    /// Start a registered server (connect, initialize handshake, discover tools)
    pub async fn start_server(&self, id: &str) -> Result<McpServerStatus, AppError> { /* ... */ }

    /// Stop a running server
    pub async fn stop_server(&self, id: &str) -> Result<(), AppError> { /* ... */ }

    /// Remove a server (stop if running, then unregister)
    pub async fn remove_server(&self, id: &str) { /* ... */ }

    /// List all servers with their current runtime status
    pub async fn list_servers(&self) -> Vec<McpServerStatus> { /* ... */ }

    /// Get status of a specific server
    pub async fn get_server_status(&self, id: &str) -> Result<McpServerStatus, AppError> { /* ... */ }

    /// Call a tool on a connected server
    pub async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> { /* ... */ }

    /// List all available tools across all connected servers
    pub async fn list_all_tools(&self) -> Vec<McpToolInfo> { /* ... */ }
}
```

#### Configuration Loading

```rust
/// Load MCP server config from `.mcp.json` file
pub fn load_mcp_json(path: &Path) -> Result<McpJsonConfig, AppError> { /* ... */ }

/// Expand environment variables in a string (supports ${VAR_NAME} syntax)
pub fn expand_env_vars(s: &str) -> String { /* ... */ }

/// Merge configs from .mcp.json and SQLite database
pub fn merge_server_configs(
    json_configs: &McpJsonConfig,
    db_configs: &[McpServerDbRow],
) -> Vec<ResolvedMcpServer> { /* ... */ }
```
```

### 3.6 audit.rs — Audit Logging

```rust
/// Log an auditable operation
pub fn log_action(
    conn: &Connection,
    action: AuditAction,
    target: &str,
    details: Option<&str>,
    user: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO audit_log (id, timestamp, action, target, details, user)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            Uuid::new_v4().to_string(),
            Utc::now().to_rfc3339(),
            action.as_str(),
            target,
            details,
            user,
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub enum AuditAction {
    ContainerCreate,
    ContainerStart,
    ContainerStop,
    ContainerDelete,
    ContainerExec,
    ApiKeySave,
    ApiKeyDelete,
    McpServerStart,
    McpServerStop,
    SettingsUpdate,
}
```

### 3.7 validation.rs — Input Validation

```rust
/// Validate container name (alphanumeric + hyphens, 1-64 chars)
pub fn validate_container_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::Validation(
            "Container name must be 1-64 characters".into(),
        ));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::Validation(
            "Container name must contain only alphanumeric characters, hyphens, or underscores".into(),
        ));
    }
    Ok(())
}

/// Validate file path is within workspace root (prevent path traversal)
pub fn validate_path_within_root(path: &Path, root: &Path) -> Result<PathBuf, AppError> {
    let canonical = path.canonicalize()
        .map_err(|_| AppError::Validation(format!("Invalid path: {}", path.display())))?;
    let root_canonical = root.canonicalize()
        .map_err(|_| AppError::Validation(format!("Invalid root: {}", root.display())))?;

    if !canonical.starts_with(&root_canonical) {
        return Err(AppError::PermissionDenied(
            "Path traversal detected: path is outside workspace root".into(),
        ));
    }
    Ok(canonical)
}

/// Validate resource limits
pub fn validate_resource_limits(cpu: u32, memory_mb: u64) -> Result<(), AppError> {
    if cpu == 0 || cpu > 16 {
        return Err(AppError::Validation("CPU cores must be 1-16".into()));
    }
    if memory_mb < 256 || memory_mb > 65536 {
        return Err(AppError::Validation("Memory must be 256-65536 MB".into()));
    }
    Ok(())
}
```

---

## 4. Tauri Command Design Patterns

### 4.1 Command Grouping

Commands are organized by domain module:

| Module | File | Commands |
|--------|------|----------|
| Container | `commands/container.rs` | templates, list, create, start, stop, delete, exec, exec_stream, logs, inspect |
| LLM | `commands/llm.rs` | proxy_stream, proxy_cancel, provider_list, provider_create, provider_update, provider_delete, provider_test, models_fetch, models_list, models_toggle |
| Storage | `commands/storage.rs` | settings_get, settings_update, api_key_save, api_key_delete, conversation_list, conversation_get_messages, conversation_create, conversation_delete, conversation_save_message, conversation_update_title |
| MCP | `commands/mcp.rs` | server_list, server_add, server_remove, server_start, server_stop, client_call_tool, client_list_tools, export_client_config |
| System | `commands/system.rs` | system_info, docker_status, runtime_status, runtime_start, runtime_stop |

### 4.2 AppState Structure

```rust
// src-tauri/src/state.rs
use bollard::Docker;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use cratebay_core::mcp::McpManager;
use cratebay_core::runtime::RuntimeManager;

pub struct AppState {
    /// Docker client (optional — Docker may not be available).
    /// Wrapped in `Arc<Mutex<...>>` so it can be updated dynamically
    /// after the built-in runtime starts Docker.
    pub docker: Arc<Mutex<Option<Arc<Docker>>>>,

    /// SQLite database connection
    pub db: Arc<Mutex<Connection>>,

    /// Application data directory (~/.cratebay/)
    pub data_dir: PathBuf,

    /// Active LLM streaming session cancellation tokens.
    /// Key is the channel_id used to identify the streaming session.
    pub llm_cancel_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,

    /// Built-in container runtime manager (platform-specific)
    pub runtime: Arc<dyn RuntimeManager>,

    /// MCP server connection manager
    pub mcp_manager: Arc<McpManager>,
}

impl AppState {
    /// Get a clone of the Docker client Arc, or error if unavailable.
    pub fn require_docker(&self) -> Result<Arc<Docker>, AppError> {
        let guard = self.docker.lock().map_err(|e| {
            AppError::Runtime(format!("Docker state mutex poisoned: {}", e))
        })?;
        guard
            .clone()
            .ok_or_else(|| AppError::Docker(/* connection unavailable */))
    }

    /// Ensure Docker is available.
    ///
    /// If no external Docker is reachable, this will auto-start the built-in runtime
    /// via `cratebay_core::engine::ensure_docker` and then cache the connected client
    /// into `AppState.docker`.
    pub async fn ensure_docker(&self) -> Result<Arc<Docker>, AppError> {
        if let Ok(d) = self.require_docker() {
            return Ok(d);
        }

        let docker = cratebay_core::engine::ensure_docker(self.runtime.as_ref(), Default::default())
            .await?;
        self.set_docker(Some(docker.clone()));
        Ok(docker)
    }

    /// Update the Docker client (e.g., after runtime starts or stops).
    pub fn set_docker(&self, docker: Option<Arc<Docker>>) {
        if let Ok(mut guard) = self.docker.lock() {
            *guard = docker;
        }
    }

    /// Check if Docker is currently available.
    pub fn has_docker(&self) -> bool {
        self.docker.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}
```

### 4.3 Command Pattern

```rust
use tauri::State;
use specta::specta;

#[tauri::command]
#[specta::specta]
pub async fn container_list(
    state: State<'_, AppState>,
    filters: Option<ContainerListFilters>,
) -> Result<Vec<ContainerInfo>, AppError> {
    let docker = state.ensure_docker().await?;
    cratebay_core::container::list(&docker, true, filters).await
}

#[tauri::command]
#[specta::specta]
pub async fn container_create(
    state: State<'_, AppState>,
    request: ContainerCreateRequest,
) -> Result<ContainerInfo, AppError> {
    // Validate input
    validation::validate_container_name(&request.name)?;
    if let (Some(cpu), Some(mem)) = (request.cpu_cores, request.memory_mb) {
        validation::validate_resource_limits(cpu, mem)?;
    }

    // Execute
    let result = cratebay_core::container::create(&state.docker, request).await?;

    // Audit
    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, AuditAction::ContainerCreate, &result.id, None, "user")?;

    Ok(result)
}
```

### 4.4 Streaming via Tauri Event

For long-running operations (LLM streaming, exec output), use Tauri Events:

```rust
use tauri::{AppHandle, Emitter};

#[tauri::command]
#[specta::specta]
pub async fn llm_proxy_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    channel_id: String,
    provider_id: String,
    messages: Vec<ChatMessage>,
) -> Result<(), AppError> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamChunk>(100);
    let event_name = format!("llm:stream:{}", channel_id);

    // Spawn streaming task
    let db = state.db.clone();
    let client = reqwest::Client::new();
    tokio::spawn(async move {
        if let Err(e) = cratebay_core::llm_proxy::stream_chat(
            &client, &db, &provider_id, messages, tx,
        ).await {
            tracing::error!("LLM stream error: {}", e);
        }
    });

    // Forward chunks as Tauri Events
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(chunk) = rx.recv().await {
            let _ = app_clone.emit(&event_name, &chunk);
        }
    });

    Ok(())
}
```

### 4.5 Error Response Format

All Tauri commands return `Result<T, AppError>`. Errors are serialized as strings to the frontend:

```typescript
// Frontend error handling
try {
    const containers = await invoke('container_list', { filters });
} catch (error) {
    // error is a string: "Docker error: connection refused"
    console.error('Command failed:', error);
}
```

**Structured error format** (for frontend consumption):

```rust
// If more structure is needed, use a custom serialization:
#[derive(Serialize)]
pub struct CommandError {
    pub code: String,      // "DOCKER_ERROR", "VALIDATION_ERROR", etc.
    pub message: String,   // Human-readable message
    pub details: Option<String>,
}
```

### 4.6 Command Registration

```rust
// src-tauri/src/main.rs
use tauri_specta::{collect_commands, Builder};

fn main() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
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
            commands::system::runtime_start,
            commands::system::runtime_stop,
        ]);

    // Export TypeScript bindings
    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/bindings.ts",
        )
        .expect("Failed to export TypeScript bindings");

    tauri::Builder::default()
        .plugin(builder.into_plugin())
        .manage(AppState::new())
        .run(tauri::generate_context!())
        .expect("Error running CrateBay");
}
```

---

## 5. CLI Design

### 5.1 Structure

The CLI uses `clap` with derive macros for subcommand parsing:

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cratebay")]
#[command(about = "CrateBay CLI — Container management from the command line")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Docker socket path (overrides auto-detection)
    #[arg(long, global = true)]
    pub docker_host: Option<String>,

    /// Output format
    #[arg(long, global = true, default_value = "table")]
    pub format: OutputFormat,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Container operations
    #[command(subcommand)]
    Container(ContainerCommands),

    /// Image operations
    #[command(subcommand)]
    Image(ImageCommands),

    /// System information
    System(SystemCommand),
}
```

### 5.2 Subcommands

```
cratebay container list [--all] [--format json|table]
cratebay container create <name> --image <image> [--cpu <cores>] [--memory <mb>]
cratebay container start <id>
cratebay container stop <id>
cratebay container delete <id> [--force]
cratebay container exec <id> -- <command...>
cratebay container logs <id> [--follow] [--tail <lines>]
cratebay container inspect <id>

cratebay image list
cratebay image search <query> [--limit <n>]
cratebay image pull <name:tag>
cratebay image delete <id>

cratebay system info
cratebay system docker-status
```

### 5.3 Output Formats

```rust
#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
}

fn print_containers(containers: Vec<ContainerInfo>, format: OutputFormat) {
    match format {
        OutputFormat::Table => {
            println!("{:<12} {:<30} {:<10} {:<20}",
                "ID", "NAME", "STATUS", "IMAGE");
            for c in containers {
                println!("{:<12} {:<30} {:<10} {:<20}",
                    &c.id[..12], c.name, c.status, c.image);
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&containers).unwrap());
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&containers).unwrap());
        }
    }
}
```

---

## 6. Docker Integration

### 6.1 bollard Usage

All Docker operations use `bollard` 0.18 with `Arc<Docker>` for shared access:

```rust
use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions,
    StartContainerOptions, StopContainerOptions,
};

/// Docker client is created once and shared via Arc
pub struct DockerManager {
    client: Arc<Docker>,
}

impl DockerManager {
    pub async fn new() -> Result<Self, AppError> {
        let runtime = crate::runtime::create_runtime_manager();
        let client = crate::engine::ensure_docker(runtime.as_ref(), Default::default()).await?;
        Ok(Self { client })
    }

    pub fn client(&self) -> &Docker {
        &self.client
    }
}
```

### 6.2 Container Creation Pattern

```rust
pub async fn create(
    docker: &Docker,
    request: ContainerCreateRequest,
) -> Result<ContainerInfo, AppError> {
    // Build Docker config from request
    let config = Config {
        image: Some(request.image.clone()),
        cmd: request.command.map(|c| vec!["/bin/sh".to_string(), "-c".to_string(), c]),
        env: request.env.clone(),
        host_config: Some(HostConfig {
            memory: request.memory_mb.map(|m| (m * 1024 * 1024) as i64),
            nano_cpus: request.cpu_cores.map(|c| (c as i64) * 1_000_000_000),
            ..Default::default()
        }),
        labels: Some(build_labels(&request)),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: &request.name,
        platform: None,
    };

    let response = docker.create_container(Some(options), config).await?;

    // Auto-start if requested
    if request.auto_start.unwrap_or(true) {
        docker.start_container::<String>(&response.id, None).await?;
    }

    inspect(docker, &response.id).await
}
```

### 6.3 Exec Streaming Pattern

```rust
use bollard::exec::{CreateExecOptions, StartExecResults};
use futures_util::StreamExt;

pub async fn exec_stream(
    docker: &Docker,
    container_id: &str,
    cmd: Vec<String>,
    on_output: impl Fn(ExecStreamChunk) + Send + 'static,
) -> Result<i64, AppError> {
    let exec = docker
        .create_exec(
            container_id,
            CreateExecOptions {
                cmd: Some(cmd),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                ..Default::default()
            },
        )
        .await?;

    let start_result = docker.start_exec(&exec.id, None).await?;

    if let StartExecResults::Attached { mut output, .. } = start_result {
        while let Some(chunk) = output.next().await {
            match chunk? {
                bollard::container::LogOutput::StdOut { message } => {
                    on_output(ExecStreamChunk::Stdout(
                        String::from_utf8_lossy(&message).to_string(),
                    ));
                }
                bollard::container::LogOutput::StdErr { message } => {
                    on_output(ExecStreamChunk::Stderr(
                        String::from_utf8_lossy(&message).to_string(),
                    ));
                }
                _ => {}
            }
        }
    }

    // Get exit code
    let inspect = docker.inspect_exec(&exec.id).await?;
    Ok(inspect.exit_code.unwrap_or(-1))
}
```

---

## 7. Dependency Management

### 7.1 Workspace Configuration

All shared dependencies are declared in the workspace `Cargo.toml`:

```toml
[workspace.dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Error handling
thiserror = "2"
anyhow = "1"      # Only in binaries, not in library

# Docker
bollard = "0.18"

# Storage
rusqlite = { version = "0.32", features = ["bundled"] }

# HTTP
reqwest = { version = "0.12", features = ["json", "stream"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Date/Time
chrono = { version = "0.4", features = ["serde"] }

# UUID
uuid = { version = "1", features = ["v4", "serde"] }

# CLI
clap = { version = "4", features = ["derive"] }

# Tauri
tauri = { version = "2", features = [] }
tauri-build = "2"
tauri-specta = { version = "2", features = ["derive", "typescript"] }
specta = { version = "2", features = ["derive"] }
specta-typescript = "0.0.7"
```

Individual crate `Cargo.toml` files reference workspace dependencies:

```toml
# crates/cratebay-core/Cargo.toml
[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
bollard = { workspace = true }
rusqlite = { workspace = true }
```

### 7.2 Dependency Budget

New dependencies must be justified:

| Criteria | Threshold |
|----------|-----------|
| Binary size impact | < 500 KB added |
| Compile time impact | < 10 s added |
| Maintenance status | Last release within 6 months |
| License | MIT, Apache-2.0, or BSD compatible |

---

## 8. Performance Requirements

### 8.1 Backend Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Docker connection init | < 1 s | Cached after first connection |
| Container list | < 500 ms | bollard API call + response mapping |
| Container create | < 3 s | Includes image pull check |
| Container exec (simple) | < 1 s | Excluding command execution time |
| SQLite query (simple) | < 10 ms | Single table, indexed lookup |
| SQLite migration | < 100 ms | Per migration step |
| LLM proxy first token | < 500 ms | Backend processing only (excludes network) |
| API key encrypt/decrypt | < 50 ms | Using system keyring |
| MCP tool call roundtrip | < 200 ms | stdio transport, excluding tool execution |

### 8.2 Resource Limits

| Resource | Limit |
|----------|-------|
| Open file descriptors | < 100 (idle) |
| Background threads | ≤ tokio default (CPU count) |
| SQLite connections | 1 (with WAL mode) |
| Docker connections | 1 (shared via Arc) |
| Memory (Rust backend) | < 50 MB RSS (idle) |

### 8.3 Benchmarking

Performance-critical paths are benchmarked with Criterion:

```rust
// benches/container_bench.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_container_list(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let docker = rt.block_on(docker::connect()).unwrap();

    c.bench_function("container_list", |b| {
        b.iter(|| {
            rt.block_on(container::list(&docker, None))
        })
    });
}

criterion_group!(benches, bench_container_list);
criterion_main!(benches);
```

Run benchmarks:

```bash
cargo bench -p cratebay-core
```
