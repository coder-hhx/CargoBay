import { useCallback, useEffect, useState } from "react";
import { useMcpStore } from "@/stores/mcpStore";
import type { McpServerInfo } from "@/types/mcp";
import type { McpServerConfig } from "@/types/mcp";
import { McpServerList } from "@/components/mcp/McpServerList";
import { McpToolList } from "@/components/mcp/McpToolList";
import { McpServerConfigDialog } from "@/components/mcp/McpServerConfig";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Plus, Plug } from "lucide-react";

/**
 * MCP server management page.
 * Configure, monitor, and test MCP server connections.
 *
 * Layout:
 * - Server list (top)
 * - Tool list from selected server (middle)
 * - Server logs (bottom, expandable)
 */
export function McpPage() {
  const fetchServers = useMcpStore((s) => s.fetchServers);
  const fetchTools = useMcpStore((s) => s.fetchTools);
  const addServer = useMcpStore((s) => s.addServer);
  const updateServer = useMcpStore((s) => s.updateServer);
  const serverLogs = useMcpStore((s) => s.serverLogs);
  const fetchServerLogs = useMcpStore((s) => s.fetchServerLogs);

  const [selectedServerId, setSelectedServerId] = useState<string | null>(null);
  const [configDialogOpen, setConfigDialogOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<McpServerInfo | undefined>(undefined);

  // Fetch servers and tools on mount
  useEffect(() => {
    void fetchServers();
    void fetchTools();
  }, [fetchServers, fetchTools]);

  // Fetch logs when a server is selected
  useEffect(() => {
    if (selectedServerId !== null) {
      void fetchServerLogs(selectedServerId);
    }
  }, [selectedServerId, fetchServerLogs]);

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

  const logs = selectedServerId !== null ? (serverLogs[selectedServerId] ?? []) : [];

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <div className="flex items-center gap-2">
          <Plug className="h-5 w-5 text-muted-foreground" />
          <h1 className="text-lg font-semibold text-foreground">MCP Servers</h1>
        </div>
        <Button size="sm" onClick={handleAddServer}>
          <Plus className="mr-1 h-4 w-4" />
          Add Server
        </Button>
      </div>

      {/* Content */}
      <div className="flex flex-1 flex-col overflow-hidden">
        {/* Server list */}
        <div className="border-b border-border px-4 py-3">
          <McpServerList
            selectedServerId={selectedServerId}
            onSelectServer={setSelectedServerId}
            onEditServer={handleEditServer}
          />
        </div>

        {/* Tool list for selected server */}
        {selectedServerId !== null && (
          <div className="border-b border-border px-4 py-3">
            <h2 className="mb-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">
              Available Tools
            </h2>
            <McpToolList serverId={selectedServerId} />
          </div>
        )}

        {/* Server logs */}
        {selectedServerId !== null && logs.length > 0 && (
          <div className="flex-1 overflow-hidden px-4 py-3">
            <h2 className="mb-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">
              Server Logs
            </h2>
            <ScrollArea className="h-full max-h-48 rounded-md border border-border bg-muted/30">
              <div className="p-3 font-mono text-xs text-muted-foreground">
                {logs.map((line, idx) => (
                  <div key={idx} className="whitespace-pre-wrap break-all leading-5">
                    {line}
                  </div>
                ))}
              </div>
            </ScrollArea>
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
