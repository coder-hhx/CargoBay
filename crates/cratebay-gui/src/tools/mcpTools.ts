/**
 * MCP (Model Context Protocol) tools for the CrateBay Agent.
 *
 * Two categories of MCP tools:
 *
 * 1. **Built-in tools** (`mcp_list_tools`, `mcp_call_tool`):
 *    Static AgentTools that are always registered. They allow the LLM
 *    to discover and call arbitrary MCP tools via generic parameters.
 *
 * 2. **MCP Bridge tools** (dynamically created via `createMcpAgentTool`):
 *    Wrap each individual MCP tool from connected servers as a native
 *    AgentTool so the LLM sees them with proper parameter schemas.
 *    Created by `useMcpToolSync` hook at runtime.
 */

import { Type, type TSchema } from "@sinclair/typebox";
import type { AgentTool, AgentToolResult } from "@mariozechner/pi-agent-core";
import { invoke } from "@/lib/tauri";
import type { McpToolInfo } from "@/types/mcp";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function textResult(text: string): AgentToolResult<undefined> {
  return {
    content: [{ type: "text", text }],
    details: undefined,
  };
}

// ---------------------------------------------------------------------------
// §4.1 — MCP Schema Mapping
// ---------------------------------------------------------------------------

/**
 * Convert an MCP tool JSON Schema to a TypeBox TSchema.
 *
 * MCP tools use standard JSON Schema for their `inputSchema`. TypeBox
 * generates compatible JSON Schema, so `Type.Unsafe()` provides a
 * direct pass-through without loss of information.
 *
 * @param mcpSchema - JSON Schema from the MCP tool definition
 * @returns A TypeBox TSchema wrapping the original schema
 */
export function mcpSchemaToTypebox(mcpSchema: Record<string, unknown>): TSchema {
  return Type.Unsafe(mcpSchema);
}

// ---------------------------------------------------------------------------
// §4.1 — MCP Bridge Tool Factory
// ---------------------------------------------------------------------------

/**
 * MCP tool definition shape expected by `createMcpAgentTool`.
 *
 * This is the minimal contract needed to create a bridge tool;
 * it matches the relevant fields of `McpToolInfo`.
 */
export interface McpToolDefinition {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

/**
 * Create an AgentTool that wraps a single MCP tool from an external server.
 *
 * The returned tool:
 * - Has a unique name: `mcp_${serverId}_${toolName}`
 * - Uses the MCP tool's JSON Schema for parameter validation
 * - Forwards execution to the Rust MCP client via `mcp_client_call_tool`
 * - Reports progress via `onUpdate` callback
 * - Propagates errors with context for LLM error recovery
 *
 * @param serverId - MCP server identifier
 * @param mcpTool - Tool definition from the MCP server
 * @returns An AgentTool instance for pi-agent-core
 */
export function createMcpAgentTool(
  serverId: string,
  mcpTool: McpToolDefinition,
): AgentTool<TSchema> {
  const toolName = mcpTool.name;

  return {
    name: `mcp_${serverId}_${toolName}`,
    label: `[${serverId}] ${toolName}`,
    description: mcpTool.description,
    parameters: mcpSchemaToTypebox(mcpTool.inputSchema),
    execute: async (_toolCallId, params, _signal, onUpdate) => {
      onUpdate?.({
        content: [{ type: "text", text: `Calling ${toolName} on ${serverId}...` }],
        details: { status: "running" },
      });

      try {
        const result = await invoke<unknown>("mcp_client_call_tool", {
          serverId,
          toolName,
          arguments: params,
        });

        return {
          content: [{ type: "text", text: JSON.stringify(result) }],
          details: result,
        };
      } catch (err) {
        throw new Error(
          `MCP tool "${toolName}" on server "${serverId}" failed: ${String(err)}`,
        );
      }
    },
  };
}

/**
 * Build MCP bridge tools from an array of McpToolInfo.
 *
 * Convenience function used by `useMcpToolSync` to batch-create
 * bridge tools from the mcpStore's `availableTools` state.
 *
 * @param tools - Available tools from mcpStore
 * @returns Array of AgentTools wrapping each MCP tool
 */
export function buildMcpBridgeTools(tools: McpToolInfo[]): AgentTool<TSchema>[] {
  return tools.map((tool) =>
    createMcpAgentTool(tool.serverId, {
      name: tool.name,
      description: tool.description,
      inputSchema: tool.inputSchema,
    }),
  );
}

// ---------------------------------------------------------------------------
// Built-in MCP tools (static, always registered)
// ---------------------------------------------------------------------------

const McpListToolsParams = Type.Object({});

const McpCallToolParams = Type.Object({
  serverId: Type.String({
    description: "MCP server ID to call the tool on",
  }),
  toolName: Type.String({
    description: "Name of the MCP tool to invoke",
  }),
  arguments: Type.Optional(
    Type.Record(Type.String(), Type.Unknown(), {
      description: "Arguments to pass to the MCP tool as key-value pairs",
    }),
  ),
});

export const mcpListToolsTool: AgentTool<typeof McpListToolsParams> = {
  name: "mcp_list_tools",
  label: "List MCP Tools",
  description:
    "List all available tools from connected MCP servers. " +
    "Returns server IDs, tool names, and descriptions. " +
    "Use this to discover what MCP tools are available before calling them.",
  parameters: McpListToolsParams,
  execute: async () => {
    const tools = await invoke<McpToolInfo[]>("mcp_client_list_tools");

    if (!Array.isArray(tools) || tools.length === 0) {
      return textResult("No MCP servers are connected or no tools are available.");
    }

    // Group tools by server
    const byServer = new Map<string, McpToolInfo[]>();
    for (const tool of tools) {
      const key = tool.serverId;
      const group = byServer.get(key);
      if (group) {
        group.push(tool);
      } else {
        byServer.set(key, [tool]);
      }
    }

    const sections: string[] = [];
    for (const [serverId, serverTools] of byServer) {
      const serverName = serverTools[0]?.serverName ?? serverId;
      const toolLines = serverTools.map(
        (t) => `  - **${t.name}**: ${t.description}`,
      );
      sections.push(`### ${serverName} (${serverId})\n${toolLines.join("\n")}`);
    }

    return textResult(`**Available MCP Tools:**\n\n${sections.join("\n\n")}`);
  },
};

export const mcpCallToolTool: AgentTool<typeof McpCallToolParams> = {
  name: "mcp_call_tool",
  label: "Call MCP Tool",
  description:
    "Call a tool on a connected MCP server. " +
    "Requires the server ID and tool name. " +
    "Use mcp_list_tools first to discover available tools and their parameters. " +
    "Risk level varies depending on the target tool — destructive tools require confirmation.",
  parameters: McpCallToolParams,
  execute: async (_toolCallId, params) => {
    const result = await invoke<{
      content: Array<{
        type: string;
        text?: string;
        data?: string;
        mimeType?: string;
      }>;
      isError?: boolean;
    }>("mcp_client_call_tool", {
      serverId: params.serverId,
      toolName: params.toolName,
      arguments: params.arguments ?? {},
    });

    if (result.isError) {
      throw new Error(
        `MCP tool '${params.toolName}' on server '${params.serverId}' returned an error: ` +
        (result.content.map((c) => c.text ?? "").join("\n") || "Unknown error"),
      );
    }

    const textParts = result.content
      .filter((c) => c.type === "text" && c.text)
      .map((c) => c.text!);

    if (textParts.length === 0) {
      return textResult(`MCP tool **${params.toolName}** executed successfully (no text output).`);
    }

    return textResult(textParts.join("\n"));
  },
};

/**
 * All built-in MCP tools exported as an array for the tool registry.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const mcpTools: AgentTool<any>[] = [
  mcpListToolsTool,
  mcpCallToolTool,
];
