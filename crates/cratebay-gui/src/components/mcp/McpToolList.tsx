import { useI18n } from "@/lib/i18n";
import { useMcpStore } from "@/stores/mcpStore";
import type { McpToolInfo } from "@/types/mcp";
import { Wrench } from "lucide-react";

interface McpToolListProps {
  serverId: string;
}

/**
 * List of available tools from a selected MCP server.
 * Shows tool name, description, and input schema.
 */
export function McpToolList({ serverId }: McpToolListProps) {
  const { t } = useI18n();
  const availableTools = useMcpStore((s) => s.availableTools);
  const tools = availableTools.filter((tool) => tool.serverId === serverId);

  if (tools.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-6 text-center text-sm text-muted-foreground">
        <Wrench className="mb-2 h-6 w-6 opacity-30" />
        <p>{t("mcp", "noTools")}</p>
      </div>
    );
  }

  return (
    <div className="space-y-1" data-testid="mcp-tool-list">
      {tools.map((tool) => (
        <McpToolRow key={`${tool.serverId}-${tool.name}`} tool={tool} />
      ))}
    </div>
  );
}

function McpToolRow({ tool }: { tool: McpToolInfo }) {
  return (
    <div className="rounded-md px-3 py-2 text-sm transition-colors hover:bg-muted/50" data-testid="mcp-tool-item">
      <div className="flex items-center gap-2">
        <Wrench className="h-3.5 w-3.5 flex-shrink-0 text-muted-foreground" />
        <span className="font-medium text-foreground">{tool.name}</span>
      </div>
      {tool.description.length > 0 && (
        <p className="mt-0.5 pl-5.5 text-xs text-muted-foreground">{tool.description}</p>
      )}
    </div>
  );
}
