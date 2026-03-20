import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";

// Mock Tauri before importing hooks that use stores
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import { useMcpToolSync } from "@/hooks/useMcpToolSync";
import { useMcpStore } from "@/stores/mcpStore";
import { builtinTools } from "@/tools";
import type { McpToolInfo } from "@/types/mcp";

// ---------------------------------------------------------------------------
// Mock Agent
// ---------------------------------------------------------------------------
interface MockAgent {
  setTools: ReturnType<typeof vi.fn>;
}

function createMockAgent(): MockAgent {
  return {
    setTools: vi.fn(),
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const makeTool = (overrides: Partial<McpToolInfo> = {}): McpToolInfo => ({
  serverId: "server-1",
  serverName: "Server One",
  name: "tool_a",
  description: "Test tool A",
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
describe("useMcpToolSync", () => {
  beforeEach(() => {
    resetStore();
    resetTauriMocks();
  });

  it("does not call setTools when agent is null", () => {
    renderHook(() => useMcpToolSync(null));

    // No error thrown, no setTools called
  });

  it("calls agent.setTools with builtin + MCP bridge tools when tools change", () => {
    const agent = createMockAgent();
    const tools = [
      makeTool({ serverId: "s1", name: "list_files" }),
      makeTool({ serverId: "s1", name: "read_file" }),
    ];

    // Set available tools in store before rendering the hook
    useMcpStore.setState({ availableTools: tools });

    // Cast mockAgent as Agent (the hook only uses setTools)
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    renderHook(() => useMcpToolSync(agent as any));

    expect(agent.setTools).toHaveBeenCalledTimes(1);

    const calledTools = agent.setTools.mock.calls[0][0];
    // Should contain all builtin tools + 2 MCP bridge tools
    expect(calledTools.length).toBe(builtinTools.length + 2);

    // Verify MCP bridge tools are present
    const toolNames = calledTools.map((t: { name: string }) => t.name);
    expect(toolNames).toContain("mcp_s1_list_files");
    expect(toolNames).toContain("mcp_s1_read_file");
  });

  it("updates tools when availableTools changes from non-empty to different", () => {
    const agent = createMockAgent();

    // Start with one MCP tool (non-empty, so initial key != "")
    useMcpStore.setState({
      availableTools: [makeTool({ serverId: "s1", name: "initial_tool" })],
    });

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const { rerender } = renderHook(() => useMcpToolSync(agent as any));

    // Initial call: builtin + 1 MCP tool
    expect(agent.setTools).toHaveBeenCalledTimes(1);
    expect(agent.setTools.mock.calls[0][0].length).toBe(builtinTools.length + 1);

    // Update: add another MCP tool
    act(() => {
      useMcpStore.setState({
        availableTools: [
          makeTool({ serverId: "s1", name: "initial_tool" }),
          makeTool({ serverId: "s1", name: "new_tool" }),
        ],
      });
    });
    rerender();

    expect(agent.setTools).toHaveBeenCalledTimes(2);
    const updatedTools = agent.setTools.mock.calls[1][0];
    expect(updatedTools.length).toBe(builtinTools.length + 2);
  });

  it("skips initial setTools when availableTools is empty (key matches default)", () => {
    const agent = createMockAgent();

    // Start with empty tools — key is "" which matches prevToolKeyRef default ""
    useMcpStore.setState({ availableTools: [] });

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    renderHook(() => useMcpToolSync(agent as any));

    // Hook skips update because toolKey === prevToolKeyRef.current === ""
    expect(agent.setTools).toHaveBeenCalledTimes(0);
  });

  it("does not call setTools again if tools haven't changed", () => {
    const agent = createMockAgent();
    const tools = [makeTool({ serverId: "s1", name: "tool_a" })];
    useMcpStore.setState({ availableTools: tools });

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const { rerender } = renderHook(() => useMcpToolSync(agent as any));

    expect(agent.setTools).toHaveBeenCalledTimes(1);

    // Re-render without changing tools
    rerender();

    // Should not call setTools again (tool key unchanged)
    expect(agent.setTools).toHaveBeenCalledTimes(1);
  });

  it("detects tool changes by serverId:name key", () => {
    const agent = createMockAgent();
    useMcpStore.setState({
      availableTools: [makeTool({ serverId: "s1", name: "tool_a" })],
    });

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const { rerender } = renderHook(() => useMcpToolSync(agent as any));
    expect(agent.setTools).toHaveBeenCalledTimes(1);

    // Change: different tool with same name but different server
    act(() => {
      useMcpStore.setState({
        availableTools: [makeTool({ serverId: "s2", name: "tool_a" })],
      });
    });
    rerender();

    // Should detect the change and call setTools again
    expect(agent.setTools).toHaveBeenCalledTimes(2);
  });
});
