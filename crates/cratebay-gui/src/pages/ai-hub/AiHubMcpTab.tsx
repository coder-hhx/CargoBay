import { I } from "../../icons"
import { EmptyState } from "../../components/EmptyState"
import { ErrorInline } from "../../components/ErrorDisplay"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { TabsContent } from "@/components/ui/tabs"
import { cardActionDanger, cardActionOutline, cardActionSecondary, iconStroke } from "@/lib/styles"
import { cn } from "@/lib/utils"
import type { McpServerEntry, McpServerStatusDto } from "../../types"
import { formatAiHubTime, getMcpStatusLabel, type TranslateFn } from "./utils"

interface AiHubMcpTabProps {
  t: TranslateFn
  mcpServers: McpServerStatusDto[]
  mcpDrafts: McpServerEntry[]
  mcpSelectedId: string
  mcpLogs: string[]
  mcpExportClient: string
  mcpExportValue: string
  mcpLoading: boolean
  mcpError: string
  mcpActingId: string
  selectedMcpDraft: McpServerEntry | null
  selectedMcpStatus: McpServerStatusDto | null
  onRefreshMcp: () => void
  onDismissMcpError: () => void
  onAddMcpServer: () => void
  onSaveMcpRegistry: () => void
  onSelectMcpServer: (id: string) => void
  onUpdateSelectedMcpDraft: (updater: (draft: McpServerEntry) => McpServerEntry) => void
  onDeleteMcpServer: (id: string) => void
  onMcpAction: (action: "start" | "stop" | "export", id?: string) => void
  onMcpExportClientChange: (value: string) => void
}

export function AiHubMcpTab({
  t,
  mcpServers,
  mcpDrafts,
  mcpSelectedId,
  mcpLogs,
  mcpExportClient,
  mcpExportValue,
  mcpLoading,
  mcpError,
  mcpActingId,
  selectedMcpDraft,
  selectedMcpStatus,
  onRefreshMcp,
  onDismissMcpError,
  onAddMcpServer,
  onSaveMcpRegistry,
  onSelectMcpServer,
  onUpdateSelectedMcpDraft,
  onDeleteMcpServer,
  onMcpAction,
  onMcpExportClientChange,
}: AiHubMcpTabProps) {
  return (
    <TabsContent value="mcp" className="space-y-3">
      <Card className="py-0">
        <CardContent className="space-y-3 py-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <div className="text-sm font-semibold text-foreground">{t("mcpRegistryTitle")}</div>
              <div className="mt-1 text-xs text-muted-foreground">{t("mcpRegistryDesc")}</div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Button
                type="button"
                variant="outline"
                size="xs"
                className={cn(cardActionOutline)}
                onClick={onRefreshMcp}
                disabled={mcpLoading}
              >
                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.refresh}</span>
                {mcpLoading ? t("working") : t("refresh")}
              </Button>
              <Button type="button" size="xs" data-testid="mcp-add-server" className={cn(cardActionSecondary)} onClick={onAddMcpServer}>
                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.play}</span>
                {t("mcpAddServer")}
              </Button>
              <Button
                type="button"
                size="xs"
                data-testid="mcp-save-registry"
                className={cn(cardActionSecondary)}
                onClick={onSaveMcpRegistry}
                disabled={mcpActingId === "save"}
              >
                {mcpActingId === "save" ? t("working") : t("mcpSaveRegistry")}
              </Button>
            </div>
          </div>
          {mcpError && <ErrorInline message={mcpError} onDismiss={onDismissMcpError} />}
        </CardContent>
      </Card>

      <div className="grid gap-3 xl:grid-cols-[0.95fr_1.05fr]">
        <Card className="py-0">
          <CardContent className="py-0">
            <div className="border-b border-border/50 px-4 py-3 text-xs text-muted-foreground">{t("mcp")}</div>
            {mcpDrafts.length === 0 ? (
              <div className="px-4 py-6">
                <EmptyState icon={I.globe} title={t("mcpNoServersTitle")} description={t("mcpNoServersDesc")} />
              </div>
            ) : (
              <ScrollArea className="max-h-[520px]">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>{t("name")}</TableHead>
                      <TableHead>{t("mcpStatus")}</TableHead>
                      <TableHead className="text-right">{t("actions")}</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {mcpDrafts.map((server) => {
                      const runtime = mcpServers.find((item) => item.id === server.id)
                      const selected = mcpSelectedId === server.id
                      const action = runtime?.running ? "stop" : "start"
                      return (
                        <TableRow key={server.id} data-testid={`mcp-row-${server.id}`} className={selected ? "bg-primary/5" : ""}>
                          <TableCell>
                            <button type="button" data-testid={`mcp-select-${server.id}`} className="text-left" onClick={() => onSelectMcpServer(server.id)}>
                              <div className="text-sm font-medium text-foreground">{server.name || server.id}</div>
                              <div className="text-xs font-mono text-muted-foreground">{server.id}</div>
                            </button>
                          </TableCell>
                          <TableCell data-testid={`mcp-status-${server.id}`} className="text-xs text-muted-foreground">{getMcpStatusLabel(runtime, t)}</TableCell>
                          <TableCell className="text-right">
                            <div className="flex justify-end gap-2">
                              <Button
                                type="button"
                                variant="outline"
                                size="xs"
                                data-testid={`mcp-toggle-${server.id}`}
                                className={runtime?.running ? cn(cardActionDanger) : cn(cardActionSecondary)}
                                onClick={() => onMcpAction(action, server.id)}
                                disabled={!!mcpActingId && mcpActingId !== `${action}:${server.id}`}
                              >
                                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{runtime?.running ? I.stop : I.play}</span>
                                {mcpActingId === `${action}:${server.id}` ? t("working") : runtime?.running ? t("stop") : t("start")}
                              </Button>
                              <Button type="button" variant="outline" size="xs" className={cn(cardActionDanger)} onClick={() => onDeleteMcpServer(server.id)}>
                                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.trash}</span>
                                {t("mcpDeleteServer")}
                              </Button>
                            </div>
                          </TableCell>
                        </TableRow>
                      )
                    })}
                  </TableBody>
                </Table>
              </ScrollArea>
            )}
          </CardContent>
        </Card>

        <div className="space-y-3">
          <Card className="py-0">
            <CardContent className="space-y-3 py-4">
              <div className="grid gap-3 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-xs font-semibold text-muted-foreground">{t("mcpServerId")}</label>
                  <Input
                    data-testid="mcp-input-id"
                    value={selectedMcpDraft?.id ?? ""}
                    onChange={(e) => onUpdateSelectedMcpDraft((draft) => ({ ...draft, id: e.target.value }))}
                    className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs font-mono"
                    disabled={!selectedMcpDraft}
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-semibold text-muted-foreground">{t("name")}</label>
                  <Input
                    data-testid="mcp-input-name"
                    value={selectedMcpDraft?.name ?? ""}
                    onChange={(e) => onUpdateSelectedMcpDraft((draft) => ({ ...draft, name: e.target.value }))}
                    className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs"
                    disabled={!selectedMcpDraft}
                  />
                </div>
                <div className="space-y-2 md:col-span-2">
                  <label className="text-xs font-semibold text-muted-foreground">{t("mcpCommand")}</label>
                  <Input
                    data-testid="mcp-input-command"
                    value={selectedMcpDraft?.command ?? ""}
                    onChange={(e) => onUpdateSelectedMcpDraft((draft) => ({ ...draft, command: e.target.value }))}
                    className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs font-mono"
                    disabled={!selectedMcpDraft}
                  />
                </div>
                <div className="space-y-2 md:col-span-2">
                  <label className="text-xs font-semibold text-muted-foreground">{t("mcpArgs")}</label>
                  <textarea
                    data-testid="mcp-input-args"
                    value={(selectedMcpDraft?.args ?? []).join("\n")}
                    onChange={(e) => onUpdateSelectedMcpDraft((draft) => ({
                      ...draft,
                      args: e.target.value.split("\n").map((item) => item.trim()).filter(Boolean),
                    }))}
                    placeholder={t("mcpArgsHint")}
                    className="min-h-[72px] w-full rounded-lg border border-border/60 bg-popover/40 px-2.5 py-2 text-xs text-foreground outline-hidden ring-ring/40 transition focus:ring-2"
                    disabled={!selectedMcpDraft}
                  />
                </div>
                <div className="space-y-2 md:col-span-2">
                  <label className="text-xs font-semibold text-muted-foreground">ENV</label>
                  <textarea
                    data-testid="mcp-input-env"
                    value={(selectedMcpDraft?.env ?? []).join("\n")}
                    onChange={(e) => onUpdateSelectedMcpDraft((draft) => ({
                      ...draft,
                      env: e.target.value.split("\n").map((item) => item.trim()).filter(Boolean),
                    }))}
                    placeholder={t("mcpEnvHint")}
                    className="min-h-[72px] w-full rounded-lg border border-border/60 bg-popover/40 px-2.5 py-2 text-xs text-foreground outline-hidden ring-ring/40 transition focus:ring-2"
                    disabled={!selectedMcpDraft}
                  />
                </div>
                <div className="space-y-2 md:col-span-2">
                  <label className="text-xs font-semibold text-muted-foreground">{t("mcpWorkingDir")}</label>
                  <Input
                    data-testid="mcp-input-working-dir"
                    value={selectedMcpDraft?.working_dir ?? ""}
                    onChange={(e) => onUpdateSelectedMcpDraft((draft) => ({ ...draft, working_dir: e.target.value }))}
                    className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs font-mono"
                    disabled={!selectedMcpDraft}
                  />
                </div>
                <div className="space-y-2 md:col-span-2">
                  <label className="text-xs font-semibold text-muted-foreground">{t("mcpNotes")}</label>
                  <textarea
                    data-testid="mcp-input-notes"
                    value={selectedMcpDraft?.notes ?? ""}
                    onChange={(e) => onUpdateSelectedMcpDraft((draft) => ({ ...draft, notes: e.target.value }))}
                    className="min-h-[72px] w-full rounded-lg border border-border/60 bg-popover/40 px-2.5 py-2 text-xs text-foreground outline-hidden ring-ring/40 transition focus:ring-2"
                    disabled={!selectedMcpDraft}
                  />
                </div>
              </div>
              {selectedMcpStatus && (
                <div data-testid="mcp-selected-status" className="rounded-lg border border-border/50 bg-muted/25 px-3 py-2 text-xs text-muted-foreground">
                  <div>{t("mcpStatus")}: <span className="text-foreground">{getMcpStatusLabel(selectedMcpStatus, t)}</span></div>
                  {selectedMcpStatus.pid && <div>PID: <span className="text-foreground">{selectedMcpStatus.pid}</span></div>}
                  {selectedMcpStatus.started_at && <div>{t("sandboxCreatedAt")}: <span className="text-foreground">{formatAiHubTime(selectedMcpStatus.started_at)}</span></div>}
                </div>
              )}
            </CardContent>
          </Card>

          <Card className="py-0">
            <CardContent className="space-y-3 py-4">
              <div className="flex flex-wrap items-center gap-2">
                <div className="text-sm font-semibold text-foreground">{t("mcpExportConfig")}</div>
                <Select value={mcpExportClient} onValueChange={onMcpExportClientChange}>
                  <SelectTrigger className="h-8 w-[160px] rounded-lg border-border/60 bg-popover/40 text-xs">
                    <SelectValue placeholder={t("mcpExportClient")} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="codex">Codex</SelectItem>
                    <SelectItem value="claude">Claude</SelectItem>
                    <SelectItem value="cursor">Cursor</SelectItem>
                  </SelectContent>
                </Select>
                <Button type="button" size="xs" className={cn(cardActionOutline)} onClick={() => onMcpAction("export")}>
                  {t("mcpExportConfig")}
                </Button>
                {mcpExportValue && (
                  <Button type="button" size="xs" className={cn(cardActionOutline)} onClick={() => navigator.clipboard.writeText(mcpExportValue)}>
                    <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.copy}</span>
                    {t("mcpCopyConfig")}
                  </Button>
                )}
              </div>
              <textarea
                value={mcpExportValue}
                readOnly
                className="min-h-[180px] w-full rounded-lg border border-border/60 bg-popover/40 px-2.5 py-2 text-xs text-foreground outline-hidden"
              />
            </CardContent>
          </Card>

          <Card className="py-0">
            <CardContent className="space-y-3 py-4">
              <div className="text-sm font-semibold text-foreground">{t("mcpLogs")}</div>
              <div data-testid="mcp-logs-output" className="min-h-[180px] whitespace-pre-wrap break-all rounded-lg border border-border/50 bg-muted/25 px-3 py-2 font-mono text-[11px] text-muted-foreground">
                {mcpLogs.length > 0 ? mcpLogs.join("\n") : "-"}
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </TabsContent>
  )
}
