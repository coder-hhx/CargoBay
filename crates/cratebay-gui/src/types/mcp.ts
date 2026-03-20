/**
 * MCP server/tool type definitions for CrateBay.
 *
 * Matches frontend-spec.md §4.4 — mcpStore types.
 */

/**
 * MCP server information including runtime status.
 */
export interface McpServerInfo {
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

/**
 * Configuration for adding or editing an MCP server.
 */
export interface McpServerConfig {
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  enabled?: boolean;
  transport?: "stdio" | "sse";
}

/**
 * Information about a tool provided by an MCP server.
 */
export interface McpToolInfo {
  serverId: string;
  serverName: string;
  name: string;
  description: string;
  inputSchema: Record<string, unknown>; // JSON Schema
}
