import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import { useMcpStore } from "@/stores/mcpStore";
import type { McpServerInfo } from "@/types/mcp";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Play, Square, Trash2, Settings, Unplug } from "lucide-react";

interface McpServerListProps {
  selectedServerId: string | null;
  onSelectServer: (id: string | null) => void;
  onEditServer: (server: McpServerInfo) => void;
}

/**
 * MCP server list with status indicators and action buttons.
 */
export function McpServerList({
  selectedServerId,
  onSelectServer,
  onEditServer,
}: McpServerListProps) {
  const { t } = useI18n();
  const servers = useMcpStore((s) => s.servers);
  const loading = useMcpStore((s) => s.loading);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-8 text-sm text-muted-foreground">
        {t("mcp", "loadingServers")}
      </div>
    );
  }

  if (servers.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-8 text-center text-sm text-muted-foreground">
        <Unplug className="mb-2 h-8 w-8 opacity-30" />
        <p>{t("mcp", "noServers")}</p>
        <p className="mt-1 text-xs">{t("mcp", "noServersHint")}</p>
      </div>
    );
  }

  return (
    <div className="space-y-1">
      {servers.map((server) => (
        <McpServerRow
          key={server.id}
          server={server}
          isSelected={server.id === selectedServerId}
          onSelect={() => onSelectServer(server.id === selectedServerId ? null : server.id)}
          onEdit={() => onEditServer(server)}
        />
      ))}
    </div>
  );
}

interface McpServerRowProps {
  server: McpServerInfo;
  isSelected: boolean;
  onSelect: () => void;
  onEdit: () => void;
}

function McpServerRow({ server, isSelected, onSelect, onEdit }: McpServerRowProps) {
  const { t } = useI18n();
  const startServer = useMcpStore((s) => s.startServer);
  const stopServer = useMcpStore((s) => s.stopServer);
  const removeServer = useMcpStore((s) => s.removeServer);

  const isConnected = server.status === "connected";
  const isStarting = server.status === "starting";

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") onSelect();
      }}
      className={cn(
        "flex items-center gap-3 rounded-md px-3 py-2.5 text-sm transition-colors cursor-pointer",
        isSelected ? "bg-primary/10" : "hover:bg-muted/50",
      )}
    >
      {/* Status indicator */}
      <div
        className={cn(
          "h-2 w-2 flex-shrink-0 rounded-full",
          isConnected && "bg-success",
          server.status === "disconnected" && "bg-muted-foreground",
          server.status === "error" && "bg-destructive",
          isStarting && "animate-pulse bg-yellow-500",
        )}
      />

      {/* Server info */}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate font-medium text-foreground">{server.name}</span>
          <StatusBadge status={server.status} />
        </div>
        <p className="mt-0.5 truncate text-xs text-muted-foreground">
          {server.command} {server.args.join(" ")}
        </p>
      </div>

      {/* Tool count */}
      {server.toolCount > 0 && (
        <Badge variant="outline" className="text-[10px]">
          {server.toolCount} {t("mcp", "tools")}
        </Badge>
      )}

      {/* Actions */}
      <div className="flex items-center gap-0.5" onClick={(e) => e.stopPropagation()}>
        <TooltipProvider delayDuration={300}>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-xs" onClick={onEdit} aria-label={t("mcp", "editServer")}>
                <Settings className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("common", "edit")}</TooltipContent>
          </Tooltip>

          {isConnected ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => void stopServer(server.id)}
                  aria-label={t("mcp", "stopServer")}
                >
                  <Square className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("common", "stop")}</TooltipContent>
            </Tooltip>
          ) : (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => void startServer(server.id)}
                  disabled={isStarting}
                  aria-label={t("mcp", "startServer")}
                >
                  <Play className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("common", "start")}</TooltipContent>
            </Tooltip>
          )}

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => void removeServer(server.id)}
                aria-label={t("mcp", "removeServer")}
              >
                <Trash2 className="h-3.5 w-3.5 text-destructive" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("common", "remove")}</TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: McpServerInfo["status"] }) {
  const { t } = useI18n();
  const config: Record<typeof status, { labelKey: "connected" | "disconnected" | "error" | "starting"; className: string }> = {
    connected: {
      labelKey: "connected",
      className: "border-success/30 bg-success/10 text-success",
    },
    disconnected: {
      labelKey: "disconnected",
      className: "border-muted bg-muted/50 text-muted-foreground",
    },
    error: {
      labelKey: "error",
      className: "border-destructive/30 bg-destructive/10 text-destructive",
    },
    starting: {
      labelKey: "starting",
      className: "border-yellow-600/30 bg-yellow-600/10 text-yellow-500",
    },
  };

  const variant = config[status];

  return (
    <Badge variant="outline" className={cn("text-[10px]", variant.className)}>
      {t("mcp", variant.labelKey)}
    </Badge>
  );
}
