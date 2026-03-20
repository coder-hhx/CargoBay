import { describe, it, expect, vi } from "vitest";

// Mock Tauri invoke before importing modules that use it
vi.mock("@/lib/tauri", () => ({
  invoke: vi.fn(() => Promise.reject(new Error("Tauri not available in test"))),
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import {
  createMcpAgentTool,
  mcpSchemaToTypebox,
  buildMcpBridgeTools,
  mcpListToolsTool,
  mcpCallToolTool,
  mcpTools,
} from "@/tools/mcpTools";
import type { McpToolInfo } from "@/types/mcp";

// ---------------------------------------------------------------------------
// mcpSchemaToTypebox
// ---------------------------------------------------------------------------
describe("mcpSchemaToTypebox", () => {
  it("converts a simple JSON Schema to TypeBox TSchema", () => {
    const schema = {
      type: "object",
      properties: {
        name: { type: "string" },
      },
      required: ["name"],
    };
    const result = mcpSchemaToTypebox(schema);
    // TypeBox wraps with Type.Unsafe, which preserves the original schema
    expect(result).toBeDefined();
    // The resulting schema should contain the original properties
    expect(result.type).toBe("object");
  });

  it("handles empty object schema", () => {
    const schema = { type: "object", properties: {} };
    const result = mcpSchemaToTypebox(schema);
    expect(result).toBeDefined();
    expect(result.type).toBe("object");
  });

  it("preserves complex nested schema", () => {
    const schema = {
      type: "object",
      properties: {
        config: {
          type: "object",
          properties: {
            port: { type: "integer", minimum: 1 },
          },
        },
        tags: {
          type: "array",
          items: { type: "string" },
        },
      },
    };
    const result = mcpSchemaToTypebox(schema);
    expect(result).toBeDefined();
  });
});

// ---------------------------------------------------------------------------
// createMcpAgentTool
// ---------------------------------------------------------------------------
describe("createMcpAgentTool", () => {
  it("generates correct tool name format: mcp_{serverId}_{toolName}", () => {
    const tool = createMcpAgentTool("my-server", {
      name: "list_files",
      description: "List files in directory",
      inputSchema: { type: "object", properties: {} },
    });
    expect(tool.name).toBe("mcp_my-server_list_files");
  });

  it("generates correct label format: [{serverId}] {toolName}", () => {
    const tool = createMcpAgentTool("shadcn", {
      name: "get_component",
      description: "Get a component",
      inputSchema: { type: "object" },
    });
    expect(tool.label).toBe("[shadcn] get_component");
  });

  it("uses the MCP tool description", () => {
    const tool = createMcpAgentTool("server1", {
      name: "tool1",
      description: "My awesome tool description",
      inputSchema: { type: "object" },
    });
    expect(tool.description).toBe("My awesome tool description");
  });

  it("converts inputSchema via mcpSchemaToTypebox", () => {
    const schema = {
      type: "object",
      properties: {
        query: { type: "string" },
      },
      required: ["query"],
    };
    const tool = createMcpAgentTool("server1", {
      name: "search",
      description: "Search",
      inputSchema: schema,
    });
    expect(tool.parameters).toBeDefined();
    expect(tool.parameters.type).toBe("object");
  });

  it("has an execute function", () => {
    const tool = createMcpAgentTool("server1", {
      name: "tool1",
      description: "test",
      inputSchema: { type: "object" },
    });
    expect(typeof tool.execute).toBe("function");
  });
});

// ---------------------------------------------------------------------------
// buildMcpBridgeTools
// ---------------------------------------------------------------------------
describe("buildMcpBridgeTools", () => {
  it("returns empty array for empty input", () => {
    const result = buildMcpBridgeTools([]);
    expect(result).toEqual([]);
  });

  it("creates one AgentTool per McpToolInfo", () => {
    const tools: McpToolInfo[] = [
      {
        serverId: "server1",
        serverName: "Server One",
        name: "tool_a",
        description: "Tool A",
        inputSchema: { type: "object" },
      },
      {
        serverId: "server1",
        serverName: "Server One",
        name: "tool_b",
        description: "Tool B",
        inputSchema: { type: "object" },
      },
      {
        serverId: "server2",
        serverName: "Server Two",
        name: "tool_c",
        description: "Tool C",
        inputSchema: { type: "object" },
      },
    ];
    const result = buildMcpBridgeTools(tools);
    expect(result).toHaveLength(3);
    expect(result[0].name).toBe("mcp_server1_tool_a");
    expect(result[1].name).toBe("mcp_server1_tool_b");
    expect(result[2].name).toBe("mcp_server2_tool_c");
  });

  it("uses serverId from McpToolInfo (not serverName)", () => {
    const tools: McpToolInfo[] = [
      {
        serverId: "my-id",
        serverName: "My Display Name",
        name: "my_tool",
        description: "test",
        inputSchema: { type: "object" },
      },
    ];
    const result = buildMcpBridgeTools(tools);
    expect(result[0].name).toBe("mcp_my-id_my_tool");
    expect(result[0].label).toBe("[my-id] my_tool");
  });
});

// ---------------------------------------------------------------------------
// Built-in MCP tools (static tools)
// ---------------------------------------------------------------------------
describe("built-in MCP tools", () => {
  it("mcpTools array contains exactly 2 tools", () => {
    expect(mcpTools).toHaveLength(2);
  });

  it("mcpListToolsTool has correct name", () => {
    expect(mcpListToolsTool.name).toBe("mcp_list_tools");
  });

  it("mcpListToolsTool has correct label", () => {
    expect(mcpListToolsTool.label).toBe("List MCP Tools");
  });

  it("mcpCallToolTool has correct name", () => {
    expect(mcpCallToolTool.name).toBe("mcp_call_tool");
  });

  it("mcpCallToolTool has correct label", () => {
    expect(mcpCallToolTool.label).toBe("Call MCP Tool");
  });

  it("mcpCallToolTool has required parameters", () => {
    expect(mcpCallToolTool.parameters).toBeDefined();
    // The parameters should include serverId and toolName
    const schema = mcpCallToolTool.parameters;
    expect(schema.properties).toBeDefined();
    expect(schema.properties.serverId).toBeDefined();
    expect(schema.properties.toolName).toBeDefined();
  });
});
