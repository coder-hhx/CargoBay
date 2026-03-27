import { useCallback, useEffect, useMemo, useState } from "react";
import { useMcpStore } from "@/stores/mcpStore";
import { useI18n } from "@/lib/i18n";
import type { McpServerInfo } from "@/types/mcp";
import type { McpServerConfig } from "@/types/mcp";
import type { McpToolInfo } from "@/types/mcp";
import { McpServerConfigDialog } from "@/components/mcp/McpServerConfig";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import {
  Plus,
  Plug,
  Unplug,
  ChevronDown,
  Folder,
  Github,
  Database,
  Container,
  Globe,
  Network,
  MessageSquare,
  Play,
  Square,
  Trash2,
  Settings,
  Search,
} from "lucide-react";

/** Icon mapping based on server name keywords */
function getServerIcon(name: string) {
  const lower = name.toLowerCase();
  if (lower.includes("file") || lower.includes("fs")) return <Folder className="h-[18px] w-[18px]" />;
  if (lower.includes("github") || lower.includes("git")) return <Github className="h-[18px] w-[18px]" />;
  if (lower.includes("database") || lower.includes("db") || lower.includes("sql"))
    return <Database className="h-[18px] w-[18px]" />;
  if (lower.includes("docker") || lower.includes("container"))
    return <Container className="h-[18px] w-[18px]" />;
  if (lower.includes("web") || lower.includes("http") || lower.includes("search"))
    return <Globe className="h-[18px] w-[18px]" />;
  if (lower.includes("k8s") || lower.includes("kube") || lower.includes("network"))
    return <Network className="h-[18px] w-[18px]" />;
  return <Plug className="h-[18px] w-[18px]" />;
}

/**
 * MCP server management page.
 * Configure, monitor, and test MCP server connections.
 * Uses card grid layout with expandable accordion cards.
 */
export function McpPage() {
  const { t } = useI18n();
  const fetchServers = useMcpStore((s) => s.fetchServers);
  const fetchTools = useMcpStore((s) => s.fetchTools);
  const addServer = useMcpStore((s) => s.addServer);
  const updateServer = useMcpStore((s) => s.updateServer);
  const servers = useMcpStore((s) => s.servers);
  const availableTools = useMcpStore((s) => s.availableTools);
  const loading = useMcpStore((s) => s.loading);

  const [configDialogOpen, setConfigDialogOpen] = useState(false);
  const [searchFilter, setSearchFilter] = useState("");
  const [editingServer, setEditingServer] = useState<McpServerInfo | undefined>(undefined);

  // Fetch servers and tools on mount
  useEffect(() => {
    void fetchServers();
    void fetchTools();
  }, [fetchServers, fetchTools]);

  const handleAddServer = useCallback(() => {
    setEditingServer(undefined);
    setConfigDialogOpen(true);
  }, []);

  const handleEditServer = useCallback((server: McpServerInfo) => {
    setEditingServer(server);
    setConfigDialogOpen(true);
  }, []);

  const handleSaveConfig = useCallback(
    (config: McpServerConfig) => {
      if (editingServer !== undefined) {
        void updateServer(editingServer.id, config);
      } else {
        void addServer(config);
      }
      setConfigDialogOpen(false);
      setEditingServer(undefined);
    },
    [editingServer, addServer, updateServer],
  );

  // Stats for header
  const connectedCount = useMemo(
    () => servers.filter((s) => s.status === "connected").length,
    [servers],
  );
  const totalTools = useMemo(
    () => servers.reduce((sum, s) => sum + s.toolCount, 0),
    [servers],
  );

  const filteredServers = useMemo(() => {
    if (searchFilter.length === 0) return servers;
    const q = searchFilter.toLowerCase();
    return servers.filter((s) => s.name.toLowerCase().includes(q));
  }, [servers, searchFilter]);

  return (
    <div className="flex h-full flex-col">
      {/* Unified toolbar */}
      <div className="flex items-center gap-3 border-b border-border px-6 py-2.5">
        <div className="relative w-56">
          <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={searchFilter}
            onChange={(e) => setSearchFilter(e.target.value)}
            placeholder="搜索服务器..."
            className="h-8 pl-8 text-xs"
          />
        </div>
        <div className="h-4 w-px bg-border" />
        <p className="text-xs text-muted-foreground">
          {servers.length} {t("mcp", "serversCount")} &middot; {connectedCount} {t("mcp", "connected").toLowerCase()} &middot; {totalTools} {t("mcp", "toolsAvailable")}
        </p>
        <div className="ml-auto">
          <Button size="sm" onClick={handleAddServer} data-testid="add-mcp-server">
            <Plus className="mr-1 h-4 w-4" />
            {t("mcp", "addServer")}
          </Button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto px-6 py-4" data-testid="mcp-server-list">
        {loading ? (
          <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
            {t("mcp", "loadingServers")}
          </div>
        ) : servers.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
            <Unplug className="mb-3 h-12 w-12 opacity-20" />
            <h3 className="text-sm font-medium">{t("mcp", "noServers")}</h3>
            <p className="mt-1 text-xs">
              {t("mcp", "noServersHint")}
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(380px,1fr))] gap-4">
            {filteredServers.map((server) => (
              <McpServerCard
                key={server.id}
                server={server}
                tools={availableTools.filter((t) => t.serverId === server.id)}
                onEdit={() => handleEditServer(server)}
              />
            ))}
          </div>
        )}
      </div>

      {/* Config dialog */}
      <McpServerConfigDialog
        server={editingServer}
        open={configDialogOpen}
        onClose={() => {
          setConfigDialogOpen(false);
          setEditingServer(undefined);
        }}
        onSave={handleSaveConfig}
      />
    </div>
  );
}

/**
 * Accordion-style MCP server card.
 * Click header to expand/collapse tool list.
 */
function McpServerCard({
  server,
  tools,
  onEdit,
}: {
  server: McpServerInfo;
  tools: McpToolInfo[];
  onEdit: () => void;
}) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  const startServer = useMcpStore((s) => s.startServer);
  const stopServer = useMcpStore((s) => s.stopServer);
  const removeServer = useMcpStore((s) => s.removeServer);

  const isConnected = server.status === "connected";
  const isStarting = server.status === "starting";

  return (
    <div
      data-testid="mcp-server-card"
      className={cn(
        "rounded-xl border bg-card transition-all",
        expanded && "border-primary/50",
        !expanded && "border-border hover:border-primary/30",
      )}
    >
      {/* Header — click to expand/collapse */}
      <div
        role="button"
        tabIndex={0}
        onClick={() => setExpanded(!expanded)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") setExpanded(!expanded);
        }}
        className="flex cursor-pointer items-center gap-3 px-4 py-3"
      >
        {/* Category icon */}
        <div className="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
          {getServerIcon(server.name)}
        </div>

        {/* Server info */}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-semibold text-foreground">{server.name}</span>
          </div>
          <div className="mt-0.5 flex items-center gap-2">
            <span className="rounded border border-border bg-muted/50 px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
              {server.transport}
            </span>
            <ServerStatusBadge status={server.status} />
            <span className="text-[10px] text-muted-foreground">
              {server.toolCount} {t("mcp", "tools")}
            </span>
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-0.5" onClick={(e) => e.stopPropagation()}>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onEdit}>
            <Settings className="h-3.5 w-3.5" />
          </Button>
          {isConnected ? (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={() => void stopServer(server.id)}
              data-testid="mcp-disconnect"
            >
              <Square className="h-3.5 w-3.5" />
            </Button>
          ) : (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={() => void startServer(server.id)}
              disabled={isStarting}
              data-testid="mcp-connect"
            >
              <Play className="h-3.5 w-3.5" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-destructive hover:text-destructive"
            onClick={() => void removeServer(server.id)}
            data-testid="mcp-delete"
          >
            <Trash2 className="h-3.5 w-3.5" />
          </Button>
        </div>

        {/* Chevron */}
        <ChevronDown
          className={cn(
            "h-4 w-4 flex-shrink-0 text-muted-foreground transition-transform duration-200",
            expanded && "rotate-180",
          )}
        />
      </div>

      {/* Expanded content — tool list */}
      {expanded && (
        <div className="border-t border-border px-4 pb-3 pt-2" data-testid="mcp-tool-list">
          {/* Server command info */}
          <p className="mb-2 truncate font-mono text-[11px] text-muted-foreground">
            {server.command} {server.args.join(" ")}
          </p>

          {server.error && (
            <p className="mb-2 text-xs text-destructive">{server.error}</p>
          )}

          {tools.length > 0 ? (
            <div className="space-y-1">
              {tools.map((tool) => (
                <div
                  key={`${tool.serverId}-${tool.name}`}
                  data-testid="mcp-tool-item"
                  className="flex items-center gap-2 rounded-md px-2 py-1.5 transition-colors hover:bg-muted/50"
                >
                  <div className="min-w-0 flex-1">
                    <span className="font-mono text-xs font-medium text-primary">
                      {tool.name}
                    </span>
                    {tool.description.length > 0 && (
                      <p className="mt-0.5 text-[11px] leading-tight text-muted-foreground">
                        {tool.description}
                      </p>
                    )}
                  </div>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-6 w-6 flex-shrink-0"
                    title={t("mcp", "useInChat")}
                  >
                    <MessageSquare className="h-3.5 w-3.5" />
                  </Button>
                </div>
              ))}
            </div>
          ) : (
            <p className="py-2 text-center text-xs text-muted-foreground">
              {t("mcp", "noTools")}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function ServerStatusBadge({ status }: { status: McpServerInfo["status"] }) {
  const { t } = useI18n();
  const config: Record<typeof status, { labelKey: "connected" | "disconnected" | "error" | "starting"; dotClass: string; textClass: string }> = {
    connected: {
      labelKey: "connected",
      dotClass: "bg-success",
      textClass: "text-success",
    },
    disconnected: {
      labelKey: "disconnected",
      dotClass: "bg-muted-foreground",
      textClass: "text-muted-foreground",
    },
    error: {
      labelKey: "error",
      dotClass: "bg-destructive",
      textClass: "text-destructive",
    },
    starting: {
      labelKey: "starting",
      dotClass: "bg-yellow-500 animate-pulse",
      textClass: "text-yellow-500",
    },
  };

  const variant = config[status] ?? {
    labelKey: "disconnected" as const,
    dotClass: "bg-muted-foreground",
    textClass: "text-muted-foreground",
  };

  return (
    <span className={cn("flex items-center gap-1 text-[10px] font-medium", variant.textClass)}>
      <span className={cn("inline-block h-1.5 w-1.5 rounded-full", variant.dotClass)} />
      {t("mcp", variant.labelKey)}
    </span>
  );
}
