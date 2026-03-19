import { I } from "../../icons"
import { EmptyState } from "../../components/EmptyState"
import { ErrorInline } from "../../components/ErrorDisplay"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { TabsContent } from "@/components/ui/tabs"
import { cardActionDanger, cardActionOutline, cardActionSecondary, iconStroke } from "@/lib/styles"
import { cn } from "@/lib/utils"
import type {
  SandboxAuditEventDto,
  SandboxExecResultDto,
  SandboxInfoDto,
  SandboxInspectDto,
  SandboxRuntimeUsageDto,
  SandboxTemplateDto,
} from "../../types"
import { formatAiHubTime, type TranslateFn } from "./utils"

interface AiHubSandboxesTabProps {
  t: TranslateFn
  sandboxTemplates: SandboxTemplateDto[]
  sandboxes: SandboxInfoDto[]
  sandboxAudit: SandboxAuditEventDto[]
  sandboxInspect: SandboxInspectDto | null
  sandboxRuntimeUsage: SandboxRuntimeUsageDto | null
  sandboxRuntimeLoading: boolean
  sandboxRuntimeError: string
  sandboxLoading: boolean
  sandboxCreating: boolean
  sandboxActingId: string
  sandboxError: string
  sandboxNotice: string
  sandboxInspectError: string
  sandboxSelectedTemplate: string
  sandboxName: string
  sandboxOwner: string
  sandboxCpu: number | ""
  sandboxMemoryMb: number | ""
  sandboxTtlHours: number | ""
  sandboxCommand: string
  sandboxEnvLines: string
  sandboxExecCommand: string
  sandboxExecResult: SandboxExecResultDto | null
  selectedSandboxTemplate: SandboxTemplateDto | null
  runningSandboxCount: number
  expiredSandboxCount: number
  onCleanupExpiredSandboxes: () => void
  onRefreshSandboxes: () => void
  onDismissSandboxError: () => void
  onSelectSandboxTemplate: (templateId: string) => void
  onSandboxNameChange: (value: string) => void
  onSandboxOwnerChange: (value: string) => void
  onSandboxCpuChange: (value: number | "") => void
  onSandboxMemoryMbChange: (value: number | "") => void
  onSandboxTtlHoursChange: (value: number | "") => void
  onSandboxCommandChange: (value: string) => void
  onSandboxEnvLinesChange: (value: string) => void
  onCreateSandbox: () => void
  onSandboxAction: (action: "start" | "stop" | "delete" | "inspect", item: SandboxInfoDto) => void
  onDismissSandboxInspectError: () => void
  onDismissSandboxRuntimeError: () => void
  onSandboxExecCommandChange: (value: string) => void
  onSandboxExec: () => void
  formatUsagePercent: (value: number | null | undefined) => string
  formatUsageMegabytes: (value: number | null | undefined) => string
}

export function AiHubSandboxesTab({
  t,
  sandboxTemplates,
  sandboxes,
  sandboxAudit,
  sandboxInspect,
  sandboxRuntimeUsage,
  sandboxRuntimeLoading,
  sandboxRuntimeError,
  sandboxLoading,
  sandboxCreating,
  sandboxActingId,
  sandboxError,
  sandboxNotice,
  sandboxInspectError,
  sandboxSelectedTemplate,
  sandboxName,
  sandboxOwner,
  sandboxCpu,
  sandboxMemoryMb,
  sandboxTtlHours,
  sandboxCommand,
  sandboxEnvLines,
  sandboxExecCommand,
  sandboxExecResult,
  selectedSandboxTemplate,
  runningSandboxCount,
  expiredSandboxCount,
  onCleanupExpiredSandboxes,
  onRefreshSandboxes,
  onDismissSandboxError,
  onSelectSandboxTemplate,
  onSandboxNameChange,
  onSandboxOwnerChange,
  onSandboxCpuChange,
  onSandboxMemoryMbChange,
  onSandboxTtlHoursChange,
  onSandboxCommandChange,
  onSandboxEnvLinesChange,
  onCreateSandbox,
  onSandboxAction,
  onDismissSandboxInspectError,
  onDismissSandboxRuntimeError,
  onSandboxExecCommandChange,
  onSandboxExec,
  formatUsagePercent,
  formatUsageMegabytes,
}: AiHubSandboxesTabProps) {
  return (
    <TabsContent value="sandboxes" className="space-y-3">
      <Card className="py-0">
        <CardContent className="space-y-3 py-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <div className={cn("text-sm font-semibold text-foreground")}>{t("aiSandboxesTitle")}</div>
                <Badge className="rounded-md border border-brand-green/20 bg-brand-green/10 px-1.5 py-0 text-[11px] text-brand-green">
                  {t("mvp")}
                </Badge>
                <Badge variant="secondary" className="rounded-md px-1.5 py-0 text-[11px]">
                  {runningSandboxCount} {t("running")}
                </Badge>
                <Badge variant="secondary" className="rounded-md px-1.5 py-0 text-[11px]">
                  {sandboxes.length} {t("sandboxInstances")}
                </Badge>
                {expiredSandboxCount > 0 && (
                  <Badge variant="secondary" className="rounded-md px-1.5 py-0 text-[11px]">
                    {expiredSandboxCount} {t("sandboxExpired")}
                  </Badge>
                )}
              </div>
              <div className="mt-1 text-xs text-muted-foreground">{t("aiSandboxesDesc")}</div>
            </div>
            <div className="flex items-center gap-2">
              <Button
                type="button"
                variant="outline"
                size="xs"
                className={cn(cardActionOutline)}
                disabled={sandboxActingId === "cleanup" || sandboxLoading || sandboxCreating}
                onClick={onCleanupExpiredSandboxes}
              >
                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.trash}</span>
                {sandboxActingId === "cleanup" ? t("working") : t("sandboxCleanupExpired")}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="xs"
                className={cn(cardActionOutline)}
                onClick={onRefreshSandboxes}
                disabled={sandboxLoading || sandboxCreating}
              >
                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.refresh}</span>
                {sandboxLoading ? t("working") : t("refresh")}
              </Button>
            </div>
          </div>

          {sandboxError && <ErrorInline message={sandboxError} onDismiss={onDismissSandboxError} />}
          {sandboxNotice && <div className="text-xs text-muted-foreground">{sandboxNotice}</div>}
        </CardContent>
      </Card>

      <Card className="py-0">
        <CardContent className="space-y-4 py-4">
          <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
            <div className="space-y-2">
              <label className="text-xs font-semibold text-muted-foreground">{t("sandboxTemplate")}</label>
              <Select value={sandboxSelectedTemplate} onValueChange={onSelectSandboxTemplate}>
                <SelectTrigger className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs">
                  <SelectValue placeholder={t("sandboxTemplate")} />
                </SelectTrigger>
                <SelectContent>
                  {sandboxTemplates.map((item) => (
                    <SelectItem key={item.id} value={item.id}>
                      {item.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <label className="text-xs font-semibold text-muted-foreground">{t("nameOptional")}</label>
              <Input
                value={sandboxName}
                onChange={(e) => onSandboxNameChange(e.target.value)}
                placeholder="cbx-node-dev-..."
                className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs"
              />
            </div>
            <div className="space-y-2">
              <label className="text-xs font-semibold text-muted-foreground">{t("sandboxOwner")}</label>
              <Input
                value={sandboxOwner}
                onChange={(e) => onSandboxOwnerChange(e.target.value)}
                placeholder={t("sandboxOwnerHint")}
                className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs"
              />
            </div>
            <div className="space-y-2">
              <label className="text-xs font-semibold text-muted-foreground">{t("sandboxCommand")}</label>
              <Input
                value={sandboxCommand}
                onChange={(e) => onSandboxCommandChange(e.target.value)}
                placeholder={selectedSandboxTemplate?.default_command ?? "sleep infinity"}
                className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs font-mono"
              />
            </div>
            <div className="space-y-2">
              <label className="text-xs font-semibold text-muted-foreground">{t("cpus")}</label>
              <Input
                type="number"
                min={1}
                max={16}
                value={sandboxCpu}
                onChange={(e) => {
                  const value = e.target.value
                  onSandboxCpuChange(value === "" ? "" : Number(value))
                }}
                className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs"
              />
            </div>
            <div className="space-y-2">
              <label className="text-xs font-semibold text-muted-foreground">{t("memoryMb")}</label>
              <Input
                type="number"
                min={256}
                max={65536}
                value={sandboxMemoryMb}
                onChange={(e) => {
                  const value = e.target.value
                  onSandboxMemoryMbChange(value === "" ? "" : Number(value))
                }}
                className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs"
              />
            </div>
            <div className="space-y-2">
              <label className="text-xs font-semibold text-muted-foreground">{t("sandboxTtlHours")}</label>
              <Input
                type="number"
                min={1}
                max={168}
                value={sandboxTtlHours}
                onChange={(e) => {
                  const value = e.target.value
                  onSandboxTtlHoursChange(value === "" ? "" : Number(value))
                }}
                className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs"
              />
            </div>
          </div>

          <div className="space-y-2">
            <label className="text-xs font-semibold text-muted-foreground">{t("sandboxEnvLines")}</label>
            <textarea
              value={sandboxEnvLines}
              onChange={(e) => onSandboxEnvLinesChange(e.target.value)}
              placeholder={t("sandboxEnvHint")}
              className="min-h-[72px] w-full rounded-lg border border-border/60 bg-popover/40 px-2.5 py-2 text-xs text-foreground outline-hidden ring-ring/40 transition focus:ring-2"
            />
          </div>

          {selectedSandboxTemplate && (
            <div className="rounded-lg border border-border/50 bg-muted/25 px-3 py-2 text-xs text-muted-foreground">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-medium text-foreground">{selectedSandboxTemplate.name}</span>
                <Badge variant="secondary" className="rounded-md px-1.5 py-0 text-[11px]">
                  {selectedSandboxTemplate.image}
                </Badge>
                <span>{selectedSandboxTemplate.description}</span>
              </div>
            </div>
          )}

          <div className="flex justify-end">
            <Button
              type="button"
              size="xs"
              className={cn(cardActionSecondary)}
              disabled={sandboxCreating || !sandboxSelectedTemplate}
              onClick={onCreateSandbox}
            >
              <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.plus}</span>
              {sandboxCreating ? t("working") : t("sandboxCreate")}
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card className="py-0">
        <CardContent className="py-0">
          <div className="border-b border-border/50 px-4 py-3 text-xs text-muted-foreground">{t("sandboxListDesc")}</div>
          {sandboxes.length === 0 ? (
            <div className="px-4 py-6">
              <EmptyState
                icon={I.server}
                title={t("sandboxListEmptyTitle")}
                description={t("sandboxListEmptyDesc")}
                code="bash scripts/setup-ai.sh --install"
              />
            </div>
          ) : (
            <ScrollArea className="max-h-[380px]">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("name")}</TableHead>
                    <TableHead>{t("sandboxTemplate")}</TableHead>
                    <TableHead>{t("status")}</TableHead>
                    <TableHead>{t("sandboxExpiresAt")}</TableHead>
                    <TableHead>{t("cpus")}</TableHead>
                    <TableHead>{t("memoryMb")}</TableHead>
                    <TableHead className="text-right">{t("actions")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {sandboxes.map((item) => (
                    <TableRow key={item.id}>
                      <TableCell className="max-w-[280px]">
                        <div className="truncate text-xs font-medium text-foreground">{item.name}</div>
                        <div className="truncate font-mono text-[11px] text-muted-foreground">{item.short_id}</div>
                      </TableCell>
                      <TableCell className="max-w-[220px] truncate text-xs text-muted-foreground">{item.template_id}</TableCell>
                      <TableCell>
                        {item.lifecycle_state === "running" ? (
                          <Badge className="gap-2 rounded-full border border-brand-green/20 bg-brand-green/10 px-3 py-1 text-[11px] font-medium text-brand-green">
                            <span className="size-1.5 rounded-full bg-brand-green shadow-[0_0_10px_hsl(var(--brand-green)/0.6)]" />
                            {t("running")}
                          </Badge>
                        ) : item.lifecycle_state === "creating" ? (
                          <Badge
                            variant="secondary"
                            className="gap-2 rounded-full border border-border/60 bg-popover/40 px-3 py-1 text-[11px] font-medium text-muted-foreground"
                          >
                            <span className="size-1.5 rounded-full bg-primary" />
                            {t("creating")}
                          </Badge>
                        ) : item.lifecycle_state === "expired" ? (
                          <Badge className="gap-2 rounded-full border border-destructive/30 bg-destructive/10 px-3 py-1 text-[11px] font-medium text-destructive">
                            <span className="size-1.5 rounded-full bg-destructive" />
                            {t("sandboxExpired")}
                          </Badge>
                        ) : (
                          <Badge
                            variant="secondary"
                            className="gap-2 rounded-full border border-border/60 bg-popover/40 px-3 py-1 text-[11px] font-medium text-muted-foreground"
                          >
                            <span className="size-1.5 rounded-full bg-destructive" />
                            {t("stopped")}
                          </Badge>
                        )}
                      </TableCell>
                      <TableCell className="max-w-[220px] truncate text-xs text-muted-foreground">
                        {formatAiHubTime(item.expires_at)}
                        {item.is_expired && <span className="ml-1 text-destructive">{t("sandboxExpired")}</span>}
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground">{item.cpu_cores}</TableCell>
                      <TableCell className="text-xs text-muted-foreground">{item.memory_mb}</TableCell>
                      <TableCell className="text-right">
                        <div className="inline-flex items-center gap-1">
                          <Button
                            type="button"
                            variant="outline"
                            size="xs"
                            className={cn(cardActionOutline)}
                            disabled={!!sandboxActingId}
                            onClick={() => onSandboxAction("inspect", item)}
                          >
                            {t("inspect")}
                          </Button>
                          {item.lifecycle_state === "running" ? (
                            <Button
                              type="button"
                              variant="outline"
                              size="xs"
                              className={cn(cardActionOutline)}
                              disabled={!!sandboxActingId}
                              onClick={() => onSandboxAction("stop", item)}
                            >
                              {sandboxActingId === `stop:${item.id}` ? t("working") : t("stop")}
                            </Button>
                          ) : (
                            <Button
                              type="button"
                              variant="outline"
                              size="xs"
                              className={cn(cardActionOutline)}
                              disabled={!!sandboxActingId}
                              onClick={() => onSandboxAction("start", item)}
                            >
                              {sandboxActingId === `start:${item.id}` ? t("working") : t("start")}
                            </Button>
                          )}
                          <Button
                            type="button"
                            variant="outline"
                            size="xs"
                            className={cn(cardActionDanger)}
                            disabled={!!sandboxActingId}
                            onClick={() => onSandboxAction("delete", item)}
                          >
                            {sandboxActingId === `delete:${item.id}` ? t("working") : t("delete")}
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </ScrollArea>
          )}
        </CardContent>
      </Card>

      {(sandboxInspect || sandboxInspectError) && (
        <Card className="py-0">
          <CardContent className="space-y-3 py-4">
            <div className="flex flex-wrap items-center gap-2">
              <div className="text-sm font-semibold text-foreground">{t("sandboxInspectTitle")}</div>
              {sandboxInspect?.running ? (
                <Badge className="rounded-md border border-brand-green/20 bg-brand-green/10 px-1.5 py-0 text-[11px] text-brand-green">
                  {t("running")}
                </Badge>
              ) : (
                <Badge variant="secondary" className="rounded-md px-1.5 py-0 text-[11px]">
                  {t("stopped")}
                </Badge>
              )}
            </div>
            {sandboxInspectError && <ErrorInline message={sandboxInspectError} onDismiss={onDismissSandboxInspectError} />}
            {sandboxInspect && (
              <div className="grid grid-cols-1 gap-2 text-xs text-muted-foreground lg:grid-cols-2">
                <div>{t("name")}: <span className="text-foreground">{sandboxInspect.name}</span></div>
                <div>ID: <span className="font-mono text-foreground">{sandboxInspect.short_id}</span></div>
                <div>{t("image")}: <span className="font-mono text-foreground">{sandboxInspect.image}</span></div>
                <div>{t("sandboxTemplate")}: <span className="text-foreground">{sandboxInspect.template_id}</span></div>
                <div>{t("sandboxOwner")}: <span className="text-foreground">{sandboxInspect.owner}</span></div>
                <div>{t("sandboxTtlHours")}: <span className="text-foreground">{sandboxInspect.ttl_hours}</span></div>
                <div>{t("sandboxCreatedAt")}: <span className="text-foreground">{formatAiHubTime(sandboxInspect.created_at)}</span></div>
                <div>{t("sandboxExpiresAt")}: <span className="text-foreground">{formatAiHubTime(sandboxInspect.expires_at)}</span></div>
                <div>{t("cpus")}: <span className="text-foreground">{sandboxInspect.cpu_cores}</span></div>
                <div>{t("memoryMb")}: <span className="text-foreground">{sandboxInspect.memory_mb}</span></div>
              </div>
            )}
            {sandboxInspect && (
              <div data-testid="sandbox-runtime-card" className="rounded-lg border border-border/50 bg-muted/25 px-3 py-2 text-xs">
                <div className="mb-2 text-muted-foreground">{t("sandboxRuntimeUsage")}</div>
                {sandboxRuntimeLoading ? (
                  <div className="text-muted-foreground">{t("working")}</div>
                ) : sandboxRuntimeError ? (
                  <ErrorInline message={sandboxRuntimeError} onDismiss={onDismissSandboxRuntimeError} />
                ) : sandboxRuntimeUsage ? (
                  <>
                    <div className="grid grid-cols-1 gap-2 text-muted-foreground lg:grid-cols-2">
                      <div>{t("cpuUsage")}: <span className="text-foreground">{formatUsagePercent(sandboxRuntimeUsage.cpu_percent)}</span></div>
                      <div>
                        {t("memoryUsage")}: <span className="text-foreground">{formatUsageMegabytes(sandboxRuntimeUsage.memory_usage_mb)} / {formatUsageMegabytes(sandboxRuntimeUsage.memory_limit_mb)} ({formatUsagePercent(sandboxRuntimeUsage.memory_percent)})</span>
                      </div>
                      <div>{t("gpuMemory")}: <span className="text-foreground">{sandboxRuntimeUsage.gpu_processes.length > 0 ? sandboxRuntimeUsage.gpu_memory_used_human : "-"}</span></div>
                      <div>{t("gpuProcesses")}: <span className="text-foreground">{sandboxRuntimeUsage.gpu_processes.length}</span></div>
                    </div>
                    <div className="mt-2 text-muted-foreground">
                      {sandboxRuntimeUsage.running
                        ? sandboxRuntimeUsage.gpu_attribution_supported
                          ? sandboxRuntimeUsage.gpu_processes.length > 0
                            ? `${sandboxRuntimeUsage.gpu_processes.length} ${t("gpuProcesses").toLowerCase()} · ${new Set(sandboxRuntimeUsage.gpu_processes.map((process) => process.gpu_index)).size} ${t("gpuDevices").toLowerCase()}`
                            : t("sandboxGpuIdle")
                          : sandboxRuntimeUsage.gpu_message || t("gpuTelemetryUnavailable")
                        : t("stopped")}
                    </div>
                    {sandboxRuntimeUsage.gpu_processes.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-2">
                        {sandboxRuntimeUsage.gpu_processes.map((process) => (
                          <Badge
                            key={`${process.pid}-${process.gpu_index}-${process.process_name}`}
                            variant="secondary"
                            className="rounded-md px-1.5 py-0 text-[11px]"
                          >
                            {process.process_name} · GPU {process.gpu_index}
                            {process.memory_used_human ? ` · ${process.memory_used_human}` : ""}
                          </Badge>
                        ))}
                      </div>
                    )}
                  </>
                ) : (
                  <div className="text-muted-foreground">-</div>
                )}
              </div>
            )}
            {sandboxInspect?.command && (
              <div className="rounded-lg border border-border/50 bg-muted/25 px-3 py-2 text-xs">
                <div className="mb-1 text-muted-foreground">{t("sandboxCommand")}</div>
                <code className="font-mono text-foreground">{sandboxInspect.command}</code>
              </div>
            )}
            {sandboxInspect?.env && sandboxInspect.env.length > 0 && (
              <div className="rounded-lg border border-border/50 bg-muted/25 px-3 py-2 text-xs">
                <div className="mb-1 text-muted-foreground">ENV</div>
                <div className="break-all font-mono text-foreground">{sandboxInspect.env.join(" · ")}</div>
              </div>
            )}
          </CardContent>
        </Card>
      )}

      <Card className="py-0">
        <CardContent className="space-y-3 py-4">
          <div className="text-sm font-semibold text-foreground">{t("sandboxExecTitle")}</div>
          <div className="flex flex-col gap-2 lg:flex-row">
            <Input
              value={sandboxExecCommand}
              onChange={(e) => onSandboxExecCommandChange(e.target.value)}
              placeholder={t("sandboxExecPlaceholder")}
              className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs font-mono"
            />
            <Button
              type="button"
              size="xs"
              className={cn(cardActionSecondary)}
              onClick={onSandboxExec}
              disabled={!sandboxExecCommand.trim() || !!sandboxActingId}
            >
              <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.terminal}</span>
              {sandboxActingId.startsWith("exec:") ? t("working") : t("sandboxExecRun")}
            </Button>
          </div>
          {sandboxExecResult && (
            <div className="space-y-2 rounded-lg border border-border/50 bg-muted/25 px-3 py-2 text-xs">
              <div className="flex flex-wrap items-center gap-2 text-muted-foreground">
                <span>{t("sandboxExecExitCode")}: {sandboxExecResult.exit_code ?? "-"}</span>
                <Badge variant={sandboxExecResult.ok ? "secondary" : "outline"}>
                  {sandboxExecResult.ok ? t("running") : t("error")}
                </Badge>
              </div>
              {!!sandboxExecResult.stdout && (
                <div>
                  <div className="mb-1 text-muted-foreground">{t("sandboxExecStdout")}</div>
                  <pre className="whitespace-pre-wrap break-all font-mono text-foreground">{sandboxExecResult.stdout}</pre>
                </div>
              )}
              {!!sandboxExecResult.stderr && (
                <div>
                  <div className="mb-1 text-muted-foreground">{t("sandboxExecStderr")}</div>
                  <pre className="whitespace-pre-wrap break-all font-mono text-foreground">{sandboxExecResult.stderr}</pre>
                </div>
              )}
              {!sandboxExecResult.stdout && !sandboxExecResult.stderr && !!sandboxExecResult.output && (
                <div>
                  <div className="mb-1 text-muted-foreground">{t("sandboxExecOutput")}</div>
                  <pre className="whitespace-pre-wrap break-all font-mono text-foreground">{sandboxExecResult.output}</pre>
                </div>
              )}
            </div>
          )}
        </CardContent>
      </Card>

      <Card className="py-0">
        <CardContent className="py-0">
          <div className="border-b border-border/50 px-4 py-3 text-xs text-muted-foreground">{t("sandboxAuditDesc")}</div>
          {sandboxAudit.length === 0 ? (
            <div className="px-4 py-6">
              <EmptyState
                icon={I.fileText}
                title={t("sandboxAuditEmptyTitle")}
                description={t("sandboxAuditEmptyDesc")}
              />
            </div>
          ) : (
            <ScrollArea className="max-h-[260px]">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("modifiedAt")}</TableHead>
                    <TableHead>{t("action")}</TableHead>
                    <TableHead>{t("name")}</TableHead>
                    <TableHead>{t("description")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {sandboxAudit.map((event, index) => (
                    <TableRow key={`${event.timestamp}-${event.action}-${event.sandbox_id}-${index}`}>
                      <TableCell className="max-w-[220px] truncate text-xs text-muted-foreground">{formatAiHubTime(event.timestamp)}</TableCell>
                      <TableCell className="text-xs text-foreground">{event.action}</TableCell>
                      <TableCell className="max-w-[180px] truncate text-xs text-muted-foreground">
                        {event.sandbox_name || event.sandbox_id}
                      </TableCell>
                      <TableCell className="max-w-[480px] truncate text-xs text-muted-foreground">{event.detail}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </ScrollArea>
          )}
        </CardContent>
      </Card>
    </TabsContent>
  )
}
