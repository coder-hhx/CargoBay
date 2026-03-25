# Agent Integration Specification

> Version: 1.2.2 | Last Updated: 2026-03-25 | Author: frontend-architect

---

## Table of Contents

1. [Agent Architecture Overview](#1-agent-architecture-overview)
2. [pi-agent-core Integration](#2-pi-agent-core-integration)
3. [LLM Proxy StreamFn](#3-llm-proxy-streamfn)
4. [Tool Definition Standard](#4-tool-definition-standard)
5. [Built-in Tools Catalog](#5-built-in-tools-catalog)
6. [System Prompt Design](#6-system-prompt-design)
7. [Conversation Persistence](#7-conversation-persistence)
8. [Multi-turn Context Management](#8-multi-turn-context-management)
9. [Safety: Risk Levels and Confirmation](#9-safety-risk-levels-and-confirmation)

---

## 1. Agent Architecture Overview

CrateBay employs a **Hybrid Agent Architecture**: TypeScript handles orchestration (pi-agent-core), while Rust handles tool execution, LLM proxying, and security.

```
┌─────────────────── TypeScript Layer ────────────────────┐
│                                                         │
│  pi-agent-core                                          │
│  ┌─────────────────────────────────────────────────┐    │
│  │  Agent Loop:                                    │    │
│  │  1. Receive user message                        │    │
│  │  2. Call streamFn → LLM response (streamed)     │    │
│  │  3. Parse tool calls from response              │    │
│  │  4. Execute AgentTool.execute()                  │    │
│  │  5. Feed tool result back to LLM                │    │
│  │  6. Repeat until no more tool calls             │    │
│  │  7. Return final response                       │    │
│  └─────────────────────────────────────────────────┘    │
│                    │                                     │
│         AgentTool.execute()                              │
│                    │                                     │
│  ┌─────────────────▼─────────────────────────────┐      │
│  │  Tool Wrappers (TS)                           │      │
│  │  - Validate parameters (TypeBox)              │      │
│  │  - Check risk level → request confirmation    │      │
│  │  - Call Tauri invoke                          │      │
│  │  - Transform result                           │      │
│  └─────────────────┬─────────────────────────────┘      │
└─────────────────────┼───────────────────────────────────┘
                      │ Tauri invoke
┌─────────────────────▼───────────────────────────────────┐
│                   Rust Layer                             │
│                                                         │
│  Tauri Commands                                         │
│  ├── container.rs  → bollard Docker SDK                 │
│  ├── llm.rs        → HTTP to LLM providers              │
│  ├── storage.rs    → SQLite (rusqlite)                  │
│  ├── mcp.rs        → MCP Client (stdio/SSE)             │
│  └── system.rs     → Runtime, Docker status              │
│                                                         │
│  Security:                                              │
│  - API Keys encrypted in SQLite                         │
│  - Container sandboxing via Docker                      │
│  - Path validation for file operations                  │
└─────────────────────────────────────────────────────────┘
```

### Why Hybrid?

| Concern | Layer | Reason |
|---------|-------|--------|
| Agent orchestration | TypeScript | pi-agent-core is a mature TS framework |
| Tool parameter validation | TypeScript | TypeBox schemas live alongside tool definitions |
| LLM API calls | Rust | API keys must never reach the frontend process |
| Docker operations | Rust | bollard SDK is Rust-native |
| SQLite storage | Rust | rusqlite is Rust-native, avoids WASM overhead |
| MCP client | Rust | Process spawning and stdio management is safer in Rust |
| Streaming to UI | Both | Rust emits Tauri Events, TS renders via Streamdown |

---

## 2. pi-agent-core Integration

### 2.1 Initialization (lib/agent.ts)

The agent is initialized once at application startup and persists across chat sessions.

```typescript
// lib/agent.ts
import { Agent, type AgentOptions } from "@mariozechner/pi-agent-core";
import { createStreamFn } from "@/lib/streamFn";
import { allTools } from "@/tools";
import { buildSystemPrompt } from "@/lib/systemPrompt";

export function createCrateBayAgent(): Agent {
  const opts: AgentOptions = {
    streamFn: createStreamFn(),
    toolExecution: "parallel",     // Execute tools in parallel when possible
    beforeToolCall: confirmToolExecution,  // Check risk level before executing
  };

  return new Agent(opts);
}

// Helper function for tool confirmation flow
async function confirmToolExecution(toolCall: AgentToolCall, context: any): Promise<boolean> {
  const riskLevel = getToolRiskLevel(toolCall.function.name);
  if (shouldConfirm(riskLevel)) {
    return await workflowStore.requestConfirmation({
      toolName: toolCall.function.name,
      toolLabel: getToolLabel(toolCall.function.name),
      description: buildConfirmationDescription(toolCall),
      riskLevel,
      parameters: JSON.parse(toolCall.function.arguments),
      consequences: buildConsequencesList(toolCall),
    });
  }
  return true;
}
```

### 2.2 AgentMessage Types

pi-agent-core uses a typed message system:

```typescript
// From pi-agent-core
interface AgentMessage {
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  toolCalls?: AgentToolCall[];
  toolCallId?: string;     // For tool result messages
  name?: string;           // Tool name for tool result messages
}

interface AgentToolCall {
  id: string;
  type: "function";
  function: {
    name: string;
    arguments: string;     // JSON string
  };
}
```

### 2.3 Event Subscription

The agent emits events throughout its lifecycle. CrateBay subscribes to these events to update the UI:

```typescript
// hooks/useAgent.ts
import { Agent, AgentEvent } from "@mariozechner/pi-agent-core";

function useAgent(agent: Agent) {
  useEffect(() => {
    const unsubscribe = agent.on("*", (event: AgentEvent) => {
      switch (event.type) {
        case "agent_start":
          // Agent begins processing a user message
          workflowStore.setAgentStatus("thinking");
          break;

        case "turn_start":
          // New turn in the agent loop (may involve tool calls)
          // event.turnIndex: number
          break;

        case "message_start":
          // LLM begins generating a response
          chatStore.setStreaming(true, event.messageId);
          break;

        case "message_update":
          // Streaming token received
          chatStore.appendStreamChunk(
            activeSessionId,
            event.messageId,
            event.delta
          );
          break;

        case "message_end":
          // LLM response complete
          chatStore.setStreaming(false);
          break;

        case "tool_execution_start":
          // Tool execution begins
          // event.toolCall: AgentToolCall
          workflowStore.setAgentStatus("executing");
          workflowStore.setCurrentToolExecution({
            id: event.toolCall.id,
            toolName: event.toolCall.function.name,
            toolLabel: getToolLabel(event.toolCall.function.name),
            parameters: JSON.parse(event.toolCall.function.arguments),
            status: "running",
            startedAt: new Date().toISOString(),
            riskLevel: getToolRiskLevel(event.toolCall.function.name),
          });
          break;

        case "tool_execution_update":
          // Tool execution progress update
          // event.update: AgentToolUpdate
          workflowStore.updateCurrentToolProgress(event.update);
          break;

        case "tool_execution_end":
          // Tool execution complete
          // event.result: AgentToolResult
          // event.error?: string
          workflowStore.setCurrentToolExecution(null);
          break;

        case "turn_end":
          // Current turn completed (tools executed, results collected)
          // event.turnIndex: number
          break;

        case "agent_end":
          // Agent finished processing
          workflowStore.setAgentStatus("idle");
          chatStore.setAgentThinking(false);
          break;
      }
    });

    return unsubscribe;
  }, [agent]);
}
```

### Event Lifecycle

```
User sends message
    │
    ▼
agent_start
    │
    ▼
turn_start (turn 0)
    │
    ├── message_start
    ├── message_update (token)
    ├── message_update (token)
    ├── ...
    ├── message_end
    │
    ├── tool_execution_start (tool call detected)
    ├── tool_execution_update (progress)
    ├── tool_execution_end (result)
    │
    ├── tool_execution_start (another tool call)
    ├── tool_execution_update (progress)
    ├── tool_execution_end (result)
    │
    ▼
turn_end (turn 0)
    │
    ▼
turn_start (turn 1) ← tool results fed back to LLM
    │
    ├── message_start
    ├── message_update ...
    ├── message_end
    │
    ▼
turn_end (turn 1) ← no more tool calls
    │
    ▼
agent_end
```

### 2.4 Agent State

The agent exposes its current state:

```typescript
agent.state: {
  status: "idle" | "running" | "error";
  currentTurn: number;
  messages: AgentMessage[];   // Full conversation history
  lastError?: Error;
}
```

### 2.5 Steering

Steering allows injecting system-level instructions mid-conversation without user visibility:

```typescript
// Inject context before the next agent turn
agent.steer("The user is working with container 'node-01'. Prefer operations on this container.");
```

**Use cases:**
- Inject current container context when user references "@container:node-01"
- Add MCP tool availability info when new servers connect
- Provide workspace/project context

### 2.6 FollowUp

After the agent completes a response, it can suggest follow-up actions:

```typescript
agent.on("agent_end", (event) => {
  if (event.followUps) {
    workflowStore.setFollowUpSuggestions(event.followUps);
  }
});
```

Follow-ups appear as clickable chips below the last message in the ChatPage.

---

## 3. LLM Proxy StreamFn

The `streamFn` is the bridge between pi-agent-core and the LLM. CrateBay implements a custom `streamFn` that routes through the Rust backend — **API keys never touch the frontend**.

### 3.1 Supported API Formats

CrateBay supports exactly three LLM API formats. The Rust backend selects the correct format based on the provider's `api_format` field stored in the database.

#### 3.1.1 Anthropic Messages API (`api_format: "anthropic"`)

The Anthropic Messages API has a distinct request structure:

```json
// Request
{
  "model": "claude-3-5-sonnet-20241022",
  "max_tokens": 4096,
  "system": "You are CrateBay AI Assistant...",   // system prompt as top-level parameter, NOT in messages
  "messages": [
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "Create a Node.js sandbox" }
      ]
    }
  ],
  "tools": [
    {
      "name": "container_create",
      "description": "Create a new container",
      "input_schema": { "type": "object", "properties": { ... } }
    }
  ],
  "stream": true
}
```

**Key differences from OpenAI format:**
- `system` is a **top-level parameter**, not a message in the `messages` array
- Message `content` uses **content blocks** (array of `{type, text}` objects), not plain strings
- Tool definitions use `input_schema` instead of `parameters`
- Tool results use `tool_result` role with `tool_use_id` reference
- SSE stream events use `content_block_delta` and `message_delta` event types
- No reasoning effort support

#### 3.1.2 OpenAI Responses API (`api_format: "openai_responses"`)

The OpenAI Responses API is the newer format with built-in reasoning support:

```json
// Request
{
  "model": "o3",
  "input": [
    { "role": "system", "content": "You are CrateBay AI Assistant..." },
    { "role": "user", "content": "Create a Node.js sandbox" }
  ],
  "tools": [
    {
      "type": "function",
      "name": "container_create",
      "description": "Create a new container",
      "parameters": { "type": "object", "properties": { ... } }
    }
  ],
  "reasoning": {
    "effort": "medium"       // "low", "medium", "high" — only this format supports reasoning effort
  },
  "stream": true
}
```

**Key differences:**
- Uses `input` instead of `messages` as the conversation array field name
- Supports `reasoning.effort` parameter (`"low"`, `"medium"`, `"high"`)
- This is the **only** format where reasoning effort / thinking settings apply
- Response includes `output` array with output items
- Stream events use `response.output_item.delta` format

#### 3.1.3 OpenAI Chat Completions (`api_format: "openai_completions"`)

The standard OpenAI Chat Completions format:

```json
// Request
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "You are CrateBay AI Assistant..." },
    { "role": "user", "content": "Create a Node.js sandbox" }
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "container_create",
        "description": "Create a new container",
        "parameters": { "type": "object", "properties": { ... } }
      }
    }
  ],
  "stream": true
}
```

**Key characteristics:**
- Standard `messages` array with `{role, content}` objects
- `system` role is a regular message in the array
- Tool definitions wrapped in `{type: "function", function: {...}}` structure
- SSE stream events use `chat.completion.chunk` format
- No reasoning effort support

### 3.2 Unified Rust Proxy Layer

The Rust backend (`llm_proxy.rs`) acts as a unified proxy that:

1. **Receives** a format-agnostic request from the frontend (standard `ChatMessage[]` array)
2. **Looks up** the provider's `api_format` from the database
3. **Transforms** the request into the correct API format:
   - Anthropic: Extract system message to top-level, convert content to blocks, rename tool schema keys
   - OpenAI Responses: Rename `messages` to `input`, add `reasoning.effort` if provided
   - OpenAI Completions: Pass through with standard structure
4. **Adds dual authentication headers** to the outgoing request
5. **Parses** the provider-specific SSE response stream into a unified `LlmStreamEvent` format
6. **Emits** unified Tauri Events to the frontend

```
Frontend (uniform ChatMessage[])      Rust LLM Proxy              LLM Provider
         │                               │                            │
         ├── invoke(llm_proxy_stream) ───▶│                            │
         │                               ├── lookup api_format          │
         │                               ├── transform request          │
         │                               ├── add dual headers           │
         │                               ├── POST to provider ────────▶│
         │                               │                            │
         │                               │◀── SSE stream ────────────┤
         │                               ├── parse provider-specific    │
         │                               │   response format           │
         │◀── Tauri Event (unified) ───┤                            │
         │                               │                            │
```

### 3.3 Dual Header Authentication

All outgoing LLM requests include **both** authentication headers simultaneously:

```rust
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

This dual-header approach ensures compatibility across all provider types:
- OpenAI-compatible providers use `Authorization: Bearer`
- Anthropic uses `x-api-key`
- Both are always sent regardless of provider type

### 3.4 Flow

```
pi-agent-core                 Frontend (TS)              Backend (Rust)           LLM Provider
     │                             │                          │                        │
     │  streamFn(messages)         │                          │                        │
     ├────────────────────────────►│                          │                        │
     │                             │  invoke("llm_proxy_     │                        │
     │                             │    stream", {messages})  │                        │
     │                             ├─────────────────────────►│                        │
     │                             │                          │  HTTP POST /v1/chat/   │
     │                             │                          │  completions (stream)  │
     │                             │                          ├───────────────────────►│
     │                             │                          │                        │
     │                             │                          │  SSE: data: {"delta":  │
     │                             │                          │◄───────────────────────┤
     │                             │  Tauri Event:            │  "token"}              │
     │                             │  "llm:token:session-id"  │                        │
     │                             │◄─────────────────────────┤                        │
     │  yield { delta: "token" }   │                          │                        │
     │◄────────────────────────────┤                          │                        │
     │                             │                          │                        │
     │  (repeat for each token)    │                          │                        │
     │                             │                          │  SSE: data: [DONE]     │
     │                             │                          │◄───────────────────────┤
     │                             │  Tauri Event:            │                        │
     │                             │  "llm:done:session-id"   │                        │
     │                             │◄─────────────────────────┤                        │
     │  return (stream ends)       │                          │                        │
     │◄────────────────────────────┤                          │                        │
```

### 3.5 Implementation

```typescript
// lib/streamFn.ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { StreamFn, StreamMessage } from "@mariozechner/pi-agent-core";

export function createStreamFn(): StreamFn {
  return async function* streamFn(
    messages: StreamMessage[],
    options?: { signal?: AbortSignal }
  ): AsyncGenerator<StreamChunk> {
    const sessionId = crypto.randomUUID();

    // Set up event listener before invoking
    const tokenQueue: StreamChunk[] = [];
    let done = false;
    let resolveWait: (() => void) | null = null;

    const unlisten = await listen<LlmTokenEvent>(
      `llm:token:${sessionId}`,
      (event) => {
        if (event.payload.done) {
          done = true;
        } else {
          tokenQueue.push({ delta: event.payload.token });
        }
        resolveWait?.();
      }
    );

    // Start the streaming request (non-blocking, Rust handles streaming)
    invoke("llm_proxy_stream", {
      sessionId,
      messages: messages.map((m) => ({
        role: m.role,
        content: m.content,
        toolCalls: m.toolCalls,
        toolCallId: m.toolCallId,
      })),
    }).catch((err) => {
      tokenQueue.push({ error: String(err) });
      done = true;
      resolveWait?.();
    });

    try {
      while (!done || tokenQueue.length > 0) {
        if (options?.signal?.aborted) {
          invoke("llm_proxy_cancel", { sessionId });
          break;
        }

        if (tokenQueue.length > 0) {
          const chunk = tokenQueue.shift()!;
          if (chunk.error) throw new Error(chunk.error);
          yield chunk;
        } else {
          await new Promise<void>((resolve) => {
            resolveWait = resolve;
          });
        }
      }
    } finally {
      unlisten();
    }
  };
}

interface StreamChunk {
  delta?: string;
  error?: string;
}
```

### 3.6 Security Guarantees

1. **API keys are stored encrypted in SQLite** on the Rust side
2. **API keys are decrypted only in-memory** during the HTTP request to the LLM provider
3. **Frontend never receives API keys** — only a provider ID reference
4. **LLM responses are streamed** via Tauri Events (not returned from invoke)
5. **Cancellation is supported** via abort signal → Rust drops the HTTP connection

---

## 4. Tool Definition Standard

### 4.1 AgentTool Interface

Every tool follows the pi-agent-core `AgentTool` interface:

```typescript
import { type AgentTool, type AgentToolResult, type AgentToolUpdateCallback } from "@mariozechner/pi-agent-core";
import { Type, type Static, type TSchema } from "@sinclair/typebox";

interface AgentTool<TParameters extends TSchema, TDetails = any> {
  name: string;           // Unique tool identifier (snake_case)
  label: string;          // Human-readable name
  description: string;    // Description for the LLM (what the tool does, when to use it)
  parameters: TParameters; // TypeBox schema for input validation
  execute: (
    toolCallId: string,
    params: Static<TParameters>,
    signal?: AbortSignal,
    onUpdate?: AgentToolUpdateCallback<TDetails>
  ) => Promise<AgentToolResult<TDetails>>;
}

// AgentToolResult: what execute() must return
interface AgentToolResult<T = any> {
  content: (TextContent | ImageContent)[];
  details: T;
}

interface TextContent {
  type: "text";
  text: string;
}

interface ImageContent {
  type: "image";
  data: string;      // base64 encoded
  mimeType: string;
}

// AgentToolUpdateCallback: for progress reporting
type AgentToolUpdateCallback<T = any> = (update: T) => void;
```

### 4.2 Tool Categories

Tools are organized into categories:

| Category | Prefix | Description |
|----------|--------|-------------|
| Container | `container_` | Container lifecycle and interaction |
| Filesystem | `file_` | File read/write within containers |
| Shell | `shell_` | Command execution in containers |
| MCP | `mcp_` | MCP server tool forwarding |
| System | `docker_`, `system_`, `runtime_` | System and runtime status queries |

### 4.3 Parameter Schema (TypeBox)

Tool parameters use TypeBox for JSON Schema generation and runtime validation:

```typescript
import { Type } from "@sinclair/typebox";

const ContainerCreateParams = Type.Object({
  templateId: Type.String({
    description: "Container template ID (e.g., 'node-dev', 'python-dev', 'rust-dev')",
  }),
  name: Type.Optional(Type.String({
    description: "Custom container name",
  })),
  cpuCores: Type.Optional(Type.Number({
    description: "CPU cores (1-16)",
    minimum: 1,
    maximum: 16,
  })),
  memoryMb: Type.Optional(Type.Number({
    description: "Memory in MB (256-65536)",
    minimum: 256,
    maximum: 65536,
  })),
  env: Type.Optional(Type.Record(Type.String(), Type.String(), {
    description: "Environment variables",
  })),
});

type ContainerCreateInput = Static<typeof ContainerCreateParams>;
```

### 4.4 Error Handling: Throw, Don't Return

Tools must **throw errors** — never return error objects. pi-agent-core catches thrown errors and feeds them back to the LLM for self-correction.

```typescript
// CORRECT: throw on error
async function execute(
  toolCallId: string,
  params: Static<typeof ContainerCreateParams>,
  signal?: AbortSignal,
  onUpdate?: AgentToolUpdateCallback
): Promise<AgentToolResult> {
  const result = await invoke("container_create", { req: params });
  if (!result) {
    throw new Error("Container creation failed: no result returned from backend");
  }
  return {
    content: [{ type: "text", text: JSON.stringify(result) }],
    details: result,
  };
}

// WRONG: returning error
async function execute(
  toolCallId: string,
  params: Static<typeof ContainerCreateParams>,
  signal?: AbortSignal,
  onUpdate?: AgentToolUpdateCallback
): Promise<AgentToolResult> {
  try {
    const result = await invoke("container_create", { req: params });
    return {
      content: [{ type: "text", text: JSON.stringify(result) }],
      details: result,
    };
  } catch (err) {
    return { content: [{ type: "text", text: String(err) }], details: { error: true } }; // DON'T DO THIS — throw instead
  }
}
```

### 4.5 Progress Updates (onUpdate)

Long-running tools report progress via the `onUpdate` callback:

```typescript
async function execute(
  toolCallId: string,
  params: { containerId: string; command: string },
  signal?: AbortSignal,
  onUpdate?: AgentToolUpdateCallback<{ status: string; message: string }>
): Promise<AgentToolResult> {
  onUpdate?.({
    status: "running",
    message: `Executing command in container ${params.containerId}...`,
  });

  const result = await invoke("container_exec", {
    id: params.containerId,
    command: params.command,
  });

  onUpdate?.({
    status: "completed",
    message: "Command completed",
  });

  return {
    content: [{ type: "text", text: JSON.stringify(result) }],
    details: result,
  };
}
```

### 4.6 Complete Tool Example

```typescript
// tools/containerTools.ts
import { Type, type Static } from "@sinclair/typebox";
import { invoke } from "@tauri-apps/api/core";
import type { AgentTool, AgentToolResult, AgentToolUpdateCallback } from "@mariozechner/pi-agent-core";

const ContainerListParams = Type.Object({
  status: Type.Optional(
    Type.Union([
      Type.Literal("all"),
      Type.Literal("running"),
      Type.Literal("stopped"),
    ], {
      description: "Filter by container status. Defaults to 'all'.",
    })
  ),
});

interface ContainerListDetails {
  count: number;
  containers: Array<{
    id: string;
    name: string;
    status: string;
    template: string;
    cpu: number;
    memory: string;
    ports: string[];
  }>;
}

export const containerListTool: AgentTool<typeof ContainerListParams, ContainerListDetails> = {
  name: "container_list",
  label: "List Containers",
  description:
    "List all managed containers/sandboxes. Returns container IDs, names, " +
    "status, template, resource allocation, and port mappings. " +
    "Use this to check what containers are available before performing operations.",
  parameters: ContainerListParams,
  execute: async (
    toolCallId: string,
    params: Static<typeof ContainerListParams>,
    signal?: AbortSignal,
    onUpdate?: AgentToolUpdateCallback<ContainerListDetails>
  ): Promise<AgentToolResult<ContainerListDetails>> => {
    onUpdate?.({ count: 0, containers: [] });

    const containers = await invoke<ContainerInfo[]>("container_list", {
      status: params.status ?? "all",
    });

    const details: ContainerListDetails = {
      count: containers.length,
      containers: containers.map((c) => ({
        id: c.shortId,
        name: c.name,
        status: c.status,
        template: c.templateId,
        cpu: c.cpuCores,
        memory: `${c.memoryMb}MB`,
        ports: c.ports.map((p) => `${p.hostPort}:${p.containerPort}`),
      })),
    };

    return {
      content: [{ type: "text", text: JSON.stringify(details, null, 2) }],
      details,
    };
  },
};
```

---

## 5. Built-in Tools Catalog

Container, image, shell, and system tools operate against CrateBay's **Docker-compatible control-plane boundary**. For runtime-related work, the **built-in runtime** is the primary product path; Podman is only a fallback / escape hatch and must not be treated as a parallel roadmap track by agents.

### 5.1 Container Tools

| Tool | Description | Risk Level | Parameters |
|------|-------------|-----------|------------|
| `container_create` | Create a new container from a template | medium | `templateId`, `name?`, `cpuCores?`, `memoryMb?`, `env?`, `ttlHours?` |
| `container_list` | List all managed containers | low | `status?` |
| `container_inspect` | Get detailed info about a container | low | `containerId` |
| `container_start` | Start a stopped container | low | `containerId` |
| `container_stop` | Stop a running container | medium | `containerId` |
| `container_delete` | Delete a container permanently | high | `containerId` |
| `container_exec` | Execute a command inside a container | medium | `containerId`, `command` |
| `container_logs` | Get container stdout/stderr logs | low | `containerId`, `tail?`, `since?` |

### 5.2 Image Tools

| Tool | Description | Risk Level | Parameters |
|------|-------------|-----------|------------|
| `image_list` | List all local images | low | — |
| `image_search` | Search registry images (Docker Hub via engine search API) | low | `query`, `limit?` |
| `image_pull` | Pull an image by reference | medium | `image`, `mirrors?` |
| `image_remove` | Remove a local image | high | `imageId`, `force?` |
| `image_inspect` | Inspect a local image | low | `imageId` |
| `image_tag` | Tag a local image | medium | `sourceImage`, `targetImage` |

### 5.3 Filesystem Tools

| Tool | Description | Risk Level | Parameters |
|------|-------------|-----------|------------|
| `file_read` | Read a file from a container | low | `containerId`, `path` |
| `file_write` | Write content to a file in a container | medium | `containerId`, `path`, `content` |
| `file_list` | List files/directories in a container path | low | `containerId`, `path?` |

### 5.4 Shell Tools

| Tool | Description | Risk Level | Parameters |
|------|-------------|-----------|------------|
| `shell_exec` | Execute a shell command in a container | medium | `containerId`, `command`, `timeout?` |

### 5.5 MCP Tools

| Tool | Description | Risk Level | Parameters |
|------|-------------|-----------|------------|
| `mcp_list_tools` | List available tools from all connected MCP servers | low | — |
| `mcp_call_tool` | Call a tool on a specific MCP server | varies* | `serverId`, `toolName`, `arguments?` |

*Risk level for `mcp_call_tool` depends on the target tool. Destructive MCP tools are detected by keyword matching (see §9).

### 5.6 System Tools

| Tool | Description | Risk Level | Parameters |
|------|-------------|-----------|------------|
| `docker_status` | Check Docker daemon connectivity, version info, and engine source (`podman` means fallback / explicit override) | low | — |
| `system_info` | Get host system information (OS, architecture, resources) | low | — |
| `runtime_status` | Check CrateBay built-in container runtime status | low | — |

### Tool Categories Summary

| Category | Prefix | Description |
|----------|--------|-------------|
| Container | `container_` | Container lifecycle and interaction |
| Image | `image_` | Image management (list, search, pull, remove, inspect, tag) |
| Filesystem | `file_` | File read/write within containers |
| Shell | `shell_` | Command execution in containers |
| MCP | `mcp_` | MCP server tool forwarding |
| System | `docker_`, `system_`, `runtime_` | System and runtime status queries |

### Tool Registry

All tools are registered in a central registry:

```typescript
// tools/index.ts
import { containerTools } from "./containerTools";
import { filesystemTools } from "./filesystemTools";
import { shellTools } from "./shellTools";
import { mcpTools } from "./mcpTools";
import { systemTools } from "./systemTools";

export const allTools: AgentTool<any>[] = [
  ...containerTools,
  ...filesystemTools,
  ...shellTools,
  ...mcpTools,
  ...systemTools,
];
```

---

## 6. System Prompt Design

The system prompt defines the agent's persona, capabilities, and behavioral constraints.

```typescript
// lib/systemPrompt.ts
export const systemPrompt = `You are CrateBay AI Assistant, a helpful development environment manager.

## Capabilities
You can manage containers, execute commands, read/write files, and interact with MCP tools.

## Available Tools
${toolDescriptions}

## Behavioral Rules

1. **Safety First**: Always check container status before operations. Never delete containers without user confirmation.

2. **Explain Before Acting**: For destructive or complex operations, explain what you plan to do before executing tools.

3. **Error Recovery**: If a tool call fails, analyze the error, explain it to the user, and suggest alternatives.

4. **Efficiency**: Prefer single commands over multiple when possible. Use file_list before file_read to confirm paths.

5. **Context Awareness**: Track which containers exist and their states. Don't attempt operations on non-existent containers.

## Response Format
- Use markdown for formatted responses
- Include code blocks with language annotations
- For multi-step operations, use numbered lists
- Keep explanations concise but informative

## Container Templates
Available templates: node-dev, python-dev, rust-dev, ubuntu-base
Each template provides a pre-configured development environment.

## Restrictions
- You cannot access the host filesystem directly — only files inside containers
- You cannot modify application settings — direct users to the Settings page
- You cannot manage LLM providers — direct users to Settings > LLM Providers
- API keys are managed securely and you have no access to them
`;
```

### Dynamic Prompt Sections

The system prompt is partially dynamic — the tool descriptions section is generated from the registered tools:

```typescript
const toolDescriptions = allTools
  .map((t) => `- **${t.name}**: ${t.description}`)
  .join("\n");
```

---

## 7. Conversation Persistence

Chat sessions and messages are persisted to SQLite via Tauri commands.

### Save Flow

```
User sends message
    │
    ▼
chatStore.addMessage()     ← optimistic UI update
    │
    ▼
invoke("conversation_save_message", { sessionId, message })
    │                              ← async, non-blocking
    ▼
Agent processes and responds
    │
    ▼
chatStore.addMessage()     ← assistant message
    │
    ▼
invoke("conversation_save_message", { sessionId, message })
```

### Load Flow (App Startup)

```typescript
// On app startup or session switch
async function loadSession(sessionId: string) {
  const messages = await invoke<ChatMessage[]>(
    "conversation_get_messages",
    { sessionId }
  );
  chatStore.setState((state) => ({
    messages: {
      ...state.messages,
      [sessionId]: messages,
    },
  }));
}
```

### Tauri Commands for Persistence

| Command | Parameters | Returns | Description |
|---------|-----------|---------|-------------|
| `conversation_list` | — | `ChatSession[]` | List all sessions |
| `conversation_create` | `title?` | `ChatSession` | Create new session |
| `conversation_delete` | `sessionId` | — | Delete session and messages |
| `conversation_get_messages` | `sessionId` | `ChatMessage[]` | Get all messages |
| `conversation_save_message` | `sessionId`, `message` | — | Save a message |
| `conversation_update_title` | `sessionId`, `title` | — | Update session title |

---

## 8. Multi-turn Context Management

### 8.1 transformContext

Before sending messages to the LLM, the conversation history is transformed to fit within the token budget:

```typescript
function transformContext(
  messages: ChatMessage[],
  maxTokens: number
): AgentMessage[] {
  // 1. Always include system prompt (first message)
  const systemMsg = messages.find((m) => m.role === "system");

  // 2. Always include the latest user message
  const latestUser = messages.filter((m) => m.role === "user").at(-1);

  // 3. Include recent messages up to token budget
  const recentMessages: AgentMessage[] = [];
  let tokenCount = estimateTokens(systemMsg) + estimateTokens(latestUser);

  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    const tokens = estimateTokens(msg);
    if (tokenCount + tokens > maxTokens) break;
    recentMessages.unshift(convertToAgentMessage(msg));
    tokenCount += tokens;
  }

  return [
    convertToAgentMessage(systemMsg!),
    ...recentMessages,
  ];
}
```

### 8.2 convertToLlm

Converts CrateBay's internal `ChatMessage` format to pi-agent-core's `AgentMessage` format:

```typescript
function convertToAgentMessage(msg: ChatMessage): AgentMessage {
  const agentMsg: AgentMessage = {
    role: msg.role,
    content: msg.content,
  };

  // Convert tool call info to AgentToolCall format
  if (msg.toolCalls && msg.toolCalls.length > 0) {
    agentMsg.toolCalls = msg.toolCalls.map((tc) => ({
      id: tc.id,
      type: "function" as const,
      function: {
        name: tc.toolName,
        arguments: JSON.stringify(tc.parameters),
      },
    }));
  }

  // Tool result messages need toolCallId
  if (msg.role === "tool" && msg.metadata?.toolCallId) {
    agentMsg.toolCallId = msg.metadata.toolCallId as string;
    agentMsg.name = msg.metadata.toolName as string;
  }

  return agentMsg;
}
```

### 8.3 Token Estimation

Simple token estimation (no need for a full tokenizer in the frontend):

```typescript
function estimateTokens(msg: ChatMessage | AgentMessage | undefined): number {
  if (!msg) return 0;
  // Rough estimate: 1 token ≈ 4 characters for English, 2 characters for CJK
  const content = typeof msg === "string" ? msg : msg.content;
  return Math.ceil(content.length / 3);
}
```

### 8.4 Context Window Strategy

| Provider | Context Window | Strategy |
|----------|---------------|----------|
| GPT-4o | 128K tokens | Keep full history up to ~100K, reserve 28K for response |
| Claude 3.5 | 200K tokens | Keep full history up to ~160K, reserve 40K for response |
| Gemini 1.5 | 1M tokens | Keep full history up to ~900K, reserve 100K for response |
| Default | 32K tokens | Keep recent 24K, reserve 8K for response |

The context window size is read from the active provider configuration in `settingsStore`.

---

## 9. Safety: Risk Levels and Confirmation

### 9.1 Risk Level Definitions

| Level | Description | Confirmation Required | Examples |
|-------|-------------|----------------------|----------|
| `low` | Read-only, no side effects | No | `container_list`, `file_read`, `mcp_list_tools` |
| `medium` | Creates resources or modifies state reversibly | No (default) / Yes (if `confirmDestructiveOps` enabled) | `container_create`, `file_write`, `shell_exec` |
| `high` | Destroys resources or makes significant changes | Yes, always | `container_delete` |
| `critical` | System-level impact, irreversible | Yes, with typed confirmation | (reserved for future operations) |

### 9.2 Risk Level Assignment

Each tool has a static risk level. For `mcp_call_tool`, risk is dynamically determined:

```typescript
// Risk level per built-in tool
const toolRiskLevels: Record<string, RiskLevel> = {
  container_list: "low",
  container_inspect: "low",
  container_logs: "low",
  container_start: "low",
  container_create: "medium",
  container_stop: "medium",
  container_exec: "medium",
  container_delete: "high",
  file_read: "low",
  file_list: "low",
  file_write: "medium",
  shell_exec: "medium",
  mcp_list_tools: "low",
  mcp_call_tool: "medium", // baseline, may be elevated
  docker_status: "low",
  system_info: "low",
  runtime_status: "low",
};

// Dynamic risk detection for MCP tool calls
const destructiveKeywords = [
  "delete", "remove", "destroy", "drop", "wipe",
  "prune", "terminate", "kill", "purge", "reset",
];

function getMcpToolRiskLevel(toolName: string): RiskLevel {
  const lower = toolName.toLowerCase();
  if (destructiveKeywords.some((kw) => lower.includes(kw))) {
    return "high";
  }
  return "medium";
}
```

### 9.3 Confirmation Flow

When a tool requires confirmation, the execution is paused and a `ConfirmDialog` is shown:

```typescript
// In beforeToolCall callback (see §2.1)
// The confirmation logic is implemented in the beforeToolCall hook
// rather than wrapping tool.execute directly.

// For tools that need runtime risk elevation (e.g., mcp_call_tool),
// the risk level is checked at execution time:
async function executeWithConfirmation(
  toolCallId: string,
  tool: AgentTool<any>,
  params: Record<string, unknown>,
  signal?: AbortSignal,
  onUpdate?: AgentToolUpdateCallback
): Promise<AgentToolResult> {
  const riskLevel = getToolRiskLevel(tool.name, params);

  if (shouldConfirm(riskLevel)) {
    const approved = await workflowStore.requestConfirmation({
      toolName: tool.name,
      toolLabel: tool.label,
      description: buildConfirmationDescription(tool, params),
      riskLevel,
      parameters: params,
      consequences: buildConsequencesList(tool, params),
    });

    if (!approved) {
      throw new Error(`User cancelled ${tool.label} operation`);
    }
  }

  return tool.execute(toolCallId, params, signal, onUpdate);
}

function shouldConfirm(riskLevel: RiskLevel): boolean {
  if (riskLevel === "high" || riskLevel === "critical") return true;
  if (riskLevel === "medium") {
    return settingsStore.getState().settings.confirmDestructiveOps;
  }
  return false;
}
```

### 9.4 Consequence Descriptions

For high-risk operations, the confirmation dialog displays specific consequences:

```typescript
function buildConsequencesList(
  tool: AgentTool,
  params: Record<string, unknown>
): string[] {
  switch (tool.name) {
    case "container_delete":
      return [
        `Container "${params.containerId}" will be permanently deleted`,
        "All data inside the container will be lost",
        "Running processes will be terminated",
        "This action cannot be undone",
      ];
    default:
      return [`This operation will execute ${tool.label}`];
  }
}
```

### 9.5 Agent Error Recovery

When the user cancels an operation, the error message is fed back to the LLM. The agent can then:

1. Explain why the operation was cancelled
2. Suggest alternatives
3. Ask the user for clarification

```
Agent: I'll delete container "node-01" for you.
       [ConfirmDialog: Delete container node-01?]
User:  [Cancel]
Agent: Understood, I won't delete node-01. Would you like me to stop it instead,
       or is there something else you'd like to do with it?
```
