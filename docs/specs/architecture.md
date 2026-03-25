# System Architecture

> Version: 1.1.2 | Last Updated: 2026-03-25 | Author: architect

---

## Table of Contents

1. [System Overview & Design Principles](#1-system-overview--design-principles)
2. [Architecture Diagram](#2-architecture-diagram)
3. [Crate Dependency Graph](#3-crate-dependency-graph)
4. [Hybrid Agent Architecture](#4-hybrid-agent-architecture)
5. [Communication Flow](#5-communication-flow)
6. [LLM Proxy Flow](#6-llm-proxy-flow)
7. [Data Flow & Storage](#7-data-flow--storage)
8. [Cross-Platform Strategy](#8-cross-platform-strategy)
9. [Built-in Runtime Architecture](#9-built-in-runtime-architecture)
10. [Security Model](#10-security-model)
11. [Performance Budget](#11-performance-budget)
12. [Extension Points](#12-extension-points)

---

## 1. System Overview & Design Principles

### 1.1 What is CrateBay?

CrateBay is an open-source desktop AI development control plane. It provides a **Chat-First interface** for managing containers, AI models, and MCP (Model Context Protocol) tools — all from a single desktop application with a **built-in container runtime** that requires no external Docker installation.

### 1.2 Design Principles

| Principle | Description |
|-----------|-------------|
| **Chat-First** | The primary interface is a conversational AI chat. All operations — container management, model configuration, tool invocation — are accessible through natural language. |
| **Zero-Dependency Runtime** | Users should not need to install Docker, Colima, or any external container engine. CrateBay ships a built-in VM-based runtime per platform. |
| **Hybrid Agent** | TypeScript handles AI orchestration (pi-agent-core); Rust handles tool execution, storage, and security. Each layer does what it does best. |
| **Security by Architecture** | API keys never leave the Rust backend. Container operations are sandboxed. All sensitive data is encrypted at rest. |
| **Spec-Driven Development** | Every feature starts with a specification document update before code is written. Documentation is the source of truth. |
| **Cross-Platform Parity** | macOS, Windows, and Linux are first-class citizens. Platform-specific code is isolated behind `#[cfg]` gates. |
| **Performance Budget** | Binary < 20 MB, startup < 3 s, idle RAM < 200 MB. These are CI-enforced constraints. |
| **Extensibility** | MCP protocol support for external tools. Plugin architecture reserved for future expansion. gRPC daemon planned for v2.1+. |

---

## 2. Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                        CrateBay Desktop App                          │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────── Frontend (React 19) ──────────────────────┐  │
│  │                                                                │  │
│  │  ┌───────────┐    ┌──────────────┐    ┌──────────────────┐    │  │
│  │  │ ChatPage  │───→│ pi-agent-core│───→│ streamFn         │    │  │
│  │  │ (default) │    │ (Agent Loop) │    │ (Tauri invoke)   │    │  │
│  │  └─────┬─────┘    └──────┬───────┘    └────────┬─────────┘    │  │
│  │        │                 │                      │              │  │
│  │  ┌─────▼─────┐    ┌─────▼──────┐    ┌─────────▼──────────┐   │  │
│  │  │Streamdown │    │AgentTool[] │    │  Tauri Event       │   │  │
│  │  │(Markdown  │    │(TS wrapper │    │  (streaming tokens │   │  │
│  │  │ rendering)│    │ → invoke)  │    │   + tool results)  │   │  │
│  │  └───────────┘    └────────────┘    └──────────────────────┘   │  │
│  │                                                                │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │  │
│  │  │  Zustand Stores                                         │  │  │
│  │  │  appStore │ chatStore │ containerStore │ mcpStore │ ... │  │  │
│  │  └─────────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────┬───────────────────────────────────┘  │
│                               │                                      │
│  ─ ─ ─ ─ ─ ─ ─ ─ ─ tauri-specta (auto-generated TS types) ─ ─ ─ ─  │
│                               │                                      │
│  ┌────────────────────────────▼──────────────────────────────────┐  │
│  │                      Backend (Rust)                            │  │
│  │                                                                │  │
│  │  ┌──────────────────────────────────────────────────────┐     │  │
│  │  │  Tauri Commands (cratebay-gui/src-tauri)             │     │  │
│  │  │  ┌────────────┐ ┌──────────┐ ┌───────────┐          │     │  │
│  │  │  │container.rs│ │  llm.rs  │ │storage.rs │          │     │  │
│  │  │  └─────┬──────┘ └────┬─────┘ └─────┬─────┘          │     │  │
│  │  │  ┌─────┼─────┐ ┌────┼─────┐ ┌──────┼──────┐         │     │  │
│  │  │  │ mcp.rs    │ │system.rs │ │  audit.rs   │         │     │  │
│  │  │  └───────────┘ └──────────┘ └─────────────┘         │     │  │
│  │  └──────────────────────┬───────────────────────────────┘     │  │
│  │                         │                                      │  │
│  │  ┌──────────────────────▼───────────────────────────────┐     │  │
│  │  │  cratebay-core (shared library)                      │     │  │
│  │  │  docker.rs │ container.rs │ llm_proxy.rs │ storage.rs│     │  │
│  │  │  mcp/ │ audit.rs │ validation.rs │ runtime/      │     │  │
│  │  └──────────────────────────────────────────────────────┘     │  │
│  │                                                                │  │
│  │  AppState { docker: Option<Arc<Docker>>, db, mcp_manager }    │  │
│  │  API Keys encrypted in SQLite (never sent to frontend)        │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│  Built-in Container Runtime                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │ macOS:       │  │ Linux:       │  │ Windows:     │               │
│  │ VZ.framework │  │ KVM/QEMU     │  │ WSL2         │               │
│  │ → Linux VM   │  │ → Linux VM   │  │ → Docker in  │               │
│  │ → Docker     │  │ → Docker     │  │   WSL2 distro│               │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘               │
│         └──────────────────┴─────────────────┘                       │
│                    Docker socket exposed to host                      │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 3. Crate Dependency Graph

CrateBay uses a Cargo workspace with **3 binary crates + 1 library crate**:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  cratebay-gui   │     │  cratebay-cli   │     │  cratebay-mcp   │
│  (Tauri v2 app) │     │  (CLI binary)   │     │  (MCP Server)   │
│  bin            │     │  bin            │     │  bin            │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │     cratebay-core       │
                    │     (shared library)    │
                    │                         │
                    │  ├── docker.rs          │
                    │  ├── container.rs       │
                    │  ├── llm_proxy.rs       │
                    │  ├── storage.rs         │
                    │  ├── mcp/               │
                    │  ├── audit.rs           │
                    │  └── validation.rs      │
                    └─────────────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │   External Dependencies │
                    │  bollard, rusqlite,     │
                    │  tokio, thiserror,      │
                    │  serde, reqwest         │
                    └─────────────────────────┘
```

### Crate Responsibilities

| Crate | Type | Purpose |
|-------|------|---------|
| `cratebay-core` | Library | Shared business logic: Docker operations, SQLite storage, LLM proxy, MCP client (`mcp/` module: config, jsonrpc, manager, transport), audit logging, input validation |
| `cratebay-gui` | Binary | Tauri v2 desktop application. Contains Rust backend (Tauri commands in `src-tauri/`) and React frontend (in `src/`) |
| `cratebay-cli` | Binary | Command-line interface using `clap`. Provides container/image operations for headless environments |
| `cratebay-mcp` | Binary | Standalone MCP Server. Exposes container sandbox tools via stdio transport for external AI clients |

### Workspace Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/cratebay-core",
    "crates/cratebay-gui/src-tauri",
    "crates/cratebay-cli",
    "crates/cratebay-mcp",
]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
bollard = "0.18"
rusqlite = { version = "0.32", features = ["bundled"] }
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
```

---

## 4. Hybrid Agent Architecture

CrateBay adopts a **Hybrid Agent** model where responsibilities are split across two runtime layers:

```
┌─────────────────────────────────────────────────┐
│            TypeScript Layer (Frontend)            │
│                                                   │
│  pi-agent-core                                    │
│  ├── Agent loop (message → think → act → observe)│
│  ├── Tool selection & parameter extraction        │
│  ├── Multi-turn conversation context              │
│  ├── System prompt & steering                     │
│  └── Streaming response orchestration             │
│                                                   │
│  pi-ai                                            │
│  └── Unified LLM provider abstraction             │
│                                                   │
│  AgentTool[] (TypeScript wrappers)                │
│  └── Each tool wraps a Tauri invoke call          │
└────────────────────┬────────────────────────────┘
                     │  Tauri invoke / Event
┌────────────────────▼────────────────────────────┐
│              Rust Layer (Backend)                 │
│                                                   │
│  Tool Execution                                   │
│  ├── Container CRUD (bollard → Docker)            │
│  ├── File operations (sandboxed)                  │
│  ├── Shell execution (in container)               │
│  └── MCP tool forwarding                          │
│                                                   │
│  LLM Proxy                                        │
│  ├── API key retrieval from encrypted SQLite      │
│  ├── Request construction & streaming              │
│  └── Token emission via Tauri Events              │
│                                                   │
│  Security & Storage                               │
│  ├── API key encryption/decryption                │
│  ├── Conversation persistence                     │
│  ├── Audit logging                                │
│  └── Input validation                             │
└──────────────────────────────────────────────────┘
```

### Why Hybrid?

| Concern | Layer | Rationale |
|---------|-------|-----------|
| Agent orchestration | TypeScript | pi-agent-core is a mature TS agent framework with proven patterns for tool calling, context management, and streaming |
| LLM API calls | Rust | API keys must never reach the frontend. Rust backend holds keys and proxies requests |
| Container operations | Rust | bollard (Docker SDK) is native Rust. Direct Docker socket access requires backend privileges |
| UI rendering | TypeScript | React + Streamdown provide optimized streaming markdown rendering |
| Storage | Rust | rusqlite with encryption runs in the backend process, isolated from the webview |
| MCP Client | Rust | stdio/SSE transport management is better suited to a long-lived backend process |

---

## 5. Communication Flow

### 5.1 Tauri Invoke (Request-Response)

Used for synchronous-style commands where the frontend awaits a result.

```
Frontend                          Backend
   │                                 │
   │  invoke("container_list", {})   │
   │────────────────────────────────→│
   │                                 │  bollard::Docker::list_containers()
   │                                 │
   │  Result<Vec<ContainerInfo>>     │
   │←────────────────────────────────│
   │                                 │
```

### 5.2 Tauri Event (Streaming)

Used for long-running operations that emit incremental data (LLM streaming, exec output).

```
Frontend                          Backend
   │                                 │
   │  invoke("llm_proxy_stream",     │
   │         { channel, messages })  │
   │────────────────────────────────→│
   │                                 │  reqwest::stream to LLM provider
   │                                 │
   │  Event: "llm:stream:{channel}"  │
   │  { token: "Hello" }            │
   │←────────────────────────────────│
   │                                 │
   │  Event: "llm:stream:{channel}"  │
   │  { token: " world" }           │
   │←────────────────────────────────│
   │                                 │
   │  Event: "llm:stream:{channel}"  │
   │  { done: true, usage: {...} }  │
   │←────────────────────────────────│
   │                                 │
```

### 5.3 tauri-specta Type Bridge

`tauri-specta` auto-generates TypeScript type bindings from Rust command signatures:

```rust
// Rust: src-tauri/src/commands/container.rs
#[tauri::command]
#[specta::specta]
pub async fn container_list(
    state: State<'_, AppState>,
    filters: Option<ContainerListFilters>,
) -> Result<Vec<ContainerInfo>, AppError> {
    // ...
}
```

```typescript
// Auto-generated: src/bindings.ts
export function containerList(
    filters?: ContainerListFilters
): Promise<ContainerInfo[]>;
```

This ensures **zero type drift** between frontend and backend.

---

## 6. LLM Proxy Flow

The LLM Proxy is a critical security component. API keys are stored encrypted in SQLite and never sent to the frontend webview.

```
┌──────────┐    ┌──────────────┐    ┌──────────────┐    ┌───────────┐
│ ChatPage │    │ pi-agent-core│    │ Rust Backend │    │ LLM API   │
│ (React)  │    │ (streamFn)   │    │ (llm.rs)     │    │ (OpenAI,  │
│          │    │              │    │              │    │  Anthropic)│
└────┬─────┘    └──────┬───────┘    └──────┬───────┘    └─────┬─────┘
     │                 │                    │                   │
     │  user message   │                    │                   │
     │────────────────→│                    │                   │
     │                 │                    │                   │
     │                 │  invoke            │                   │
     │                 │  "llm_proxy_stream"│                   │
     │                 │  { messages,       │                   │
     │                 │    provider_id,    │                   │
     │                 │    channel_id }    │                   │
     │                 │───────────────────→│                   │
     │                 │                    │                   │
     │                 │                    │  decrypt API key  │
     │                 │                    │  from SQLite      │
     │                 │                    │                   │
     │                 │                    │  POST /chat/      │
     │                 │                    │  completions      │
     │                 │                    │  (streaming: true)│
     │                 │                    │──────────────────→│
     │                 │                    │                   │
     │                 │                    │  SSE chunks       │
     │                 │                    │←──────────────────│
     │                 │                    │                   │
     │  Tauri Event    │                    │                   │
     │  "llm:stream:   │  Tauri Event       │                   │
     │   {channel_id}" │  (token data)      │                   │
     │←────────────────│←───────────────────│                   │
     │                 │                    │                   │
     │  Streamdown     │                    │                   │
     │  renders token  │                    │                   │
     │                 │                    │                   │
```

### Key Security Properties

1. **API keys are decrypted only in the Rust process** — never serialized to the frontend
2. **Channel-based isolation** — each streaming session uses a unique channel ID
3. **Provider abstraction** — the Rust backend normalizes different LLM provider APIs (OpenAI, Anthropic, etc.) into a unified streaming protocol

---

## 7. Data Flow & Storage

### 7.1 Storage Architecture

```
~/.cratebay/
├── cratebay.db          # SQLite database (encrypted API keys)
├── runtime/             # VM images and runtime state
│   ├── vm-image.qcow2   # Linux VM image (platform-specific)
│   └── docker.sock      # Exposed Docker socket
└── logs/                # Application logs
    └── cratebay.log
```

### 7.2 SQLite Schema Overview

| Table | Purpose | Key Fields |
|-------|---------|------------|
| `api_keys` | Encrypted LLM provider API keys | provider_id, encrypted_key, nonce |
| `conversations` | Chat session metadata | id, title, created_at, updated_at |
| `messages` | Individual chat messages | conversation_id, role, content, tool_calls |
| `container_templates` | Sandbox preset configurations | id, name, image, cpu, memory, env |
| `mcp_servers` | MCP server configurations | id, name, command, args, env, enabled |
| `ai_providers` | LLM provider settings | id, name, api_base, model, enabled |
| `audit_log` | Operation audit trail | timestamp, action, target, details |
| `settings` | Key-value application settings | key, value, updated_at |

### 7.3 Data Flow Diagram

```
User Input (Chat)
       │
       ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ chatStore    │────→│ pi-agent-core│────→│ Tauri invoke │
│ (Zustand)    │     │              │     │              │
└──────────────┘     └──────┬───────┘     └──────┬───────┘
                            │                     │
                     Tool calls            Backend logic
                            │                     │
                            ▼                     ▼
                     ┌──────────────┐     ┌──────────────┐
                     │ AgentTool    │     │ SQLite       │
                     │ (TS wrapper) │     │ (persistence)│
                     └──────┬───────┘     └──────────────┘
                            │
                     Tauri invoke
                            │
                            ▼
                     ┌──────────────┐
                     │ Docker       │
                     │ (bollard)    │
                     └──────────────┘
```

---

## 8. Cross-Platform Strategy

### 8.1 Platform Matrix

| Feature | macOS | Linux | Windows |
|---------|-------|-------|---------|
| Desktop framework | Tauri (WebKit) | Tauri (WebKitGTK) | Tauri (WebView2) |
| Container runtime | VZ.framework VM | KVM/QEMU VM | WSL2 |
| Docker socket | Unix socket | Unix socket | Named pipe / WSL2 socket |
| File sharing | VirtioFS | VirtioFS | WSL2 mount |
| System keyring | macOS Keychain | Secret Service API | Windows Credential Manager |

### 8.2 Platform Isolation Pattern

All platform-specific code is isolated using Rust's `#[cfg]` attributes:

```rust
// cratebay-core/src/runtime/mod.rs
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

pub use platform::RuntimeManager;

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "windows")]
use windows as platform;
```

### 8.3 Docker Connection Strategy

The backend attempts Docker connections in priority order:

```
1. Check DOCKER_HOST environment variable
2. Platform-specific socket detection:
   ├── macOS: ~/.colima/default/docker.sock
   │          ~/.orbstack/run/docker.sock
   │          /var/run/docker.sock
   │          ~/.docker/run/docker.sock
   ├── Linux: /var/run/docker.sock
   │          CrateBay runtime socket
   └── Windows: //./pipe/docker_engine
                WSL2 socket
3. If no socket found → start built-in runtime
4. If all fail → return error with guidance
```

---

## 9. Built-in Runtime Architecture

### 9.1 Design Goal

Users install CrateBay and run containers immediately — no Docker Desktop, no Colima, no manual configuration.

**Runtime strategy:** the built-in runtime is CrateBay's primary product path across macOS, Linux, and Windows. Podman is a secondary compatibility fallback / escape hatch, not a co-equal roadmap track. The control-plane boundary remains Docker-compatible (`bollard`, Docker socket/host semantics) regardless of whether the active engine source is external Docker, the built-in runtime, or Podman fallback.

See [runtime-spec.md](runtime-spec.md) §1.1 for the product runtime strategy and [tech-decisions.md](../references/tech-decisions.md) ADR-013 for the decision record.

### 9.2 Platform Implementations

```
┌─────────────────────────────────────────────────────────┐
│                    CrateBay Application                  │
│                                                          │
│  RuntimeManager::detect() → provision() → start()        │
└────────────┬────────────────────┬────────────────┬───────┘
             │                    │                │
    ┌────────▼────────┐  ┌───────▼───────┐  ┌────▼────────┐
    │     macOS        │  │    Linux      │  │   Windows   │
    │                  │  │               │  │             │
    │  VZ.framework    │  │  KVM/QEMU     │  │  WSL2       │
    │  VZVirtualMachine│  │  lightweight  │  │  (built-in) │
    │       │          │  │  Linux VM     │  │      │      │
    │       ▼          │  │      │        │  │      ▼      │
    │  Alpine Linux VM │  │      ▼        │  │  Ubuntu WSL │
    │  + Docker Engine │  │  Alpine Linux │  │  + Docker   │
    │       │          │  │  + Docker     │  │  Engine     │
    │       ▼          │  │      │        │  │      │      │
    │  VirtioFS share  │  │      ▼        │  │      ▼      │
    │  Unix socket     │  │  VirtioFS     │  │  WSL2 mount │
    │  exposed to host │  │  Unix socket  │  │  Socket via │
    │                  │  │  exposed      │  │  localhost   │
    └──────────────────┘  └───────────────┘  └─────────────┘
```

### 9.3 Runtime Lifecycle

```
detect → provision → start → ready → [operational] → stop

detect:    Check for existing Docker / runtime
provision: Download VM image on first run (~500 MB)
start:     Boot VM, start Docker engine inside
ready:     Docker socket exposed, health check passes
stop:      Graceful VM shutdown on app exit
```

See [runtime-spec.md](runtime-spec.md) for detailed platform-specific implementation.

---

## 10. Security Model

### 10.1 Threat Model

| Threat | Mitigation |
|--------|------------|
| API key leakage to frontend | Keys stored encrypted in SQLite; decrypted only in Rust process; never serialized to webview |
| Container escape | Containers run inside a VM (not on bare host). VM provides hardware-level isolation |
| Path traversal | All file operations validated against workspace root. `..` components rejected |
| Malicious MCP tools | Tool execution requires user confirmation for destructive operations |
| Data at rest exposure | API keys encrypted with system keyring-derived key. SQLite file protected by OS permissions |

### 10.2 API Key Isolation

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│  Frontend   │     │ Rust Backend │     │ System       │
│  (webview)  │     │              │     │ Keyring      │
│             │     │              │     │              │
│ Never sees  │     │ 1. Get key   │     │ Provides     │
│ plaintext   │     │    from      │────→│ encryption   │
│ API keys    │     │    keyring   │     │ key          │
│             │     │              │     │              │
│ Only sends  │     │ 2. Decrypt   │     └──────────────┘
│ provider_id │────→│    API key   │
│             │     │    from DB   │     ┌──────────────┐
│             │     │              │     │ SQLite       │
│             │     │ 3. Use key   │     │ (encrypted   │
│             │     │    for LLM   │────→│  api_keys    │
│             │     │    request   │     │  table)      │
│             │     │              │     └──────────────┘
└─────────────┘     └──────────────┘
```

### 10.3 Container Sandboxing

```
Host OS
└── CrateBay Runtime VM (hardware isolation)
    └── Docker Engine
        └── User Container (namespace isolation)
            └── User Code (restricted capabilities)
```

Two layers of isolation:
1. **VM isolation**: Containers run inside a lightweight VM, not directly on the host
2. **Docker isolation**: Standard container namespaces, cgroups, seccomp profiles

### 10.4 Destructive Operation Confirmation

Operations classified by risk level:

| Risk Level | Examples | Confirmation |
|------------|----------|-------------|
| **Low** | List containers, inspect, logs | None |
| **Medium** | Create container, exec command | Implicit (user initiated) |
| **High** | Delete container, stop container | Explicit confirmation dialog |
| **Critical** | Cleanup all expired, delete all | Double confirmation with typing |

Keywords triggering confirmation: `delete`, `remove`, `destroy`, `drop`, `wipe`, `prune`, `terminate`, `kill`.

---

## 11. Performance Budget

All metrics are validated in CI via `scripts/bench-perf.sh`:

| Metric | Target | Measurement |
|--------|--------|-------------|
| Binary size (release) | < 20 MB | `ls -la target/release/cratebay-gui` |
| Cold startup time | < 3 s | Time from launch to first paint |
| Idle RAM usage | < 200 MB | RSS after startup, no active operations |
| Container list latency | < 500 ms | Time to fetch and display container list |
| LLM first token | < 2 s | Time from send to first streaming token (network excluded) |
| SQLite query (simple) | < 10 ms | Single table lookup |

### Optimization Strategies

- **Binary size**: Tauri v2's WebView-based approach avoids bundling a full browser engine
- **Startup time**: Lazy initialization of Docker connection and runtime detection
- **Memory**: Zustand stores are minimal; Streamdown recycles DOM nodes for long conversations
- **Dependencies**: Strict dependency budget — new crates must justify their size contribution

---

## 12. Extension Points

### 12.1 MCP (Model Context Protocol)

CrateBay supports MCP in two modes:

**MCP Server** (`cratebay-mcp` binary):
- Standalone binary exposing container sandbox tools via stdio transport
- External AI clients (Claude Desktop, Cursor, etc.) connect to it
- Tools: container create/exec/delete, file read/write, sandbox management

**MCP Client** (in `cratebay-core/src/mcp/`):
- Multi-file module: `config.rs` (config loading & env expansion), `jsonrpc.rs` (JSON-RPC 2.0 types), `manager.rs` (`McpManager` lifecycle), `transport.rs` (stdio + SSE)
- `McpManager` manages multiple MCP server connections with register/start/stop/remove lifecycle
- Connects to external MCP servers registered in `.mcp.json` or added via GUI
- Bridges external tools into the pi-agent-core tool catalog
- Supports stdio and SSE transports

```
External AI Client                    CrateBay MCP Server
(Claude Desktop)                      (cratebay-mcp binary)
       │                                      │
       │  MCP stdio protocol                  │
       │─────────────────────────────────────→│
       │  tool_call: "container_create"       │
       │                                      │  Docker operations
       │  result: { container_id: "abc123" }  │
       │←─────────────────────────────────────│

CrateBay App                          External MCP Server
(pi-agent-core)                       (e.g., shadcn MCP)
       │                                      │
       │  MCP stdio protocol                  │
       │  (via cratebay-core MCP client)      │
       │─────────────────────────────────────→│
       │  tool_call: "search_components"      │
       │                                      │
       │  result: [{ name: "Button", ... }]   │
       │←─────────────────────────────────────│
```

### 12.2 Future: gRPC Daemon (v2.1+)

Reserved for remote container management. Not implemented in v2.0.

```
Remote Client ──→ gRPC (tonic) ──→ cratebay-core ──→ Docker
```

The `proto/` directory is reserved for protobuf definitions. The architecture is designed so that `cratebay-core` can be called from both Tauri commands (local) and gRPC handlers (remote) without duplication.

### 12.3 Plugin Architecture (Future)

The agent tool system is inherently extensible:
- New `AgentTool` implementations can be added in TypeScript
- Each tool wraps a Tauri invoke, so backend capabilities can be expanded independently
- MCP bridge allows third-party tools without code changes

---

## Appendix A: Key Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| tauri | 2.x | Desktop application framework |
| bollard | 0.18 | Docker Engine API client |
| rusqlite | 0.32 | SQLite bindings (bundled) |
| tokio | 1.x | Async runtime |
| thiserror | 2.x | Error type derivation |
| serde / serde_json | 1.x | Serialization |
| serde_yaml | 0.9 | YAML serialization (CLI output) |
| reqwest | 0.12 | HTTP client (LLM proxy) |
| futures-util | 0.3 | Stream utilities (Docker logs/pull) |
| tracing | 0.1 | Structured logging |
| chrono | 0.4 | Date/time handling |
| uuid | 1.x | Unique identifier generation |
| tauri-specta | latest | Rust → TypeScript type generation |
| @mariozechner/pi-agent-core | latest | Agent orchestration framework |
| @mariozechner/pi-ai | latest | Unified LLM provider API |
| react | 19.x | Frontend UI framework |
| zustand | latest | State management |
| streamdown | latest | Streaming markdown renderer |

## Appendix B: Decision Records

Key architectural decisions are documented in [tech-decisions.md](../references/tech-decisions.md) using ADR format. Notable decisions:

- **ADR-005**: Hybrid Agent Architecture (TS orchestration + Rust tools)
- **ADR-006**: LLM Proxy through Rust Backend (security)
- **ADR-004**: Built-in Container Runtime (zero-dependency UX)
- **ADR-009**: Defer gRPC Daemon to v2.1+
