# Frontend Specification

> Version: 1.2.7 | Last Updated: 2026-03-26 | Author: frontend-architect

---

## Table of Contents

1. [Technology Stack & Versions](#1-technology-stack--versions)
2. [Project Structure](#2-project-structure)
3. [shadcn/ui Component Usage](#3-shadcnui-component-usage)
4. [Zustand Store Design](#4-zustand-store-design)
5. [Page Architecture](#5-page-architecture)
6. [Routing Strategy](#6-routing-strategy)
7. [Chat UI Components](#7-chat-ui-components)
8. [Responsive Layout](#8-responsive-layout)
9. [i18n Strategy](#9-i18n-strategy)
10. [Tauri Integration](#10-tauri-integration)
11. [Dependency Budget](#11-dependency-budget)
12. [Code Style](#12-code-style)

---

## 1. Technology Stack & Versions

| Package | Version | Purpose |
|---------|---------|---------|
| React | 19.x | UI framework with concurrent features |
| shadcn/ui | latest | Composable UI component system (copy-paste, not npm) |
| Radix UI | latest | Accessible headless primitives (underlying shadcn) |
| Tailwind CSS | v4 | Utility-first CSS with CSS-first configuration |
| Zustand | latest | Lightweight state management (~2KB) |
| Streamdown | latest | Streaming markdown renderer (Vercel) with native shadcn integration |
| @mariozechner/pi-agent-core | latest | Agent orchestration engine (TS) |
| @mariozechner/pi-ai | latest | Unified multi-provider LLM API |
| tauri-specta | latest | Auto-generated TypeScript bindings for Tauri commands |
| Vite | 6.x | Build tool with HMR |
| TypeScript | 5.x | Strict mode, no `any` |
| Vitest | latest | Unit & component testing |
| Playwright | latest | E2E testing |

### Tailwind CSS v4 Notes

CrateBay uses Tailwind CSS v4 with its CSS-first configuration model:

- Configuration lives in `src/app.css` using `@theme` directive (no `tailwind.config.js`)
- CSS variables define the design tokens
- `@import "tailwindcss"` replaces the old `@tailwind` directives
- Arbitrary values and variants remain supported

```css
/* src/app.css */
@import "tailwindcss";

@theme {
  --color-primary: oklch(0.546 0.245 262.881);
  --color-accent: oklch(0.777 0.152 181.912);
  --color-background: oklch(0.141 0.005 285.823);
  --color-foreground: oklch(0.985 0.002 247.858);
  --color-muted: oklch(0.274 0.006 286.033);
  --color-muted-foreground: oklch(0.705 0.015 286.067);
  --color-card: oklch(0.178 0.006 285.823);
  --color-border: oklch(0.274 0.006 286.033);
  --color-destructive: oklch(0.577 0.245 27.325);
  --color-success: oklch(0.696 0.17 162.48);
  --radius-sm: 0.25rem;
  --radius-md: 0.375rem;
  --radius-lg: 0.5rem;
  --radius-xl: 0.75rem;
}
```

---

## 2. Project Structure

```
crates/cratebay-gui/src/
├── main.tsx                    # React entry point, render <App />
├── App.tsx                     # Root component: layout + routing
├── app.css                     # Global styles + Tailwind @theme
├── lib/
│   ├── utils.ts                # cn() helper, common utilities
│   ├── tauri.ts                # Tauri invoke/listen wrappers (specta-generated)
│   └── constants.ts            # App-wide constants
├── types/
│   ├── index.ts                # Re-exports
│   ├── container.ts            # Container/sandbox types
│   ├── chat.ts                 # Chat/message types
│   ├── mcp.ts                  # MCP server/tool types
│   ├── agent.ts                # Agent/tool types
│   └── settings.ts             # Settings types
├── stores/
│   ├── appStore.ts             # App-level state (theme, sidebar, page)
│   ├── chatStore.ts            # Chat sessions, messages, streaming
│   ├── containerStore.ts       # Container list, status, operations
│   ├── mcpStore.ts             # MCP server connections, tools
│   ├── settingsStore.ts        # User preferences, API key refs
│   └── workflowStore.ts       # Agent workflow state, confirmations
├── hooks/
│   ├── useTauriEvent.ts        # Subscribe to Tauri events with auto-cleanup
│   ├── useStreamingMessage.ts  # Handle streaming LLM responses
│   ├── useContainerActions.ts  # Container CRUD hook
│   └── useAgent.ts             # pi-agent-core lifecycle hook
├── components/
│   ├── ui/                     # shadcn/ui primitives (auto-generated)
│   │   ├── button.tsx
│   │   ├── dialog.tsx
│   │   ├── input.tsx
│   │   ├── scroll-area.tsx
│   │   ├── tooltip.tsx
│   │   └── ...                 # Other shadcn components
│   ├── layout/
│   │   ├── AppLayout.tsx       # Main layout: sidebar + content
│   │   ├── Sidebar.tsx         # Navigation sidebar
│   │   ├── TopBar.tsx          # Top bar with breadcrumbs
│   │   └── StatusBar.tsx       # Bottom status bar (Docker, runtime)
│   ├── chat/
│   │   ├── ChatInput.tsx       # Input with @mention autocomplete
│   │   ├── MessageList.tsx     # Scrollable message list
│   │   ├── MessageBubble.tsx   # Single message with Streamdown
│   │   ├── AgentThinking.tsx   # Reasoning chain display
│   │   ├── ToolCallCard.tsx    # Tool execution status card
│   │   └── ConfirmDialog.tsx   # Destructive operation confirmation
│   ├── container/
│   │   ├── ContainerList.tsx   # Container table/grid
│   │   ├── ContainerCard.tsx   # Single container status card
│   │   ├── ContainerDetail.tsx # Container inspect view
│   │   ├── ContainerLogs.tsx   # Container stdout/stderr log viewer
│   │   ├── ContainerMonitoring.tsx # Container CPU/MEM stats panel
│   │   └── TerminalView.tsx    # Container exec terminal
│   └── mcp/
│       ├── McpServerList.tsx   # MCP server list with status
│       ├── McpToolList.tsx     # Available tools from MCP servers
│       └── McpServerConfig.tsx # Server add/edit form
├── pages/
│   ├── ChatPage.tsx            # Default page — Chat-First interface
│   ├── ContainersPage.tsx      # Unified container management
│   ├── ImagesPage.tsx          # Container image management (pull, list, delete)
│   ├── McpPage.tsx             # MCP server management
│   └── SettingsPage.tsx        # App settings and preferences
└── tools/
    ├── index.ts                # Tool registry — exports all AgentTools
    ├── containerTools.ts       # container_create, container_list, etc.
    ├── filesystemTools.ts      # file_read, file_write, file_list
    ├── shellTools.ts           # shell_exec
    └── mcpTools.ts             # mcp_call_tool, mcp_list_tools
```

### Key Conventions

- **`components/ui/`**: Auto-generated by `npx shadcn add`. Never manually edit.
- **`components/{domain}/`**: Domain-specific composite components. May compose `ui/` primitives.
- **`pages/`**: Top-level page components. One per navigable view.
- **`stores/`**: One Zustand store per domain. No cross-store direct imports (use selectors).
- **`tools/`**: AgentTool definitions for pi-agent-core. Each file exports an array of tools.
- **`hooks/`**: Custom React hooks. Must start with `use` prefix.
- **`types/`**: TypeScript type definitions. Augmented by tauri-specta auto-generated types.

---

## 3. shadcn/ui Component Usage

### 3.1 Registry and Installation

shadcn/ui components are installed via CLI and live as source code in the project:

```bash
# Install a component
npx shadcn@latest add button

# Install multiple components
npx shadcn@latest add dialog input scroll-area tooltip

# Search for components
npx shadcn@latest search "data table"
```

Configuration is in `components.json` at the frontend root:

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "src/app.css",
    "baseColor": "zinc",
    "cssVariables": true
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  }
}
```

### 3.2 Customization Rules

1. **Never modify files in `components/ui/`** directly. Override via Tailwind classes or wrapper components.
2. **Compose, don't fork.** Build domain components by composing shadcn primitives.
3. **Use `cn()` for conditional classes:**

```typescript
import { cn } from "@/lib/utils";

function StatusBadge({ active }: { active: boolean }) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-2 py-1 text-xs font-medium",
        active
          ? "bg-success/20 text-success"
          : "bg-muted text-muted-foreground"
      )}
    >
      {active ? "Running" : "Stopped"}
    </span>
  );
}
```

### 3.3 Theme System

CrateBay uses CSS variables for theming, supporting dark and light modes:

```css
/* Light mode overrides (applied via class on <html>) */
.light {
  --color-background: oklch(1 0 0);
  --color-foreground: oklch(0.145 0.005 285.823);
  --color-card: oklch(1 0 0);
  --color-border: oklch(0.922 0.004 286.033);
  --color-muted: oklch(0.967 0.001 286.033);
  --color-muted-foreground: oklch(0.556 0.014 286.067);
}
```

**Default is dark mode.** Theme switching is handled by `appStore.theme`:

```typescript
// Toggle theme
const toggleTheme = () => {
  const next = theme === "dark" ? "light" : "dark";
  document.documentElement.classList.toggle("light", next === "light");
  set({ theme: next });
};
```

### 3.4 Accessibility Requirements

- All interactive components must be keyboard navigable (handled by Radix primitives).
- Focus rings must be visible: use `focus-visible:ring-2 focus-visible:ring-primary` pattern.
- Color contrast must meet WCAG 2.1 AA (4.5:1 for normal text, 3:1 for large text).
- ARIA labels required for icon-only buttons.
- Tooltips for all icon buttons via `<Tooltip>` from shadcn.
- Screen reader announcements for state changes (loading, errors) via `aria-live` regions.

---

## 4. Zustand Store Design

CrateBay uses 6 Zustand stores, one per domain. Stores are independent — no direct cross-store imports. Components select from multiple stores as needed.

### 4.1 appStore

Manages application-level state: theme, sidebar, current page, global status.

```typescript
type DockerSource = "builtin" | "colima" | "other" | null;

interface AppState {
  // Navigation
  currentPage: "chat" | "containers" | "images" | "mcp" | "settings";
  setCurrentPage: (page: AppState["currentPage"]) => void;

  // Theme
  theme: "dark" | "light";
  toggleTheme: () => void;

  // Sidebar
  sidebarOpen: boolean;
  sidebarWidth: number;
  toggleSidebar: () => void;
  setSidebarWidth: (width: number) => void;

  // Global status
  dockerConnected: boolean;
  runtimeStatus: "starting" | "running" | "stopped" | "error";
  setDockerConnected: (connected: boolean) => void;
  setRuntimeStatus: (status: AppState["runtimeStatus"]) => void;

  // Built-in runtime status (decoupled from external Docker)
  builtinRuntimeReady: boolean;
  setBuiltinRuntimeReady: (ready: boolean) => void;

  // Which Docker backend is currently connected
  dockerSource: DockerSource;
  setDockerSource: (source: DockerSource) => void;

  // Runtime control operations
  runtimeLoading: boolean;
  setRuntimeLoading: (loading: boolean) => void;

  // Notifications
  notifications: Notification[];
  addNotification: (n: Omit<Notification, "id" | "timestamp">) => void;
  dismissNotification: (id: string) => void;
}

interface Notification {
  id: string;
  type: "info" | "success" | "warning" | "error";
  title: string;
  message?: string;
  timestamp: number;
  dismissable: boolean;
}
```

**Runtime status handling**

- Initial values come from `docker_status` + `runtime_status` on app startup.
- Ongoing updates come from the global `runtime:health` event (every ~20s).
- The frontend normalizes backend Docker source signals into `appStore.dockerSource`: `"builtin"` for CrateBay Runtime, `"colima"` for Colima fallback, `"other"` for other external Docker-compatible backends, and `null` when unavailable or unknown.
- `builtinRuntimeReady` becomes `true` only when Docker is responsive and `dockerSource === "builtin"`.
- External Docker / Podman fallback must not be presented as a separate first-class runtime mode in the UI.
- To avoid transient UI flicker (e.g., brief ping misses), the UI applies a downgrade grace window (`RUNTIME_HEALTH_DOWNGRADE_GRACE_MS`, default 90s) before showing `running → starting`.

### 4.2 chatStore

Manages chat sessions, messages, and streaming state.

```typescript
interface ChatState {
  // Sessions
  sessions: ChatSession[];
  activeSessionId: string | null;
  createSession: () => ChatSession;
  deleteSession: (id: string) => void;
  setActiveSession: (id: string) => void;

  // Messages
  messages: Record<string, ChatMessage[]>; // sessionId → messages
  addMessage: (sessionId: string, message: ChatMessage) => void;
  updateMessage: (sessionId: string, messageId: string, patch: Partial<ChatMessage>) => void;

  // Streaming
  isStreaming: boolean;
  streamingMessageId: string | null;
  appendStreamChunk: (sessionId: string, messageId: string, chunk: string) => void;
  setStreaming: (streaming: boolean, messageId?: string) => void;

  // Agent state
  agentThinking: boolean;
  agentThinkingContent: string | null;
  setAgentThinking: (thinking: boolean, content?: string) => void;

  // Input
  inputDraft: string;
  setInputDraft: (draft: string) => void;
  mentionQuery: string | null;
  setMentionQuery: (query: string | null) => void;
}

interface ChatSession {
  id: string;
  title: string;
  createdAt: string; // ISO 8601
  updatedAt: string;
  messageCount: number;
}

interface ChatMessage {
  id: string;
  sessionId: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  timestamp: string;
  status: "sending" | "streaming" | "complete" | "error";
  toolCalls?: ToolCallInfo[];
  reasoning?: string; // agent thinking/reasoning content
  metadata?: Record<string, unknown>;
}

interface ToolCallInfo {
  id: string;
  toolName: string;
  toolLabel: string;
  parameters: Record<string, unknown>;
  result?: unknown;
  status: "pending" | "running" | "success" | "error";
  error?: string;
  startedAt?: string;
  completedAt?: string;
}
```

### 4.3 containerStore

Manages container list, status, and operations.

```typescript
interface ContainerState {
  // Container list
  containers: ContainerInfo[];
  loading: boolean;
  error: string | null;
  fetchContainers: () => Promise<void>;

  // Selection
  selectedContainerId: string | null;
  selectContainer: (id: string | null) => void;

  // Operations
  createContainer: (req: ContainerCreateRequest) => Promise<ContainerInfo>;
  startContainer: (id: string) => Promise<void>;
  stopContainer: (id: string) => Promise<void>;
  deleteContainer: (id: string) => Promise<void>;

  // Templates
  templates: ContainerTemplate[];
  fetchTemplates: () => Promise<void>;

  // Filters
  filter: ContainerFilter;
  setFilter: (filter: Partial<ContainerFilter>) => void;
}

interface ContainerInfo {
  id: string;
  shortId: string;
  name: string;
  image: string;
  // Backend statuses (Docker) + frontend placeholder.
  // `creating` is frontend-only for optimistic UI placeholders.
  status:
    | "running"
    | "stopped"
    | "created"
    | "restarting"
    | "removing"
    | "paused"
    | "exited"
    | "dead"
    | "creating";
  state: string;
  createdAt: string;
  ports: PortMapping[];
  labels: Record<string, string>;
  cpuCores?: number;
  memoryMb?: number;
}

interface ContainerCreateRequest {
  name: string;
  image: string;
  templateId?: string;
  command?: string;
  env?: string[]; // ["KEY=VALUE", ...]
  cpuCores?: number;
  memoryMb?: number;
  autoStart?: boolean;
}

interface ContainerTemplate {
  id: string;
  name: string;
  description: string;
  image: string;
  defaultCommand: string;
  defaultCpuCores: number;
  defaultMemoryMb: number;
  tags: string[];
}

interface ContainerFilter {
  status: "all" | "running" | "stopped" | "creating";
  search: string;
  templateId: string | null;
}

interface PortMapping {
  hostPort: number;
  containerPort: number;
  protocol: "tcp" | "udp";
}
```

### 4.4 mcpStore

Manages MCP server connections and available tools.

```typescript
interface McpState {
  // Servers
  servers: McpServerInfo[];
  loading: boolean;
  fetchServers: () => Promise<void>;

  // Server operations
  startServer: (id: string) => Promise<void>;
  stopServer: (id: string) => Promise<void>;
  addServer: (config: McpServerConfig) => Promise<void>;
  removeServer: (id: string) => Promise<void>;
  updateServer: (id: string, config: Partial<McpServerConfig>) => Promise<void>;

  // Tools from connected MCP servers
  availableTools: McpToolInfo[];
  fetchTools: () => Promise<void>;

  // Tool execution
  callTool: (serverId: string, toolName: string, args: Record<string, unknown>) => Promise<unknown>;

  // Server logs
  serverLogs: Record<string, string[]>; // serverId → log lines
  fetchServerLogs: (id: string) => Promise<void>;
}

interface McpServerInfo {
  id: string;
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
  status: "connected" | "disconnected" | "error" | "starting";
  transport: "stdio" | "sse";
  toolCount: number;
  lastConnectedAt?: string;
  error?: string;
}

interface McpServerConfig {
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  enabled?: boolean;
  transport?: "stdio" | "sse";
}

interface McpToolInfo {
  serverId: string;
  serverName: string;
  name: string;
  description: string;
  inputSchema: Record<string, unknown>; // JSON Schema
}
```

### 4.5 settingsStore

Manages user preferences, LLM providers, models, and application settings.

```typescript
interface SettingsState {
  // LLM Providers
  providers: LlmProviderInfo[];
  activeProviderId: string | null;
  fetchProviders: () => Promise<void>;
  setActiveProvider: (id: string) => void;
  createProvider: (request: LlmProviderCreateRequest) => Promise<LlmProviderInfo>;
  updateProvider: (id: string, request: LlmProviderUpdateRequest) => Promise<LlmProviderInfo>;
  deleteProvider: (id: string) => Promise<void>;
  testProvider: (id: string) => Promise<ProviderTestResult>;

  // Models
  models: Record<string, LlmModelInfo[]>;  // providerId → models[]
  activeModelId: string | null;
  setActiveModel: (modelId: string) => void;
  fetchModels: (providerId: string) => Promise<void>;
  toggleModel: (providerId: string, modelId: string, enabled: boolean) => Promise<void>;
  enabledModels: () => LlmModelInfo[];  // computed: all enabled models across providers

  // General settings
  settings: AppSettings;
  updateSettings: (patch: Partial<AppSettings>) => Promise<void>;
  fetchSettings: () => Promise<void>;

  // API Key management (keys never leave Rust backend)
  hasApiKey: (providerId: string) => boolean;
  saveApiKey: (providerId: string, key: string) => Promise<void>;
  deleteApiKey: (providerId: string) => Promise<void>;
}

/// Supported API format types (matches Rust ApiFormat enum)
type ApiFormat = "anthropic" | "openai_responses" | "openai_completions";

interface LlmProviderInfo {
  id: string;
  name: string;
  apiBase: string;          // Base URL (e.g., "https://api.openai.com")
  apiFormat: ApiFormat;     // Determines request structure and available options
  hasApiKey: boolean;       // true if key exists in backend (key value never exposed)
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
}

interface LlmProviderCreateRequest {
  name: string;
  apiBase: string;
  apiKey: string;           // Plaintext, encrypted on backend
  apiFormat: ApiFormat;
}

interface LlmProviderUpdateRequest {
  name?: string;
  apiBase?: string;
  apiKey?: string;          // If provided, re-encrypts the key
  apiFormat?: ApiFormat;
  enabled?: boolean;
}

interface LlmModelInfo {
  id: string;               // Model ID from API (e.g., "gpt-4o")
  providerId: string;
  name: string;
  isEnabled: boolean;       // User toggle state
  supportsReasoning: boolean; // Whether model supports reasoning effort
}

interface ProviderTestResult {
  success: boolean;
  latencyMs: number;
  model: string;
  error: string | null;
}

interface AppSettings {
  language: "en" | "zh-CN";
  theme: "dark" | "light" | "system";
  sendOnEnter: boolean;
  showAgentThinking: boolean;
  maxConversationHistory: number;
  containerDefaultTtlHours: number;
  confirmDestructiveOps: boolean;
  reasoningEffort: "low" | "medium" | "high"; // Global reasoning effort preference
  registryMirrors: string[];
  runtimeHttpProxy: string;
  runtimeHttpProxyBridge: boolean;
  runtimeHttpProxyBindHost: string;
  runtimeHttpProxyBindPort: number;
  runtimeHttpProxyGuestHost: string;
  allowExternalDocker: boolean; // Allow Colima / Docker Desktop fallback when built-in runtime is unavailable
}
```

#### Provider Settings UI

The Settings > LLM Providers tab presents a form-based UI for managing providers:

```
┌──────────────────────────────────────────────┐
│  LLM Providers                    [+ Add]    │
├──────────────────────────────────────────────┤
│                                              │
│  ┌────────────────────────────────────────┐  │
│  │ Provider Name:  [Anthropic         ]  │  │
│  │ Base URL:       [https://api.anthro]  │  │
│  │ API Key:        [•••••••••••••••••••]  │  │
│  │ API Format:     [▼ Anthropic Messages]  │  │
│  │                                        │  │
│  │ [Test Connection]  [Save]  [Delete]    │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  Models (from /v1/models):                    │
│  ┌────────────────────────────────────────┐  │
│  │ [✓] claude-3-5-sonnet-20241022           │  │
│  │ [✓] claude-3-5-haiku-20241022            │  │
│  │ [ ] claude-3-opus-20240229               │  │
│  │                                        │  │
│  │ [Refresh Models]                        │  │
│  └────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

**Provider form fields:**
- **Provider Name** (text input): Display name for the provider
- **Base URL** (text input): API base URL (e.g., `https://api.openai.com`)
- **API Key** (password input): Masked input, sent to backend for encrypted storage
- **API Format** (select dropdown): One of:
  - "Anthropic Messages" (`anthropic`)
  - "OpenAI Responses" (`openai_responses`)
  - "OpenAI Chat Completions" (`openai_completions`)

**Model selection:**
- Models are fetched from the provider's `/v1/models` endpoint via `llm_models_fetch`
- Displayed as a checkbox list where users can enable/disable individual models
- Only enabled models appear in the chat model selector
- "Refresh Models" button re-fetches from the remote API

#### Reasoning Effort Control

The reasoning effort control is **conditionally displayed** based on the active model's provider format:

```typescript
// Only show reasoning effort when active provider uses OpenAI Responses API format
const activeProvider = providers.find(p => p.id === activeProviderId);
const showReasoningEffort = activeProvider?.apiFormat === "openai_responses";
```

When visible, it renders as a segmented control with three options:
- **Low** — faster responses, less reasoning
- **Medium** — balanced (default)
- **High** — deeper reasoning, slower responses

This control appears in the chat interface (near the model selector), not in the Settings page. The value is stored in `settingsStore.settings.reasoningEffort` and passed to `llm_proxy_stream` via the `options.reasoning_effort` parameter.

### 4.6 workflowStore

Manages agent workflow state, confirmations, and execution progress.

```typescript
interface WorkflowState {
  // Agent lifecycle
  agentStatus: "idle" | "thinking" | "executing" | "waiting_confirmation" | "error";
  setAgentStatus: (status: WorkflowState["agentStatus"]) => void;

  // Current tool execution
  currentToolExecution: ToolExecution | null;
  setCurrentToolExecution: (exec: ToolExecution | null) => void;

  // Confirmation flow
  pendingConfirmation: ConfirmationRequest | null;
  requestConfirmation: (req: Omit<ConfirmationRequest, "id">) => Promise<boolean>;
  resolveConfirmation: (id: string, approved: boolean) => void;

  // Execution history (current session)
  executionHistory: ToolExecution[];
  addToHistory: (exec: ToolExecution) => void;
  clearHistory: () => void;

  // Follow-up suggestions
  followUpSuggestions: string[];
  setFollowUpSuggestions: (suggestions: string[]) => void;
}

interface ToolExecution {
  id: string;
  toolName: string;
  toolLabel: string;
  parameters: Record<string, unknown>;
  status: "pending" | "running" | "success" | "error";
  result?: unknown;
  error?: string;
  startedAt: string;
  completedAt?: string;
  riskLevel: "low" | "medium" | "high" | "critical";
}

interface ConfirmationRequest {
  id: string;
  toolName: string;
  toolLabel: string;
  description: string;
  riskLevel: "medium" | "high" | "critical";
  parameters: Record<string, unknown>;
  consequences: string[];
}
```

### Store Best Practices

1. **No cross-store imports.** Components select from multiple stores:
   ```typescript
   function ChatPage() {
     const { messages, isStreaming } = useChatStore();
     const { agentStatus } = useWorkflowStore();
     // ...
   }
   ```

2. **Async actions call Tauri invoke internally:**
   ```typescript
   const useContainerStore = create<ContainerState>()((set, get) => ({
     fetchContainers: async () => {
       set({ loading: true, error: null });
       try {
         const containers = await invoke<ContainerInfo[]>("container_list");
         set({ containers, loading: false });
       } catch (err) {
         set({ error: String(err), loading: false });
       }
     },
   }));
   ```

3. **Persist user preferences** via Tauri commands (SQLite), not localStorage.

4. **Immutable updates** via Zustand's built-in immer middleware where complex state is needed.

---

## 5. Page Architecture

### 5.1 ChatPage (Default — Chat-First)

The primary interface. Users interact with CrateBay through natural language. The agent can manage containers, execute commands, and configure MCP servers — all from the chat.

**Layout:**
```
┌─────────────────────────────────────────────┐
│  TopBar: session title, new chat, settings  │
├─────────────────────────────────────────────┤
│                                             │
│  MessageList (scrollable)                   │
│  ┌─────────────────────────────────────┐    │
│  │ User: "Create a Node.js sandbox"    │    │
│  │                                     │    │
│  │ Agent: [AgentThinking]              │    │
│  │        [ToolCallCard: creating...]  │    │
│  │        "Done! Sandbox `node-01` is  │    │
│  │         running on port 3000."      │    │
│  │                                     │    │
│  │ User: "Run npm install express"     │    │
│  │ ...                                 │    │
│  └─────────────────────────────────────┘    │
│                                             │
├─────────────────────────────────────────────┤
│  ChatInput: [@mention] [attachments] [send] │
│  FollowUp suggestions (chips)               │
└─────────────────────────────────────────────┘
```

**Key Behaviors:**
- Messages render with `Streamdown` for streaming markdown (code blocks, tables, etc.)
- Tool calls appear as inline `ToolCallCard` components within the message flow
- Agent thinking/reasoning is collapsible via `AgentThinking` component
- Destructive operations trigger `ConfirmDialog` before execution
- Session list in sidebar for conversation history

### 5.2 ContainersPage

Unified container and sandbox management — a visual dashboard for what the agent can also do via chat.

**Layout:**
```
┌──────────────────────────────────────────────┐
│  TopBar: "Containers" + Create button        │
├──────────┬───────────────────────────────────┤
│ Filters  │  ContainerList (table/grid)       │
│ ─────    │  ┌──────────────────────────────┐ │
│ Status   │  │ node-01  Running  2 CPU 2GB  │ │
│ Template │  │ py-dev   Stopped  1 CPU 1GB  │ │
│ Search   │  │ rust-01  Running  4 CPU 4GB  │ │
│          │  └──────────────────────────────┘ │
│          ├───────────────────────────────────┤
│          │  ContainerDetail (selected)       │
│          │  - Status, ports, logs, terminal  │
└──────────┴───────────────────────────────────┘
```

**Key Behaviors:**
- Toggle between table and grid view
- Real-time status updates via Tauri events
- Inline actions: start, stop, delete, open terminal
- Container detail panel with logs and exec terminal

### 5.3 ImagesPage

Container image management -- pull, list, and delete container images.

**Layout:**
```
┌──────────────────────────────────────────────┐
│  TopBar: "Images" + Pull Image button        │
├──────────────────────────────────────────────┤
│  Image List                                  │
│  ┌──────────────────────────────────────┐    │
│  │ node:20-alpine   150 MB   2 days ago │    │
│  │ python:3.12      200 MB   1 week ago │    │
│  │ alpine:latest     8 MB   3 days ago  │    │
│  └──────────────────────────────────────┘    │
├──────────────────────────────────────────────┤
│  Image Detail (selected)                     │
│  - Tags, size, layers, created date          │
│  - Delete button                             │
└──────────────────────────────────────────────┘
```

**Key Behaviors:**
- Lists all locally available container images from Docker
- Pull new images by name:tag with progress indication
- Delete unused images with confirmation dialog
- Display image metadata: repository, tags, size, creation date

### 5.4 McpPage

MCP server management — configure, monitor, and test MCP server connections.

**Layout:**
```
┌──────────────────────────────────────────────┐
│  TopBar: "MCP Servers" + Add Server button   │
├──────────────────────────────────────────────┤
│  McpServerList                               │
│  ┌──────────────────────────────────────┐    │
│  │ shadcn MCP    ● Connected   7 tools  │    │
│  │ cratebay-mcp  ○ Stopped     12 tools │    │
│  └──────────────────────────────────────┘    │
├──────────────────────────────────────────────┤
│  McpToolList (from selected server)          │
│  ┌──────────────────────────────────────┐    │
│  │ get_project_registries  - List...    │    │
│  │ list_items_in_registries - List...   │    │
│  └──────────────────────────────────────┘    │
├──────────────────────────────────────────────┤
│  Server Logs (expandable)                    │
└──────────────────────────────────────────────┘
```

### 5.5 SettingsPage

Application configuration with 6 tabs: General, Providers, Appearance, Runtime, Advanced, About.

**Layout:**
```
┌──────────────────────────────────────────────────────────────┐
│  TopBar: "Settings"                                          │
├──────────────────────────────────────────────────────────────┤
│  Tabs: [General] [Providers] [Appearance] [Runtime]          │
│        [Advanced] [About]                                    │
├──────────────────────────────────────────────────────────────┤
│  General:                                                    │
│  - Language (en / zh-CN)                                     │
│  - Theme (dark / light / system)                             │
│  - Send on Enter toggle                                      │
│  - Show agent thinking toggle                                │
│                                                              │
│  Providers:                                                  │
│  - Provider list with add/edit/delete                        │
│  - Provider form: name, base URL, API key,                   │
│    API format (dropdown)                                     │
│  - API key input (masked, saved to backend)                  │
│  - Test connection button                                    │
│  - Model list (fetched from /v1/models)                      │
│  - Model enable/disable checkboxes                           │
│  - Refresh models button                                     │
│  - Reasoning effort selector (shown only                     │
│    for OpenAI Responses API providers)                       │
│                                                              │
│  Appearance:                                                 │
│  - Theme mode (dark / light / system) with icon buttons      │
│  - Font size slider (12-18px)                                │
│  - Accent color selector (color swatches)                    │
│                                                              │
│  Runtime:                                                    │
│  - VM Status (running/starting/stopped/error indicator)      │
│  - Docker Connection status + backend source label           │
│  - Runtime Control: Start / Stop / Restart buttons           │
│    (calls runtime_start / runtime_stop Tauri commands)        │
│  - Allow External Docker toggle                              │
│    (Colima / Docker Desktop fallback, restart required)      │
│  - Runtime HTTP Proxy settings + bridge options              │
│    (persisted via settings_get/settings_update)              │
│  - CPU Cores slider (1-16)                                   │
│  - Memory Allocation slider (2-32 GB)                        │
│                                                              │
│  Advanced:                                                   │
│  - Container default TTL                                     │
│  - Max conversation history                                  │
│  - Confirm destructive operations                            │
│                                                              │
│  About:                                                      │
│  - CrateBay logo and branding                                │
│  - Version number                                            │
│  - Built with info (Tauri v2 + React + TypeScript)           │
│  - License (MIT)                                             │
│  - Links to GitHub and website                               │
└──────────────────────────────────────────────────────────────┘
```

---

## 6. Routing Strategy

CrateBay uses **Zustand-based routing** — no React Router. This is a desktop app with a fixed set of pages, and Zustand provides simpler state management for navigation.

### Implementation

```typescript
// In appStore.ts
interface AppState {
  currentPage: "chat" | "containers" | "images" | "mcp" | "settings";
  setCurrentPage: (page: AppState["currentPage"]) => void;
}

// In App.tsx
function App() {
  const currentPage = useAppStore((s) => s.currentPage);

  return (
    <AppLayout>
      {currentPage === "chat" && <ChatPage />}
      {currentPage === "containers" && <ContainersPage />}
      {currentPage === "images" && <ImagesPage />}
      {currentPage === "mcp" && <McpPage />}
      {currentPage === "settings" && <SettingsPage />}
    </AppLayout>
  );
}
```

### Rationale

- **5 pages** — URL-based routing adds unnecessary complexity for a desktop app
- **No deep linking needed** — this is not a web app
- **State preservation** — pages maintain state when switching (Zustand persists)
- **Faster transitions** — no route matching overhead

### Navigation

Sidebar items trigger `setCurrentPage()`. Active page is highlighted via Zustand state:

```typescript
function SidebarItem({ page, icon, label }: SidebarItemProps) {
  const { currentPage, setCurrentPage } = useAppStore();
  return (
    <button
      onClick={() => setCurrentPage(page)}
      className={cn(
        "flex items-center gap-2 px-3 py-2 rounded-md text-sm",
        currentPage === page
          ? "bg-primary/10 text-primary"
          : "text-muted-foreground hover:bg-muted"
      )}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}
```

---

## 7. Chat UI Components

### 7.1 ChatInput

A rich input component with @mention autocomplete for referencing tools, containers, and MCP servers.

```typescript
interface ChatInputProps {
  onSend: (message: string) => void;
  disabled?: boolean;
  placeholder?: string;
}
```

**Features:**
- **@mention autocomplete**: Typing `@` triggers a popup listing available tools, containers, and MCP servers
- **Multi-line input**: Shift+Enter for new line, Enter to send (configurable)
- **Keyboard shortcuts**: Ctrl+Enter always sends, Escape clears
- **History navigation**: Up/Down arrows navigate through sent message history
- **Auto-resize**: Textarea grows with content up to max height

**Mention Categories:**
| Prefix | Category | Examples |
|--------|----------|---------|
| `@tool:` | Agent tools | `@tool:container_create`, `@tool:shell_exec` |
| `@container:` | Running containers | `@container:node-01`, `@container:py-dev` |
| `@mcp:` | MCP servers/tools | `@mcp:shadcn`, `@mcp:cratebay` |

### 7.2 MessageList

Scrollable list of chat messages with auto-scroll and Streamdown rendering.

```typescript
interface MessageListProps {
  messages: ChatMessage[];
  isStreaming: boolean;
}
```

**Features:**
- **Auto-scroll**: Scrolls to bottom on new messages (unless user has scrolled up)
- **Streamdown rendering**: Markdown rendered via Streamdown for streaming-friendly display
- **Virtualized rendering**: Large conversation histories use virtual scrolling
- **Message grouping**: Consecutive messages from the same role are visually grouped

### 7.3 AgentThinking

Displays the agent's reasoning chain in a collapsible panel.

```typescript
interface AgentThinkingProps {
  content: string;
  isActive: boolean; // true while agent is still thinking
}
```

**Visual Design:**
- Muted background with subtle left border accent
- Animated ellipsis while active
- Collapsible — default open during thinking, collapsed after completion
- Monospace font for reasoning text

### 7.4 ToolCallCard

Inline card showing tool execution status within the message flow.

```typescript
interface ToolCallCardProps {
  toolCall: ToolCallInfo;
}
```

**States:**
| Status | Visual |
|--------|--------|
| `pending` | Muted card, "Preparing..." |
| `running` | Animated border, spinner, "Executing..." |
| `success` | Success border, check icon, result preview |
| `error` | Destructive border, error message |

**Features:**
- Expandable result/error details
- Parameter display (collapsible)
- Duration display on completion
- Re-run button for failed tools

### 7.5 ConfirmDialog

Modal dialog for destructive operations requiring user approval.

```typescript
interface ConfirmDialogProps {
  request: ConfirmationRequest;
  onConfirm: () => void;
  onCancel: () => void;
}
```

**Risk Level Styling:**
| Level | Color | Requires |
|-------|-------|----------|
| `medium` | Warning (amber) | Single click confirm |
| `high` | Destructive (red) | Type confirmation text |
| `critical` | Destructive + bold | Type exact resource name |

**Content:**
- Tool name and description
- Parameter summary
- Consequences list (bullet points)
- Confirm/Cancel buttons

---

## 8. Responsive Layout

CrateBay is a desktop application with responsive layout for different window sizes.

### Breakpoints

| Breakpoint | Width | Layout |
|-----------|-------|--------|
| Large | ≥1400px | Full layout: sidebar (280px) + content + optional detail panel |
| Medium | 1100–1399px | Compact layout: narrow sidebar (60px icons) + content |
| Small | <1100px | Collapsed: overlay sidebar + full-width content |

### Implementation

```typescript
// useBreakpoint.ts
function useBreakpoint() {
  const [width, setWidth] = useState(window.innerWidth);

  useEffect(() => {
    const handle = () => setWidth(window.innerWidth);
    window.addEventListener("resize", handle);
    return () => window.removeEventListener("resize", handle);
  }, []);

  return {
    isLarge: width >= 1400,
    isMedium: width >= 1100 && width < 1400,
    isSmall: width < 1100,
  };
}
```

### Sidebar Behavior

- **Large**: Full sidebar with labels and session list
- **Medium**: Icon-only sidebar with tooltips
- **Small**: Hidden sidebar, toggle via hamburger button (overlay)
- Sidebar width is resizable (drag handle) in large mode, stored in `appStore`

---

## 9. i18n Strategy

CrateBay uses a **typesafe, compile-time checked** i18n system. No runtime key lookup failures.

### Architecture

```typescript
// types/i18n.ts
interface Translations {
  common: {
    confirm: string;
    cancel: string;
    save: string;
    delete: string;
    loading: string;
    error: string;
  };
  chat: {
    newSession: string;
    placeholder: string;
    sendButton: string;
    thinking: string;
    toolExecuting: string;
  };
  containers: {
    title: string;
    create: string;
    start: string;
    stop: string;
    delete: string;
    noContainers: string;
  };
  mcp: {
    title: string;
    addServer: string;
    connected: string;
    disconnected: string;
  };
  settings: {
    title: string;
    general: string;
    providers: string;
    advanced: string;
    language: string;
    theme: string;
  };
}
```

### Implementation

```typescript
// lib/i18n.ts
import en from "@/locales/en";
import zhCN from "@/locales/zh-CN";

const locales: Record<string, Translations> = {
  en,
  "zh-CN": zhCN,
};

function createI18n(locale: string) {
  const translations = locales[locale] ?? locales.en;

  function t<K extends keyof Translations>(
    namespace: K
  ): Translations[K];
  function t<
    K extends keyof Translations,
    S extends keyof Translations[K]
  >(namespace: K, key: S): string;
  function t(namespace: string, key?: string) {
    const ns = (translations as Record<string, Record<string, string>>)[namespace];
    if (!ns) return key ?? namespace;
    if (key) return ns[key] ?? key;
    return ns;
  }

  return { t, locale };
}

// Usage in components
const { t } = useI18n();
t("chat", "placeholder"); // TypeScript validates both namespace and key
```

### Locale Files

```
src/locales/
├── en.ts       # English (complete)
└── zh-CN.ts    # Simplified Chinese (complete)
```

Each locale file must satisfy the `Translations` interface — TypeScript will error if any key is missing.

---

## 10. Tauri Integration

### 10.1 Invoke (Frontend → Backend)

All Tauri command calls go through `tauri-specta` generated typed bindings:

```typescript
// Auto-generated by tauri-specta (lib/tauri.ts)
import { invoke } from "@tauri-apps/api/core";

export const commands = {
  containerList: () => invoke<ContainerInfo[]>("container_list"),
  containerCreate: (req: ContainerCreateRequest) =>
    invoke<ContainerInfo>("container_create", { req }),
  containerExec: (id: string, command: string) =>
    invoke<string>("container_exec", { id, command }),
  llmProxyStream: (providerId: string, messages: LlmMessage[]) =>
    invoke<void>("llm_proxy_stream", { providerId, messages }),
  // ... all commands auto-generated
};
```

### 10.2 Listen (Backend → Frontend Events)

For streaming data (LLM tokens, container logs), the Rust backend emits Tauri events:

```typescript
import { listen } from "@tauri-apps/api/event";

// Event channel naming convention: {domain}:{action}:{id?}
// Examples:
//   llm:token:session-123
//   container:log:abc123
//   container:status-change:abc123

// Hook for subscribing to Tauri events
function useTauriEvent<T>(
  eventName: string,
  handler: (payload: T) => void
) {
  useEffect(() => {
    const unlisten = listen<T>(eventName, (event) => {
      handler(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [eventName, handler]);
}
```

### 10.3 Event Payload Types

```typescript
// LLM streaming token
interface LlmTokenEvent {
  sessionId: string;
  token: string;
  done: boolean;
}

// Container status change
interface ContainerStatusEvent {
  containerId: string;
  status: "running" | "stopped" | "error";
  message?: string;
}

// Container log line
interface ContainerLogEvent {
  containerId: string;
  line: string;
  stream: "stdout" | "stderr";
  timestamp: string;
}
```

### 10.4 Error Handling

All Tauri invoke errors are caught and mapped to user-friendly messages:

```typescript
import { TauriError } from "@/types";

async function safeTauriInvoke<T>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    const tauriError = err as TauriError;
    // Map backend error codes to i18n keys
    throw new AppError(tauriError.code, tauriError.message);
  }
}
```

---

## 11. Dependency Budget

Every new frontend dependency must justify its inclusion. The budget is reviewed during code review.

### Current Budget

| Dependency | Gzipped Size | Justification |
|-----------|-------------|---------------|
| react + react-dom | ~45KB | Core framework |
| zustand | ~2KB | State management |
| @tauri-apps/api | ~8KB | Tauri bridge |
| tailwindcss | Build-time | CSS utility framework |
| streamdown | ~15KB | Streaming markdown (Vercel, native shadcn) |
| @mariozechner/pi-agent-core | ~25KB | Agent orchestration |
| @mariozechner/pi-ai | ~10KB | LLM API |
| @sinclair/typebox | ~12KB | JSON Schema validation (for tool params) |
| lucide-react | Tree-shaken | Icons (only imported icons are bundled) |

**Total estimated**: ~120KB gzipped (excluding tree-shaken portions)

### Rules

1. **No dependency >50KB gzipped** without team-lead approval
2. **Prefer built-in browser APIs** over polyfill libraries
3. **Tree-shaking required**: Libraries must support ESM tree-shaking
4. **No UI framework overlap**: shadcn/ui + Radix only, no Material UI/Ant Design/etc.
5. **Bundle analysis**: Run `npx vite-bundle-visualizer` before adding dependencies

---

## 12. Code Style

### ESLint Configuration

```jsonc
// .eslintrc.json
{
  "extends": [
    "eslint:recommended",
    "plugin:@typescript-eslint/strict-type-checked",
    "plugin:react/recommended",
    "plugin:react-hooks/recommended"
  ],
  "rules": {
    "@typescript-eslint/no-explicit-any": "error",
    "@typescript-eslint/no-unused-vars": ["error", { "argsIgnorePattern": "^_" }],
    "@typescript-eslint/strict-boolean-expressions": "error",
    "react/react-in-jsx-scope": "off",
    "react/prop-types": "off",
    "no-console": ["warn", { "allow": ["warn", "error"] }]
  }
}
```

### Prettier Configuration

```jsonc
// .prettierrc
{
  "semi": true,
  "singleQuote": false,
  "tabWidth": 2,
  "trailingComma": "all",
  "printWidth": 100,
  "bracketSpacing": true,
  "arrowParens": "always",
  "endOfLine": "lf"
}
```

### Naming Conventions

| Item | Convention | Example |
|------|-----------|---------|
| Components | PascalCase | `ChatInput`, `MessageList` |
| Hooks | camelCase with `use` prefix | `useTauriEvent`, `useAgent` |
| Stores | camelCase with `Store` suffix | `chatStore`, `appStore` |
| Types/Interfaces | PascalCase | `ChatMessage`, `ContainerInfo` |
| Constants | UPPER_SNAKE_CASE | `MAX_MESSAGE_LENGTH`, `DEFAULT_TTL` |
| Files: components | PascalCase.tsx | `ChatInput.tsx`, `MessageList.tsx` |
| Files: hooks | camelCase.ts | `useTauriEvent.ts` |
| Files: stores | camelCase.ts | `chatStore.ts` |
| Files: utils/lib | camelCase.ts | `utils.ts`, `constants.ts` |
| CSS classes | Tailwind utilities | `flex items-center gap-2` |
| Event names | kebab-case with colon separators | `llm:token:session-123` |

### TypeScript Rules

- **Strict mode enabled** (`"strict": true` in `tsconfig.json`)
- **No `any`** — use `unknown` and narrow with type guards
- **No type assertions** (`as Type`) unless absolutely necessary (with comment explaining why)
- **Prefer interfaces** for object shapes, `type` for unions/intersections
- **Export types** alongside their implementations
- **Use `satisfies`** for type validation without widening:
  ```typescript
  const config = {
    theme: "dark",
    language: "en",
  } satisfies Partial<AppSettings>;
  ```
