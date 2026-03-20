import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";

// Mock Tauri before importing tools
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import {
  containerListTool,
  containerCreateTool,
  containerStartTool,
  containerStopTool,
  containerDeleteTool,
  containerExecTool,
  containerLogsTool,
  containerInspectTool,
  containerTools,
} from "@/tools/containerTools";
import {
  builtinTools,
  toolRiskLevels,
  getToolRiskLevel,
  getToolLabel,
} from "@/tools";

// ---------------------------------------------------------------------------
// Tool execution tests (testing-spec §4.2 pattern)
// ---------------------------------------------------------------------------
describe("containerTools — tool execution", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  it("containerListTool calls container_list and formats output", async () => {
    mockInvoke.mockResolvedValueOnce([
      {
        short_id: "abc123",
        name: "node-01",
        image: "node:20-slim",
        state: "running",
        cpu_cores: 2,
        memory_mb: 2048,
      },
      {
        short_id: "def456",
        name: "py-dev",
        image: "python:3.12-slim",
        state: "stopped",
        cpu_cores: 1,
        memory_mb: 1024,
      },
    ]);

    const result = await containerListTool.execute("tc-1", {});

    expect(mockInvoke).toHaveBeenCalledWith("container_list", { filters: undefined });
    expect(result.content[0].type).toBe("text");
    if (result.content[0].type === "text") {
      expect(result.content[0].text).toContain("2 container(s)");
      expect(result.content[0].text).toContain("node-01");
      expect(result.content[0].text).toContain("py-dev");
    }
  });

  it("containerListTool returns 'No containers found' for empty list", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    const result = await containerListTool.execute("tc-1", {});

    if (result.content[0].type === "text") {
      expect(result.content[0].text).toBe("No containers found.");
    }
  });

  it("containerListTool passes status filter", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    await containerListTool.execute("tc-1", { status: "running" });

    expect(mockInvoke).toHaveBeenCalledWith("container_list", {
      filters: { status: ["Running"] },
    });
  });

  it("containerCreateTool calls container_create with request params", async () => {
    mockInvoke.mockResolvedValueOnce({
      short_id: "ghi789",
      name: "new-box",
      image: "node:20-slim",
      state: "running",
    });

    const result = await containerCreateTool.execute("tc-2", {
      name: "new-box",
      image: "node:20-slim",
      cpu_cores: 2,
      memory_mb: 2048,
    });

    expect(mockInvoke).toHaveBeenCalledWith("container_create", {
      request: expect.objectContaining({
        name: "new-box",
        image: "node:20-slim",
        cpu_cores: 2,
        memory_mb: 2048,
        auto_start: true,
      }),
    });

    if (result.content[0].type === "text") {
      expect(result.content[0].text).toContain("new-box");
      expect(result.content[0].text).toContain("created successfully");
    }
  });

  it("containerCreateTool throws on empty result", async () => {
    mockInvoke.mockResolvedValueOnce(null);

    await expect(
      containerCreateTool.execute("tc-2", { name: "bad", image: "test" }),
    ).rejects.toThrow("Container creation failed");
  });

  it("containerStartTool calls container_start", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    const result = await containerStartTool.execute("tc-3", { id: "abc123" });

    expect(mockInvoke).toHaveBeenCalledWith("container_start", { id: "abc123" });
    if (result.content[0].type === "text") {
      expect(result.content[0].text).toContain("abc123 started");
    }
  });

  it("containerStopTool calls container_stop with timeout", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await containerStopTool.execute("tc-4", { id: "abc123", timeout: 30 });

    expect(mockInvoke).toHaveBeenCalledWith("container_stop", {
      id: "abc123",
      timeout: 30,
    });
  });

  it("containerDeleteTool calls container_delete", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    const result = await containerDeleteTool.execute("tc-5", { id: "abc123" });

    expect(mockInvoke).toHaveBeenCalledWith("container_delete", {
      id: "abc123",
      force: false,
    });
    if (result.content[0].type === "text") {
      expect(result.content[0].text).toContain("abc123 deleted");
    }
  });

  it("containerExecTool calls container_exec and formats output", async () => {
    mockInvoke.mockResolvedValueOnce({
      exit_code: 0,
      stdout: "hello world\n",
      stderr: "",
    });

    const result = await containerExecTool.execute("tc-6", {
      id: "abc123",
      cmd: ["echo", "hello world"],
    });

    expect(mockInvoke).toHaveBeenCalledWith("container_exec", {
      id: "abc123",
      cmd: ["echo", "hello world"],
      working_dir: undefined,
    });

    if (result.content[0].type === "text") {
      expect(result.content[0].text).toContain("hello world");
      expect(result.content[0].text).toContain("exit code:** 0");
    }
  });

  it("containerLogsTool calls container_logs and formats output", async () => {
    mockInvoke.mockResolvedValueOnce([
      { stream: "stdout", message: "Server started\n", timestamp: null },
      { stream: "stderr", message: "Warning: deprecated\n", timestamp: null },
    ]);

    const result = await containerLogsTool.execute("tc-7", { id: "abc123" });

    expect(mockInvoke).toHaveBeenCalledWith("container_logs", {
      id: "abc123",
      options: { tail: 100, since: undefined },
    });

    if (result.content[0].type === "text") {
      expect(result.content[0].text).toContain("[stdout] Server started");
      expect(result.content[0].text).toContain("[stderr] Warning: deprecated");
    }
  });

  it("containerLogsTool returns 'No logs found' for empty result", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    const result = await containerLogsTool.execute("tc-7", { id: "abc123" });

    if (result.content[0].type === "text") {
      expect(result.content[0].text).toBe("No logs found.");
    }
  });

  it("containerInspectTool calls container_inspect and formats detailed output", async () => {
    mockInvoke.mockResolvedValueOnce({
      short_id: "abc123",
      name: "node-01",
      image: "node:20-slim",
      state: "running",
      created_at: "2026-03-20T10:00:00Z",
      started_at: "2026-03-20T10:01:00Z",
      cpu_cores: 2,
      memory_mb: 2048,
      env: ["NODE_ENV=production"],
      ports: [{ host_port: 3000, container_port: 3000, protocol: "tcp" }],
      mounts: [],
      network: { ip_address: "172.17.0.2", gateway: "172.17.0.1", network_name: "bridge" },
    });

    const result = await containerInspectTool.execute("tc-8", { containerId: "abc123" });

    if (result.content[0].type === "text") {
      const text = result.content[0].text;
      expect(text).toContain("node-01");
      expect(text).toContain("running");
      expect(text).toContain("2 cores");
      expect(text).toContain("2048 MB");
      expect(text).toContain("3000:3000/tcp");
      expect(text).toContain("172.17.0.2");
    }
  });
});

// ---------------------------------------------------------------------------
// Tool registry tests
// ---------------------------------------------------------------------------
describe("tool registry", () => {
  it("containerTools contains 8 tools", () => {
    expect(containerTools).toHaveLength(8);
  });

  it("builtinTools contains all registered tools from all categories", () => {
    // containerTools(8) + filesystemTools(3) + shellTools(1) + mcpTools(2) + systemTools(3)
    expect(builtinTools.length).toBeGreaterThanOrEqual(8);

    // Check container tools are all present
    const names = builtinTools.map((t) => t.name);
    expect(names).toContain("container_list");
    expect(names).toContain("container_create");
    expect(names).toContain("container_start");
    expect(names).toContain("container_stop");
    expect(names).toContain("container_delete");
    expect(names).toContain("container_exec");
    expect(names).toContain("container_logs");
    expect(names).toContain("container_inspect");
  });

  it("each tool has name, label, description, parameters, and execute", () => {
    for (const tool of builtinTools) {
      expect(tool.name).toBeDefined();
      expect(tool.label).toBeDefined();
      expect(tool.description).toBeDefined();
      expect(tool.parameters).toBeDefined();
      expect(typeof tool.execute).toBe("function");
    }
  });
});

// ---------------------------------------------------------------------------
// Risk level assessment tests
// ---------------------------------------------------------------------------
describe("getToolRiskLevel", () => {
  it("returns correct static risk levels", () => {
    expect(toolRiskLevels.container_list).toBe("low");
    expect(toolRiskLevels.container_create).toBe("medium");
    expect(toolRiskLevels.container_delete).toBe("high");
    expect(toolRiskLevels.container_exec).toBe("medium");
    expect(toolRiskLevels.file_read).toBe("low");
    expect(toolRiskLevels.file_write).toBe("medium");
    expect(toolRiskLevels.shell_exec).toBe("medium");
    expect(toolRiskLevels.mcp_list_tools).toBe("low");
    expect(toolRiskLevels.mcp_call_tool).toBe("medium");
  });

  it("returns static level for known tools", () => {
    expect(getToolRiskLevel("container_list")).toBe("low");
    expect(getToolRiskLevel("container_delete")).toBe("high");
  });

  it("detects destructive keywords for unknown tools", () => {
    expect(getToolRiskLevel("mcp_server1_delete_all")).toBe("high");
    expect(getToolRiskLevel("mcp_s1_remove_user")).toBe("high");
    expect(getToolRiskLevel("mcp_s1_destroy_db")).toBe("high");
    expect(getToolRiskLevel("mcp_s1_wipe_data")).toBe("high");
    expect(getToolRiskLevel("mcp_s1_prune_images")).toBe("high");
    expect(getToolRiskLevel("mcp_s1_terminate_process")).toBe("high");
    expect(getToolRiskLevel("mcp_s1_kill_session")).toBe("high");
  });

  it("returns 'medium' for unknown tools without destructive keywords", () => {
    expect(getToolRiskLevel("mcp_server1_get_info")).toBe("medium");
    expect(getToolRiskLevel("mcp_server1_list_items")).toBe("medium");
    expect(getToolRiskLevel("totally_unknown_tool")).toBe("medium");
  });
});

// ---------------------------------------------------------------------------
// getToolLabel tests
// ---------------------------------------------------------------------------
describe("getToolLabel", () => {
  it("returns tool label for known tools", () => {
    expect(getToolLabel("container_list")).toBe("List Containers");
    expect(getToolLabel("container_create")).toBe("Create Container");
    expect(getToolLabel("container_delete")).toBe("Delete Container");
  });

  it("returns tool name as fallback for unknown tools", () => {
    expect(getToolLabel("unknown_tool")).toBe("unknown_tool");
    expect(getToolLabel("mcp_s1_custom")).toBe("mcp_s1_custom");
  });
});

// ---------------------------------------------------------------------------
// Agent helper function tests
// ---------------------------------------------------------------------------
describe("agent helpers", () => {
  // We need to import after mock setup
  it("createUserMessage creates a valid user message", async () => {
    const { createUserMessage } = await import("@/lib/agent");

    const msg = createUserMessage("Hello agent");

    expect(msg.role).toBe("user");
    expect(msg.content).toBe("Hello agent");
    expect(msg.timestamp).toBeDefined();
    expect(typeof msg.timestamp).toBe("number");
  });
});
