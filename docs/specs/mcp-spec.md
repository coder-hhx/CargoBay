# MCP Server + Client Specification

> Version: 1.1.0 | Last Updated: 2026-03-20 | Author: frontend-architect

---

## Table of Contents

1. [MCP Overview](#1-mcp-overview)
2. [MCP Server (cratebay-mcp)](#2-mcp-server-cratebay-mcp)
3. [MCP Client (cratebay-core)](#3-mcp-client-cratebay-core)
4. [MCP Tool Bridge](#4-mcp-tool-bridge)
5. [Development Tools](#5-development-tools)
6. [Configuration](#6-configuration)

---

## 1. MCP Overview

The **Model Context Protocol (MCP)** is an open standard for connecting AI assistants to external tools and data sources. CrateBay implements MCP in two roles:

### CrateBay as MCP Server

The `cratebay-mcp` binary is a standalone MCP server that exposes CrateBay's container management capabilities to external AI clients (Claude Desktop, Cursor, VS Code Copilot, etc.). External AI assistants can create/manage containers and execute commands through this interface.

### CrateBay as MCP Client

The `cratebay-core` crate includes an MCP client that connects to external MCP servers (e.g., shadcn MCP, custom tool servers). These external tools become available to CrateBay's built-in AI assistant through the Tool Bridge.

```
                    ┌──────────────────────────────┐
                    │    External AI Clients        │
                    │  (Claude Desktop, Cursor,     │
                    │   VS Code Copilot, etc.)      │
                    └──────────┬───────────────────┘
                               │ MCP Protocol (stdio)
                    ┌──────────▼───────────────────┐
                    │   cratebay-mcp (Server)       │
                    │   Standalone binary            │
                    │   Tools: container ops,        │
                    │   file ops, sandbox mgmt       │
                    └──────────────────────────────┘

  ┌────────────────────────────────────────────────────────────┐
  │                    CrateBay Desktop App                     │
  │                                                            │
  │  ┌───────────────────────────────────────────────────┐     │
  │  │  pi-agent-core (TS)                               │     │
  │  │  ┌─────────────┐  ┌────────────────────────┐     │     │
  │  │  │ Built-in    │  │ MCP Bridge Tools       │     │     │
  │  │  │ AgentTools  │  │ (from external servers)│     │     │
  │  │  └─────────────┘  └───────────┬────────────┘     │     │
  │  └──────────────────────────────────────────────────┘     │
  │                                   │                        │
  │  ┌───────────────────────────────▼────────────────────┐   │
  │  │  cratebay-core MCP Client (Rust)                   │   │
  │  │  ├── stdio transport                                │   │
  │  │  ├── SSE transport                                  │   │
  │  │  └── Connection lifecycle                           │   │
  │  └───────────┬──────────────────┬─────────────────────┘   │
  │              │                  │                           │
  └──────────────┼──────────────────┼──────────────────────────┘
                 │                  │
     ┌───────────▼──────┐  ┌──────▼──────────────┐
     │  shadcn MCP      │  │  Custom MCP Servers  │
     │  (7 tools)       │  │  (user-configured)   │
     └──────────────────┘  └─────────────────────┘
```

---

## 2. MCP Server (cratebay-mcp)

### 2.1 Binary Overview

`cratebay-mcp` is a standalone Rust binary in `crates/cratebay-mcp/`. It communicates via **stdio** (stdin/stdout) using the MCP JSON-RPC protocol.

```bash
# Run the MCP server
cratebay-mcp

# With workspace root restriction
CRATEBAY_MCP_WORKSPACE_ROOT=/path/to/workspace cratebay-mcp
```

### 2.2 Tool Catalog

The MCP server exposes the following tools:

| Tool Name | Description | Parameters |
|-----------|-------------|------------|
| `cratebay_sandbox_templates` | List available sandbox templates | — |
| `cratebay_sandbox_list` | List all managed sandboxes | `status?` |
| `cratebay_sandbox_inspect` | Get detailed sandbox information | `sandbox_id` |
| `cratebay_sandbox_create` | Create a new sandbox from template | `template_id`, `name?`, `image?`, `command?`, `env?`, `cpu_cores?`, `memory_mb?`, `ttl_hours?`, `owner?` |
| `cratebay_sandbox_start` | Start a stopped sandbox | `sandbox_id` |
| `cratebay_sandbox_stop` | Stop a running sandbox | `sandbox_id` |
| `cratebay_sandbox_delete` | Delete a sandbox permanently | `sandbox_id` |
| `cratebay_sandbox_exec` | Execute a command in a sandbox | `sandbox_id`, `command`, `timeout?` |
| `cratebay_sandbox_cleanup_expired` | Remove all expired sandboxes | — |
| `cratebay_sandbox_put_path` | Copy a file into a sandbox | `sandbox_id`, `container_path`, `content` (base64) |
| `cratebay_sandbox_get_path` | Copy a file from a sandbox | `sandbox_id`, `container_path` |

### 2.3 Sandbox Template System

Templates provide pre-configured development environments:

```json
{
  "templates": [
    {
      "id": "node-dev",
      "name": "Node.js Development",
      "description": "Node.js 20 LTS with npm, yarn, and common dev tools",
      "image": "node:20-bookworm",
      "default_command": "sleep infinity",
      "default_cpu_cores": 2,
      "default_memory_mb": 2048,
      "tags": ["javascript", "typescript", "node"]
    },
    {
      "id": "python-dev",
      "name": "Python Development",
      "description": "Python 3.12 with pip, venv, and common scientific packages",
      "image": "python:3.12-bookworm",
      "default_command": "sleep infinity",
      "default_cpu_cores": 2,
      "default_memory_mb": 2048,
      "tags": ["python", "data-science", "ml"]
    },
    {
      "id": "rust-dev",
      "name": "Rust Development",
      "description": "Rust stable with cargo, rustfmt, clippy",
      "image": "rust:1-bookworm",
      "default_command": "sleep infinity",
      "default_cpu_cores": 4,
      "default_memory_mb": 4096,
      "tags": ["rust", "systems"]
    },
    {
      "id": "ubuntu-base",
      "name": "Ubuntu Base",
      "description": "Clean Ubuntu 24.04 with basic tools",
      "image": "ubuntu:24.04",
      "default_command": "sleep infinity",
      "default_cpu_cores": 1,
      "default_memory_mb": 1024,
      "tags": ["general", "linux"]
    }
  ]
}
```

### 2.4 Security Model

#### Workspace Root Restriction

The `CRATEBAY_MCP_WORKSPACE_ROOT` environment variable restricts file operations to a specific directory tree:

```rust
// Path validation for file operations
fn validate_path(requested_path: &str, workspace_root: &Path) -> Result<PathBuf, McpError> {
    let canonical = workspace_root.join(requested_path).canonicalize()?;

    // Prevent path traversal
    if !canonical.starts_with(workspace_root) {
        return Err(McpError::PathTraversal {
            requested: requested_path.to_string(),
            root: workspace_root.display().to_string(),
        });
    }

    // Reject paths with ".." components (before canonicalization too)
    if requested_path.contains("..") {
        return Err(McpError::PathTraversal {
            requested: requested_path.to_string(),
            root: workspace_root.display().to_string(),
        });
    }

    Ok(canonical)
}
```

#### Confirmation for Destructive Operations

The MCP server marks destructive tools with `confirmation_required: true` in the tool schema. MCP clients (Claude Desktop, etc.) will prompt the user before executing these tools.

Destructive tools:
- `cratebay_sandbox_delete`
- `cratebay_sandbox_cleanup_expired`
- `cratebay_sandbox_stop` (if running)

#### Docker Label Metadata

Sandbox metadata is stored as Docker container labels, providing a stateless management model:

```
com.cratebay.sandbox.managed=true
com.cratebay.sandbox.template_id=node-dev
com.cratebay.sandbox.owner=current_user
com.cratebay.sandbox.created_at=2026-03-20T10:30:00Z
com.cratebay.sandbox.expires_at=2026-03-21T10:30:00Z
com.cratebay.sandbox.ttl_hours=24
com.cratebay.sandbox.cpu_cores=2
com.cratebay.sandbox.memory_mb=2048
```

### 2.5 Audit Logging

All MCP tool calls are logged for security auditing:

```rust
struct AuditEntry {
    timestamp: DateTime<Utc>,
    tool_name: String,
    parameters: serde_json::Value,  // sanitized (no secrets)
    result: AuditResult,            // success/error with brief message
    caller: String,                 // MCP client identifier
    duration_ms: u64,
}
```

Audit logs are written to `~/.cratebay/logs/mcp-audit.jsonl` (one JSON object per line).

---

## 3. MCP Client (cratebay-core)

### 3.1 Transport Support

The MCP client in `cratebay-core` supports two transport mechanisms:

| Transport | Protocol | Use Case |
|-----------|----------|----------|
| **stdio** | JSON-RPC over stdin/stdout | Local MCP servers (spawned as child processes) |
| **SSE** | Server-Sent Events over HTTP | Remote MCP servers |

### 3.2 .mcp.json Configuration Format

MCP servers are configured in `.mcp.json` at the project root:

```json
{
  "mcpServers": {
    "shadcn": {
      "command": "npx",
      "args": ["shadcn@latest", "mcp"],
      "env": {},
      "transport": "stdio"
    },
    "custom-server": {
      "command": "/usr/local/bin/my-mcp-server",
      "args": ["--port", "3001"],
      "env": {
        "API_KEY": "${CUSTOM_API_KEY}"
      },
      "transport": "stdio"
    },
    "remote-server": {
      "url": "https://mcp.example.com/sse",
      "transport": "sse",
      "headers": {
        "Authorization": "Bearer ${REMOTE_TOKEN}"
      }
    }
  }
}
```

**Environment variable expansion**: Values containing `${VAR_NAME}` are expanded from the process environment at connection time.

### 3.3 Server Discovery

On application startup, the MCP client:

1. Reads `.mcp.json` from the project root
2. Reads user-configured servers from SQLite (`mcp_servers` table)
3. Merges configurations (user config overrides `.mcp.json` for same server names)
4. Stores merged configuration in `mcpStore`

### 3.4 Connection Lifecycle

```
┌──────────┐     ┌─────────────┐     ┌───────────┐     ┌──────────────┐
│  Config   │────►│  Spawning   │────►│ Connected │────►│ Disconnected │
│  Loaded   │     │  (stdio) /  │     │           │     │              │
│           │     │  Connecting │     │           │     │              │
│           │     │  (SSE)      │     │           │     │              │
└──────────┘     └──────┬──────┘     └─────┬─────┘     └──────────────┘
                        │                   │
                        │  error            │  error / server exit
                        ▼                   ▼
                 ┌──────────────┐    ┌──────────────┐
                 │    Error     │    │    Error     │
                 │  (retry 3x) │    │  (retry 3x) │
                 └──────────────┘    └──────────────┘
```

**stdio transport lifecycle:**

```rust
// 1. Spawn child process
let child = Command::new(&config.command)
    .args(&config.args)
    .envs(&config.env)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

// 2. Send initialize request
let init_response = send_request(&child, "initialize", json!({
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {
        "name": "CrateBay",
        "version": env!("CARGO_PKG_VERSION")
    }
}))?;

// 3. Send initialized notification
send_notification(&child, "notifications/initialized", json!({}))?;

// 4. Discover tools
let tools_response = send_request(&child, "tools/list", json!({}))?;
```

**Retry policy:**
- 3 retries with exponential backoff (1s, 2s, 4s)
- After 3 failures, mark server as `error` state
- User can manually retry via McpPage UI

### 3.5 Tool Call Forwarding

When the CrateBay agent (pi-agent-core) invokes an MCP-bridged tool:

```
pi-agent-core         MCP Client (Rust)        External MCP Server
     │                      │                        │
     │ mcp_call_tool(       │                        │
     │   serverId,          │                        │
     │   toolName,          │                        │
     │   arguments)         │                        │
     ├─────────────────────►│                        │
     │                      │  JSON-RPC: tools/call  │
     │                      │  { name, arguments }   │
     │                      ├───────────────────────►│
     │                      │                        │
     │                      │  JSON-RPC response     │
     │                      │  { content: [...] }    │
     │                      │◄───────────────────────┤
     │  result              │                        │
     │◄─────────────────────┤                        │
```

---

## 4. MCP Tool Bridge

The Tool Bridge makes external MCP tools appear as native `AgentTool` instances in pi-agent-core.

### 4.1 Schema Mapping

MCP tool schemas (JSON Schema) are mapped to TypeBox schemas for pi-agent-core compatibility:

```typescript
// tools/mcpTools.ts
import { Type } from "@sinclair/typebox";

function mcpSchemaToTypebox(mcpSchema: Record<string, unknown>): TSchema {
  // MCP tools use standard JSON Schema
  // TypeBox generates compatible JSON Schema
  // Direct pass-through is usually sufficient
  return Type.Unsafe(mcpSchema);
}

function createMcpAgentTool(
  serverId: string,
  mcpTool: McpToolDefinition
): AgentTool {
  return {
    name: `mcp_${serverId}_${mcpTool.name}`,
    label: `[${serverId}] ${mcpTool.name}`,
    description: mcpTool.description,
    parameters: mcpSchemaToTypebox(mcpTool.inputSchema),
    execute: async (toolCallId, params, signal, onUpdate) => {
      onUpdate?.({
        content: [{ type: "text", text: `Calling ${mcpTool.name} on ${serverId}...` }],
      });

      const result = await invoke("mcp_client_call_tool", {
        serverId,
        toolName: mcpTool.name,
        arguments: params,
      });

      return result;
    },
  };
}
```

### 4.2 Dynamic Tool Registration

When MCP servers connect or disconnect, the tool registry is updated dynamically:

```typescript
// hooks/useMcpToolSync.ts
function useMcpToolSync(agent: Agent) {
  const servers = useMcpStore((s) => s.servers);
  const availableTools = useMcpStore((s) => s.availableTools);

  useEffect(() => {
    // Build MCP bridge tools from connected servers
    const mcpBridgeTools = availableTools.map((tool) =>
      createMcpAgentTool(tool.serverId, {
        name: tool.name,
        description: tool.description,
        inputSchema: tool.inputSchema,
      })
    );

    // Update agent with combined built-in + MCP tools
    agent.setTools([...builtinTools, ...mcpBridgeTools]);
  }, [availableTools, agent]);
}
```

### 4.3 Error Propagation

MCP tool errors are propagated through the bridge as thrown exceptions, ensuring the LLM can attempt error recovery:

```typescript
execute: async (toolCallId, params, signal, onUpdate) => {
  try {
    return await invoke("mcp_client_call_tool", {
      serverId,
      toolName,
      arguments: params,
    });
  } catch (err) {
    // Throw with context so LLM understands the failure
    throw new Error(
      `MCP tool "${toolName}" on server "${serverId}" failed: ${String(err)}`
    );
  }
},
```

---

## 5. Development Tools

### 5.1 shadcn MCP Server

The shadcn MCP server provides AI-assisted component discovery and installation.

**7 Tools:**

| Tool | Description | Parameters |
|------|-------------|------------|
| `get_project_registries` | List component registries configured in `components.json` | — |
| `list_items_in_registries` | List all available components, blocks, and templates in registries | `registryNames?` |
| `search_items_in_registries` | Fuzzy search across registries for components matching a query | `query`, `registryNames?` |
| `view_items_in_registries` | View full source code and metadata for specific components | `itemNames`, `registryNames?` |
| `get_item_examples_from_registries` | Get usage examples for components (files ending in `-demo` or `example-`) | `itemNames`, `registryNames?` |
| `get_add_command_for_items` | Generate the `npx shadcn add` command to install components | `itemNames` |
| `get_audit_checklist` | Get a UI code review checklist based on shadcn/ui best practices | — |

**Configuration:**

```json
{
  "mcpServers": {
    "shadcn": {
      "command": "npx",
      "args": ["shadcn@latest", "mcp"]
    }
  }
}
```

**Usage Examples:**

```
User: "I need a data table component"
Agent: [calls search_items_in_registries with query="data table"]
       → Found: data-table, table, sortable-table
Agent: [calls view_items_in_registries with itemNames=["data-table"]]
       → Shows source code and dependencies
Agent: [calls get_add_command_for_items with itemNames=["data-table"]]
       → "npx shadcn@latest add data-table"
```

### 5.2 Tauri MCP Server

The Tauri MCP Server enables AI-driven UI automation and IPC monitoring during development. **Only enabled in debug builds.**

**20 Tools in 3 categories:**

#### UI Automation (14 tools)

| Tool | Description |
|------|-------------|
| `tauri_click` | Click an element by selector |
| `tauri_type` | Type text into an input element |
| `tauri_select` | Select an option from a dropdown |
| `tauri_scroll` | Scroll an element or the page |
| `tauri_screenshot` | Capture a screenshot of the app |
| `tauri_get_text` | Get text content of an element |
| `tauri_get_attribute` | Get an attribute value of an element |
| `tauri_wait_for` | Wait for an element to appear |
| `tauri_evaluate` | Execute JavaScript in the webview |
| `tauri_get_elements` | Query multiple elements by selector |
| `tauri_focus` | Focus an element |
| `tauri_hover` | Hover over an element |
| `tauri_drag_drop` | Drag an element to a target |
| `tauri_keyboard` | Send keyboard events |

#### IPC Monitoring (5 tools)

| Tool | Description |
|------|-------------|
| `tauri_ipc_start_capture` | Start capturing Tauri invoke calls |
| `tauri_ipc_stop_capture` | Stop capturing and return results |
| `tauri_ipc_list_commands` | List all registered Tauri commands |
| `tauri_ipc_call` | Directly invoke a Tauri command |
| `tauri_ipc_events` | List recent Tauri events |

#### Settings (1 tool)

| Tool | Description |
|------|-------------|
| `tauri_configure` | Configure the MCP bridge (app URL, timeout, etc.) |

**Installation (debug only):**

```rust
// src-tauri/src/main.rs
#[cfg(debug_assertions)]
builder = builder.plugin(tauri_plugin_mcp_bridge::init());
```

```json
// .mcp.json (only used during development)
{
  "mcpServers": {
    "tauri": {
      "command": "npx",
      "args": ["-y", "@hypothesi/tauri-mcp-server"]
    }
  }
}
```

---

## 6. Configuration

### 6.1 .mcp.json Format

The `.mcp.json` file at the project root configures MCP server connections:

```json
{
  "mcpServers": {
    "<server-name>": {
      "command": "<executable>",
      "args": ["<arg1>", "<arg2>"],
      "env": {
        "<KEY>": "<value or ${ENV_VAR}>"
      },
      "transport": "stdio",
      "enabled": true,
      "notes": "Optional description"
    }
  }
}
```

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `command` | string | Yes (stdio) | Executable path or command name |
| `args` | string[] | No | Command arguments |
| `env` | Record<string, string> | No | Environment variables (supports `${VAR}` expansion) |
| `transport` | `"stdio"` \| `"sse"` | No | Transport mechanism (default: `"stdio"`) |
| `url` | string | Yes (SSE) | Server URL for SSE transport |
| `headers` | Record<string, string> | No | HTTP headers for SSE transport |
| `enabled` | boolean | No | Enable/disable without removing config (default: `true`) |
| `notes` | string | No | Human-readable description |

### 6.2 CrateBay Production Configuration

```json
{
  "mcpServers": {
    "shadcn": {
      "command": "npx",
      "args": ["shadcn@latest", "mcp"],
      "notes": "shadcn/ui component discovery and installation"
    }
  }
}
```

### 6.3 CrateBay Development Configuration

```json
{
  "mcpServers": {
    "shadcn": {
      "command": "npx",
      "args": ["shadcn@latest", "mcp"],
      "notes": "shadcn/ui component discovery"
    },
    "tauri": {
      "command": "npx",
      "args": ["-y", "@hypothesi/tauri-mcp-server"],
      "notes": "Tauri UI automation and IPC monitoring (dev only)"
    }
  }
}
```

### 6.4 User-Configured Servers

Users can add custom MCP servers through the McpPage UI. These are stored in SQLite and merged with `.mcp.json` at runtime:

```sql
CREATE TABLE mcp_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    command TEXT NOT NULL,
    args TEXT NOT NULL DEFAULT '[]',     -- JSON array
    env TEXT NOT NULL DEFAULT '{}',      -- JSON object
    transport TEXT NOT NULL DEFAULT 'stdio',
    url TEXT,                            -- For SSE transport
    enabled INTEGER NOT NULL DEFAULT 1,
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

**Merge priority:** User-configured servers (SQLite) override `.mcp.json` entries with the same name. This allows users to customize default server configurations without modifying the project file.

### 6.5 MCP Server Export

CrateBay can export its MCP server configuration for use with external AI clients:

```typescript
// Export for Claude Desktop
const claudeConfig = {
  mcpServers: {
    cratebay: {
      command: "cratebay-mcp",
      args: [],
      env: {
        CRATEBAY_MCP_WORKSPACE_ROOT: "/path/to/workspace",
      },
    },
  },
};

// Write to Claude Desktop config
// macOS: ~/Library/Application Support/Claude/claude_desktop_config.json
// Windows: %APPDATA%/Claude/claude_desktop_config.json
```

This is exposed via the Tauri command `mcp_export_client_config` and available in the McpPage UI.
