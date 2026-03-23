import type { Page } from "@playwright/test";

export interface MockTauriData {
  settings: Record<string, string>;
  conversationList: Array<{
    id: string;
    title: string;
    message_count: number;
    created_at: string;
    updated_at: string;
    last_message_preview: string | null;
  }>;
  conversationMessages: Record<
    string,
    {
      id: string;
      title: string;
      created_at: string;
      updated_at: string;
      messages: Array<{
        role: string;
        content: string;
        tool_calls?: { id: string; name: string; arguments: string }[];
        tool_call_id?: string;
      }>;
    }
  >;
  containerList: Array<Record<string, unknown>>;
  containerTemplates: Array<Record<string, unknown>>;
  mcpServers: Array<Record<string, unknown>>;
  mcpTools: Array<Record<string, unknown>>;
  dockerStatus: Record<string, unknown>;
  runtimeStatus: Record<string, unknown>;
  llmTokens: string[];
}

const DEFAULT_DATE = new Date("2026-03-23T00:00:00.000Z").toISOString();

const defaultMockData: MockTauriData = {
  settings: {
    language: "en",
    theme: "dark",
    sendOnEnter: "true",
    showAgentThinking: "true",
    maxConversationHistory: "50",
    containerDefaultTtlHours: "8",
    confirmDestructiveOps: "true",
    reasoningEffort: "medium",
    registryMirrors: "[]",
  },
  conversationList: [
    {
      id: "session-1",
      title: "Welcome",
      message_count: 1,
      created_at: DEFAULT_DATE,
      updated_at: DEFAULT_DATE,
      last_message_preview: "Hello from CrateBay",
    },
  ],
  conversationMessages: {
    "session-1": {
      id: "session-1",
      title: "Welcome",
      created_at: DEFAULT_DATE,
      updated_at: DEFAULT_DATE,
      messages: [{ role: "assistant", content: "Hello from CrateBay" }],
    },
  },
  containerList: [
    {
      id: "abc123",
      shortId: "abc123",
      name: "node-01",
      status: "running",
      state: "running",
      image: "node:latest",
      templateId: "node-dev",
      cpuCores: 2,
      memoryMb: 2048,
      ports: [{ hostPort: 3000, containerPort: 3000, protocol: "tcp" }],
      createdAt: DEFAULT_DATE,
      labels: {},
    },
    {
      id: "def456",
      shortId: "def456",
      name: "python-dev",
      status: "stopped",
      state: "stopped",
      image: "python:latest",
      templateId: "python-dev",
      cpuCores: 1,
      memoryMb: 1024,
      ports: [],
      createdAt: DEFAULT_DATE,
      labels: {},
    },
  ],
  containerTemplates: [
    { id: "node-dev", name: "Node.js", description: "Node.js development", image: "node:latest" },
    { id: "python-dev", name: "Python", description: "Python development", image: "python:latest" },
    { id: "rust-dev", name: "Rust", description: "Rust development", image: "rust:latest" },
  ],
  mcpServers: [
    {
      id: "shadcn-1",
      name: "shadcn",
      command: "shadcn",
      args: [],
      env: {},
      enabled: true,
      status: "connected",
      transport: "stdio",
      toolCount: 7,
    },
    {
      id: "cratebay-1",
      name: "cratebay-mcp",
      command: "cratebay-mcp",
      args: ["--workspace", "/workspace"],
      env: { CRATEBAY_MCP_WORKSPACE_ROOT: "/workspace" },
      enabled: false,
      status: "disconnected",
      transport: "stdio",
      toolCount: 12,
    },
  ],
  mcpTools: [
    {
      serverId: "shadcn-1",
      serverName: "shadcn",
      name: "get_project_registries",
      description: "Get configured registry names from components.json",
      inputSchema: { type: "object", properties: {} },
    },
    {
      serverId: "shadcn-1",
      serverName: "shadcn",
      name: "list_items_in_registries",
      description: "List items from registries",
      inputSchema: { type: "object", properties: {} },
    },
  ],
  dockerStatus: {
    connected: true,
    version: "25.0.0",
    api_version: "1.44",
    os: "linux",
    arch: "arm64",
    source: "runtime",
    socket_path: "/tmp/docker.sock",
  },
  runtimeStatus: {
    state: "ready",
    platform: "macos-vz",
    cpu_cores: 2,
    memory_mb: 2048,
    disk_gb: 20,
    docker_responsive: true,
    uptime_seconds: 120,
    resource_usage: null,
  },
  llmTokens: ["Here", " ", "are", " ", "your", " ", "containers", "."],
};

export async function installTauriMock(
  page: Page,
  overrides: Partial<MockTauriData> = {},
): Promise<void> {
  const merged: MockTauriData = {
    ...defaultMockData,
    ...overrides,
    settings: { ...defaultMockData.settings, ...(overrides.settings ?? {}) },
    conversationMessages: {
      ...defaultMockData.conversationMessages,
      ...(overrides.conversationMessages ?? {}),
    },
  };

  await page.addInitScript(({ mockData }) => {
    (window as any).__MOCK_TAURI__ = mockData;

    const ensureSession = (id: string) => {
      const state = (window as any).__MOCK_TAURI__;
      if (!state.conversationMessages[id]) {
        state.conversationMessages[id] = {
          id,
          title: "New Chat",
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
          messages: [],
        };
      }
      return state.conversationMessages[id];
    };

    const emit = (eventName: string, payload: unknown) => {
      const event = new CustomEvent(eventName, {
        detail: { payload },
      });
      window.dispatchEvent(event);
    };

    (window as any).__MOCK_TAURI_INVOKE__ = async (
      command: string,
      args?: Record<string, unknown>
    ) => {
      const state = (window as any).__MOCK_TAURI__;

      switch (command) {
        case "settings_get": {
          const key = (args?.key as string) ?? "";
          return key in state.settings ? state.settings[key] : null;
        }
        case "settings_update": {
          const key = (args?.key as string) ?? "";
          const value = args?.value as string;
          state.settings[key] = String(value ?? "");
          return null;
        }
        case "conversation_list":
          return state.conversationList;
        case "conversation_create": {
          const newId = `session-${Date.now()}`;
          const title = (args?.title as string) ?? "New Chat";
          const detail = {
            id: newId,
            title,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
            messages: [],
          };
          state.conversationMessages[newId] = detail;
          state.conversationList.unshift({
            id: newId,
            title,
            message_count: 0,
            created_at: detail.created_at,
            updated_at: detail.updated_at,
            last_message_preview: null,
          });
          return detail;
        }
        case "conversation_get_messages": {
          const id = (args?.id as string) ?? "";
          return ensureSession(id);
        }
        case "conversation_save_message":
        case "conversation_update_title":
        case "conversation_delete":
          return null;
        case "container_list":
          return state.containerList;
        case "container_templates":
          return state.containerTemplates;
        case "container_start":
        case "container_stop":
        case "container_delete":
          return null;
        case "image_list":
          return [];
        case "image_pull":
        case "image_remove":
          return null;
        case "docker_status":
          return state.dockerStatus;
        case "runtime_status":
          return state.runtimeStatus;
        case "runtime_start":
        case "runtime_stop":
          return "ok";
        case "mcp_server_list":
          return state.mcpServers;
        case "mcp_client_list_tools":
          return state.mcpTools;
        case "mcp_server_start":
        case "mcp_server_stop":
        case "mcp_server_add":
        case "mcp_server_remove":
          return null;
        case "mcp_client_call_tool":
          return { ok: true };
        case "llm_provider_list":
          return [];
        case "llm_models_list":
        case "llm_models_fetch":
          return [];
        case "llm_provider_test":
          return { success: true, latencyMs: 0, model: "mock", error: null };
        case "llm_proxy_stream": {
          const channelId = (args?.channel_id as string) ?? "default";
          const eventName = `llm:stream:${channelId}`;
          const tokens: string[] = state.llmTokens ?? [];
          let delay = 0;
          for (const token of tokens) {
            delay += 30;
            setTimeout(() => {
              emit(eventName, { type: "Token", content: token });
            }, delay);
          }
          delay += 30;
          setTimeout(() => {
            emit(eventName, {
              type: "Done",
              usage: {
                prompt_tokens: 5,
                completion_tokens: tokens.length,
                total_tokens: 5 + tokens.length,
              },
            });
          }, delay);
          return null;
        }
        case "llm_proxy_cancel":
          return null;
        default:
          return null;
      }
    };
  }, { mockData: merged });
}
