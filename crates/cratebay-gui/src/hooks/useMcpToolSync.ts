/**
 * useMcpToolSync — Dynamic MCP tool synchronization hook.
 *
 * Watches mcpStore.availableTools for changes and dynamically builds
 * MCP bridge AgentTools that wrap each external MCP tool as a native
 * tool for pi-agent-core. When the available tools change (servers
 * connect/disconnect), the agent's tool set is updated to include
 * both built-in tools and MCP bridge tools.
 *
 * See mcp-spec.md §4.2 — Dynamic Tool Registration.
 */

import { useEffect, useRef } from "react";
import type { Agent } from "@mariozechner/pi-agent-core";
import { useMcpStore } from "@/stores/mcpStore";
import { builtinTools } from "@/tools";
import { buildMcpBridgeTools } from "@/tools/mcpTools";

/**
 * Synchronize MCP tools with the pi-agent-core Agent.
 *
 * When `mcpStore.availableTools` changes, this hook:
 * 1. Builds MCP bridge tools via `createMcpAgentTool` for each available tool
 * 2. Merges them with the static built-in tools
 * 3. Calls `agent.setTools()` to update the agent's tool registry
 *
 * This ensures the LLM always sees the latest set of tools, including
 * tools from newly connected MCP servers and excluding tools from
 * disconnected servers.
 *
 * @param agent - The pi-agent-core Agent instance, or null if not yet created
 */
export function useMcpToolSync(agent: Agent | null): void {
  const availableTools = useMcpStore((s) => s.availableTools);

  // Track the previous tool count to avoid unnecessary updates
  const prevToolKeyRef = useRef<string>("");

  useEffect(() => {
    if (!agent) return;

    // Build a stable key from the available tools to detect real changes
    const toolKey = availableTools
      .map((t) => `${t.serverId}:${t.name}`)
      .sort()
      .join(",");

    // Skip update if the tool set hasn't actually changed
    if (toolKey === prevToolKeyRef.current) return;
    prevToolKeyRef.current = toolKey;

    // Build MCP bridge tools from available tools
    const mcpBridgeTools = buildMcpBridgeTools(availableTools);

    // Merge built-in tools with MCP bridge tools and update the agent
    agent.setTools([...builtinTools, ...mcpBridgeTools]);
  }, [availableTools, agent]);
}
