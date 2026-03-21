import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";

// Mock Tauri before importing stores
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import { useMcpStore } from "@/stores/mcpStore";
import type { McpServerInfo, McpToolInfo } from "@/types/mcp";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const makeServer = (overrides: Partial<McpServerInfo> = {}): McpServerInfo => ({
  id: "mcp-1",
  name: "Test MCP",
  command: "/usr/local/bin/mcp-server",
  args: ["--stdio"],
  env: {},
  enabled: true,
  status: "connected",
  transport: "stdio",
  toolCount: 3,
  ...overrides,
});

const makeTool = (overrides: Partial<McpToolInfo> = {}): McpToolInfo => ({
  serverId: "mcp-1",
  serverName: "Test MCP",
  name: "test_tool",
  description: "A test tool",
  inputSchema: { type: "object", properties: {} },
  ...overrides,
});

function resetStore() {
  useMcpStore.setState({
    servers: [],
    loading: false,
    availableTools: [],
    serverLogs: {},
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
describe("mcpStore", () => {
  beforeEach(() => {
    resetStore();
    resetTauriMocks();
  });

  // -------------------------------------------------------------------------
  // fetchServers
  // -------------------------------------------------------------------------
  describe("fetchServers", () => {
    it("populates servers from invoke and triggers fetchTools", async () => {
      const servers = [makeServer(), makeServer({ id: "mcp-2", name: "Another" })];
      // First call: mcp_server_list, second call: mcp_client_list_tools
      mockInvoke
        .mockResolvedValueOnce(servers)
        .mockResolvedValueOnce([]);

      await useMcpStore.getState().fetchServers();

      expect(mockInvoke).toHaveBeenCalledWith("mcp_server_list");
      expect(useMcpStore.getState().servers).toEqual(servers);
      expect(useMcpStore.getState().loading).toBe(false);
    });

    it("sets loading=false on failure (non-Tauri mode)", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("no Tauri"));

      await useMcpStore.getState().fetchServers();

      expect(useMcpStore.getState().loading).toBe(false);
    });
  });

  // -------------------------------------------------------------------------
  // fetchTools
  // -------------------------------------------------------------------------
  describe("fetchTools", () => {
    it("populates availableTools from invoke", async () => {
      const tools = [
        makeTool({ name: "tool_a" }),
        makeTool({ name: "tool_b", serverId: "mcp-2" }),
      ];
      useMcpStore.setState({
        servers: [
          makeServer({ id: "mcp-1", name: "Server One" }),
          makeServer({ id: "mcp-2", name: "Server Two" }),
        ],
      });
      mockInvoke.mockResolvedValueOnce(tools);

      await useMcpStore.getState().fetchTools();

      const available = useMcpStore.getState().availableTools;
      expect(available).toHaveLength(2);
    });

    it("enriches tools with serverName from servers list", async () => {
      const tools = [
        makeTool({ name: "tool_a", serverId: "mcp-1", serverName: "" }),
      ];
      useMcpStore.setState({
        servers: [makeServer({ id: "mcp-1", name: "My Server" })],
      });
      mockInvoke.mockResolvedValueOnce(tools);

      await useMcpStore.getState().fetchTools();

      expect(useMcpStore.getState().availableTools[0].serverName).toBe("My Server");
    });

    it("falls back gracefully when invoke fails", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("no Tauri"));

      await useMcpStore.getState().fetchTools();

      expect(useMcpStore.getState().availableTools).toEqual([]);
    });
  });

  // -------------------------------------------------------------------------
  // startServer
  // -------------------------------------------------------------------------
  describe("startServer", () => {
    it("optimistically marks server as starting, then refreshes", async () => {
      const server = makeServer({ id: "mcp-1", status: "disconnected" });
      useMcpStore.setState({ servers: [server] });

      // mcp_server_start → void, then mcp_server_list, then mcp_client_list_tools
      mockInvoke
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce([makeServer({ id: "mcp-1", status: "connected" })])
        .mockResolvedValueOnce([]);

      await useMcpStore.getState().startServer("mcp-1");

      expect(mockInvoke).toHaveBeenCalledWith("mcp_server_start", { id: "mcp-1" });
    });

    it("sets error status when start fails", async () => {
      const server = makeServer({ id: "mcp-1", status: "disconnected" });
      useMcpStore.setState({ servers: [server] });

      mockInvoke.mockRejectedValueOnce(new Error("Connection refused"));

      await useMcpStore.getState().startServer("mcp-1");

      const updatedServer = useMcpStore.getState().servers.find((s) => s.id === "mcp-1");
      expect(updatedServer?.status).toBe("error");
      expect(updatedServer?.error).toContain("Connection refused");
    });
  });

  // -------------------------------------------------------------------------
  // stopServer
  // -------------------------------------------------------------------------
  describe("stopServer", () => {
    it("calls mcp_server_stop and refreshes server list", async () => {
      const server = makeServer({ id: "mcp-1", status: "connected" });
      useMcpStore.setState({ servers: [server] });

      // mcp_server_stop, then mcp_server_list (via fetchServers), then mcp_client_list_tools
      mockInvoke
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce([makeServer({ id: "mcp-1", status: "disconnected" })])
        .mockResolvedValueOnce([]);

      await useMcpStore.getState().stopServer("mcp-1");

      expect(mockInvoke).toHaveBeenCalledWith("mcp_server_stop", { id: "mcp-1" });
    });
  });

  // -------------------------------------------------------------------------
  // addServer
  // -------------------------------------------------------------------------
  describe("addServer", () => {
    it("invokes mcp_server_add and refreshes server list", async () => {
      const config = {
        name: "New Server",
        command: "/usr/bin/mcp",
        args: ["--port", "3000"],
      };
      const updatedServers = [makeServer({ id: "mcp-new", name: "New Server" })];

      // mcp_server_add, then mcp_server_list, then mcp_client_list_tools
      mockInvoke
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce(updatedServers)
        .mockResolvedValueOnce([]);

      await useMcpStore.getState().addServer(config);

      expect(mockInvoke).toHaveBeenCalledWith("mcp_server_add", { config });
      expect(useMcpStore.getState().servers).toEqual(updatedServers);
    });

    it("creates mock server when invoke fails", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("no Tauri"));

      await useMcpStore.getState().addServer({
        name: "Mock Server",
        command: "/bin/mock",
        args: [],
      });

      expect(useMcpStore.getState().servers).toHaveLength(1);
      expect(useMcpStore.getState().servers[0].name).toBe("Mock Server");
      expect(useMcpStore.getState().servers[0].status).toBe("disconnected");
    });
  });

  // -------------------------------------------------------------------------
  // removeServer
  // -------------------------------------------------------------------------
  describe("removeServer", () => {
    it("removes server from list", async () => {
      const s1 = makeServer({ id: "mcp-1" });
      const s2 = makeServer({ id: "mcp-2", name: "Other" });
      useMcpStore.setState({ servers: [s1, s2] });
      mockInvoke.mockResolvedValueOnce(undefined);

      await useMcpStore.getState().removeServer("mcp-1");

      expect(useMcpStore.getState().servers).toHaveLength(1);
      expect(useMcpStore.getState().servers[0].id).toBe("mcp-2");
    });

    it("cleans up server logs when removing a server", async () => {
      useMcpStore.setState({
        servers: [makeServer({ id: "mcp-1" })],
        serverLogs: { "mcp-1": ["log line 1"], "mcp-2": ["log line 2"] },
      });
      mockInvoke.mockResolvedValueOnce(undefined);

      await useMcpStore.getState().removeServer("mcp-1");

      expect(useMcpStore.getState().serverLogs).not.toHaveProperty("mcp-1");
      expect(useMcpStore.getState().serverLogs["mcp-2"]).toEqual(["log line 2"]);
    });
  });

  // -------------------------------------------------------------------------
  // callTool
  // -------------------------------------------------------------------------
  describe("callTool", () => {
    it("invokes mcp_client_call_tool with correct arguments", async () => {
      const result = { output: "tool result" };
      mockInvoke.mockResolvedValueOnce(result);

      const response = await useMcpStore.getState().callTool("mcp-1", "my_tool", { key: "val" });

      expect(mockInvoke).toHaveBeenCalledWith("mcp_client_call_tool", {
        serverId: "mcp-1",
        toolName: "my_tool",
        arguments: { key: "val" },
      });
      expect(response).toEqual(result);
    });

    it("throws with descriptive error on failure", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("server down"));

      await expect(
        useMcpStore.getState().callTool("mcp-1", "bad_tool", {}),
      ).rejects.toThrow("MCP tool call failed");
    });
  });

  // -------------------------------------------------------------------------
  // serverLogs
  // -------------------------------------------------------------------------
  describe("serverLogs", () => {
    it("fetchServerLogs is a no-op (backend does not support mcp_server_logs)", async () => {
      await useMcpStore.getState().fetchServerLogs("mcp-1");

      // Should not call invoke since the command is not in api-spec
      expect(mockInvoke).not.toHaveBeenCalled();
      // Logs state should remain empty
      expect(useMcpStore.getState().serverLogs["mcp-1"]).toBeUndefined();
    });

    it("appendServerLog adds a log line", () => {
      useMcpStore.setState({ serverLogs: { "mcp-1": ["line 1"] } });

      useMcpStore.getState().appendServerLog("mcp-1", "line 2");

      expect(useMcpStore.getState().serverLogs["mcp-1"]).toEqual(["line 1", "line 2"]);
    });

    it("appendServerLog creates array for new serverId", () => {
      useMcpStore.getState().appendServerLog("mcp-new", "first line");

      expect(useMcpStore.getState().serverLogs["mcp-new"]).toEqual(["first line"]);
    });
  });

  // -------------------------------------------------------------------------
  // updateServer
  // -------------------------------------------------------------------------
  describe("updateServer", () => {
    it("uses mock fallback to update server in place (remove + add fails in non-Tauri)", async () => {
      useMcpStore.setState({
        servers: [makeServer({ id: "mcp-1", name: "Old Name" })],
      });
      // First invoke (mcp_server_remove) fails, so mock fallback runs
      mockInvoke.mockRejectedValueOnce(new Error("no Tauri"));

      await useMcpStore.getState().updateServer("mcp-1", { name: "New Name" });

      expect(useMcpStore.getState().servers[0].name).toBe("New Name");
    });
  });
});
