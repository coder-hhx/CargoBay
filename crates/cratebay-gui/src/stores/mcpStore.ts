import { create } from "zustand";
import { invoke } from "@/lib/tauri";
import type { McpServerInfo, McpServerConfig, McpToolInfo } from "@/types/mcp";

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
  appendServerLog: (serverId: string, line: string) => void;
}

let mockServerIdCounter = 0;

export const useMcpStore = create<McpState>()((set, get) => ({
  servers: [],
  loading: false,

  fetchServers: async () => {
    set({ loading: true });
    try {
      const servers = await invoke<McpServerInfo[]>("mcp_server_list");
      set({ servers, loading: false });
      // Refresh available tools whenever server list is refreshed
      void get().fetchTools();
    } catch {
      // Mock for non-Tauri development
      set({ loading: false });
    }
  },

  startServer: async (id) => {
    // Optimistic: mark as starting
    set((state) => ({
      servers: state.servers.map((s) =>
        s.id === id ? { ...s, status: "starting" as const } : s,
      ),
    }));
    try {
      await invoke("mcp_server_start", { id });
      // Refresh full server list and tools after start
      await get().fetchServers();
    } catch (err) {
      set((state) => ({
        servers: state.servers.map((s) =>
          s.id === id ? { ...s, status: "error" as const, error: String(err) } : s,
        ),
      }));
    }
  },

  stopServer: async (id) => {
    try {
      await invoke("mcp_server_stop", { id });
    } catch {
      // Mock
    }
    // Refresh full server list and tools after stop
    await get().fetchServers();
  },

  addServer: async (config) => {
    try {
      await invoke("mcp_server_add", { config });
      // Refresh server list
      const servers = await invoke<McpServerInfo[]>("mcp_server_list");
      set({ servers });
      // Refresh tools after adding a server
      void get().fetchTools();
    } catch {
      // Mock for non-Tauri development
      const newServer: McpServerInfo = {
        id: `mcp-${++mockServerIdCounter}-${Date.now()}`,
        name: config.name,
        command: config.command,
        args: config.args,
        env: config.env ?? {},
        enabled: config.enabled ?? true,
        status: "disconnected",
        transport: config.transport ?? "stdio",
        toolCount: 0,
      };
      set((state) => ({ servers: [...state.servers, newServer] }));
    }
  },

  removeServer: async (id) => {
    try {
      await invoke("mcp_server_remove", { id });
    } catch {
      // Mock
    }
    set((state) => ({
      servers: state.servers.filter((s) => s.id !== id),
      serverLogs: Object.fromEntries(
        Object.entries(state.serverLogs).filter(([k]) => k !== id),
      ),
    }));
  },

  updateServer: async (id, config) => {
    try {
      await invoke("mcp_update_server", { id, config });
      const servers = await invoke<McpServerInfo[]>("mcp_server_list");
      set({ servers });
    } catch {
      // Mock for non-Tauri development
      set((state) => ({
        servers: state.servers.map((s) =>
          s.id === id
            ? {
                ...s,
                ...(config.name !== undefined ? { name: config.name } : {}),
                ...(config.command !== undefined ? { command: config.command } : {}),
                ...(config.args !== undefined ? { args: config.args } : {}),
                ...(config.env !== undefined ? { env: config.env } : {}),
                ...(config.enabled !== undefined ? { enabled: config.enabled } : {}),
                ...(config.transport !== undefined ? { transport: config.transport } : {}),
              }
            : s,
        ),
      }));
    }
  },

  availableTools: [],

  fetchTools: async () => {
    try {
      const rawTools = await invoke<McpToolInfo[]>("mcp_client_list_tools");
      // Enrich tools with serverName from the servers list.
      // The Rust McpToolInfo may not include serverName, so we resolve it
      // from the local servers state.
      const { servers } = get();
      const serverNameMap = new Map(servers.map((s) => [s.id, s.name]));
      const tools = rawTools.map((t) => ({
        ...t,
        serverName: t.serverName || serverNameMap.get(t.serverId) || t.serverId,
      }));
      set({ availableTools: tools });
    } catch {
      // Mock for non-Tauri development
    }
  },

  callTool: async (serverId, toolName, args) => {
    try {
      return await invoke<unknown>("mcp_client_call_tool", {
        serverId,
        toolName,
        arguments: args,
      });
    } catch (err) {
      throw new Error(`MCP tool call failed: ${String(err)}`);
    }
  },

  serverLogs: {},

  fetchServerLogs: async (id) => {
    try {
      const logs = await invoke<string[]>("mcp_server_logs", { id });
      set((state) => ({
        serverLogs: { ...state.serverLogs, [id]: logs },
      }));
    } catch {
      // Mock for non-Tauri development
    }
  },

  appendServerLog: (serverId, line) =>
    set((state) => ({
      serverLogs: {
        ...state.serverLogs,
        [serverId]: [...(state.serverLogs[serverId] ?? []), line],
      },
    })),
}));
