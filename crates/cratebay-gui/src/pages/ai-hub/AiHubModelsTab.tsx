import { I } from "../../icons"
import { EmptyState } from "../../components/EmptyState"
import { ErrorInline } from "../../components/ErrorDisplay"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { TabsContent } from "@/components/ui/tabs"
import { cardActionDanger, cardActionOutline, cardActionSecondary, iconStroke } from "@/lib/styles"
import { cn } from "@/lib/utils"
import type {
  GpuStatusDto,
  OllamaModelDto,
  OllamaStatusDto,
  OllamaStorageInfoDto,
} from "../../types"
import type { TranslateFn } from "./utils"

interface AiHubModelsTabProps {
  t: TranslateFn
  ollamaStatus: OllamaStatusDto | null
  ollamaModels: OllamaModelDto[]
  ollamaStorage: OllamaStorageInfoDto | null
  gpuStatus: GpuStatusDto | null
  ollamaLoading: boolean
  ollamaError: string
  ollamaNotice: string
  ollamaPullName: string
  ollamaActingName: string
  totalModelBytes: number
  onRefreshOllama: () => void
  onDismissOllamaError: () => void
  onPullNameChange: (value: string) => void
  onPullModel: () => void
  onDeleteModel: (name: string) => void
  formatGpuMetric: (value: number | null | undefined, suffix: string) => string
  formatGpuMemory: (used?: string | null, total?: string | null) => string
}

export function AiHubModelsTab({
  t,
  ollamaStatus,
  ollamaModels,
  ollamaStorage,
  gpuStatus,
  ollamaLoading,
  ollamaError,
  ollamaNotice,
  ollamaPullName,
  ollamaActingName,
  totalModelBytes,
  onRefreshOllama,
  onDismissOllamaError,
  onPullNameChange,
  onPullModel,
  onDeleteModel,
  formatGpuMetric,
  formatGpuMemory,
}: AiHubModelsTabProps) {
  return (
    <TabsContent value="models" className="space-y-3">
      <Card className="py-0">
        <CardContent className="space-y-3 py-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <div className={cn("text-sm font-semibold text-foreground")}>{t("ollamaRuntime")}</div>
                {ollamaStatus?.running ? (
                  <Badge className="gap-2 rounded-full border border-brand-green/20 bg-brand-green/10 px-3 py-1 text-xs font-medium text-brand-green">
                    <span className="size-1.5 rounded-full bg-brand-green shadow-[0_0_10px_hsl(var(--brand-green)/0.6)]" />
                    {t("running")}
                  </Badge>
                ) : (
                  <Badge
                    variant="secondary"
                    className="gap-2 rounded-full border border-border/60 bg-popover/40 px-3 py-1 text-xs font-medium text-muted-foreground"
                  >
                    <span className="size-1.5 rounded-full bg-destructive" />
                    {t("stopped")}
                  </Badge>
                )}
                {ollamaStatus?.version && (
                  <Badge variant="secondary" className="rounded-md px-1.5 py-0 text-[11px]">
                    v{ollamaStatus.version}
                  </Badge>
                )}
              </div>
              <div className="mt-1 text-xs text-muted-foreground">
                {t("aiBaseUrl")}: {" "}
                <code className="rounded-md border border-border/60 bg-muted/60 px-1.5 py-0.5 font-mono text-[11px] text-foreground">
                  {ollamaStatus?.base_url ?? "-"}
                </code>
              </div>
            </div>

            <div className="flex items-center gap-2">
              <Button
                type="button"
                variant="outline"
                size="xs"
                className={cn(cardActionOutline)}
                onClick={onRefreshOllama}
                disabled={ollamaLoading}
              >
                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.refresh}</span>
                {ollamaLoading ? t("working") : t("refresh")}
              </Button>
            </div>
          </div>

          {ollamaError && <ErrorInline message={ollamaError} onDismiss={onDismissOllamaError} />}
          {ollamaNotice && <div className="text-xs text-muted-foreground">{ollamaNotice}</div>}

          <div className="grid gap-3 lg:grid-cols-3">
            <div className="rounded-xl border border-border/50 bg-muted/20 p-3">
              <div className="text-xs font-semibold text-muted-foreground">{t("aiModelStorage")}</div>
              <div className="mt-2 text-sm font-semibold text-foreground">
                {ollamaStorage?.total_size_human ?? `${Math.round((totalModelBytes / (1024 * 1024 * 1024)) * 10) / 10} GB`}
              </div>
              <div className="mt-1 text-xs text-muted-foreground">
                {(ollamaStorage?.model_count ?? ollamaModels.length)} {t("models").toLowerCase()}
              </div>
              <div className="mt-3 text-xs text-muted-foreground">{t("ollamaStoragePath")}</div>
              <code className="mt-1 block break-all rounded-lg border border-border/50 bg-popover/40 px-2 py-2 text-[11px] text-foreground">
                {ollamaStorage?.path || t("ollamaStorageMissing")}
              </code>
            </div>

            <div data-testid="ollama-gpu-card" className="rounded-xl border border-border/50 bg-muted/20 p-3">
              <div className="flex items-center gap-2">
                <div className="text-xs font-semibold text-muted-foreground">{t("gpuRuntime")}</div>
                {gpuStatus?.backend && (
                  <Badge variant="secondary" className="rounded-md px-1.5 py-0 text-[11px]">
                    {gpuStatus.backend}
                  </Badge>
                )}
              </div>
              <div className="mt-2 text-sm font-semibold text-foreground">
                {gpuStatus?.available ? `${gpuStatus.devices.length} ${t("gpuDevices").toLowerCase()}` : t("gpuTelemetryUnavailable")}
              </div>
              <div className="mt-1 text-xs text-muted-foreground">
                {gpuStatus?.message || t("gpuTelemetryUnavailable")}
              </div>
              {gpuStatus?.devices.length ? (
                <div className="mt-3 space-y-2">
                  {gpuStatus.devices.map((device) => (
                    <div
                      key={`${device.index}-${device.name}`}
                      data-testid={`ollama-gpu-device-${device.index}`}
                      className="rounded-lg border border-border/50 bg-popover/40 px-2.5 py-2"
                    >
                      <div className="truncate text-xs font-semibold text-foreground">{device.name}</div>
                      <div className="mt-1 grid grid-cols-2 gap-2 text-[11px] text-muted-foreground">
                        <div>
                          {t("gpuUtilization")}: <span className="text-foreground">{formatGpuMetric(device.utilization_percent, "%")}</span>
                        </div>
                        <div>
                          {t("gpuMemory")}: <span className="text-foreground">{formatGpuMemory(device.memory_used_human, device.memory_total_human)}</span>
                        </div>
                        <div>
                          {t("gpuTemperature")}: <span className="text-foreground">{formatGpuMetric(device.temperature_celsius, "°C")}</span>
                        </div>
                        <div>
                          {t("gpuPower")}: <span className="text-foreground">{formatGpuMetric(device.power_watts, "W")}</span>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              ) : null}
            </div>

            <div className="space-y-2 rounded-xl border border-border/50 bg-muted/20 p-3">
              <div className="text-xs font-semibold text-muted-foreground">{t("ollamaPullLabel")}</div>
              <Input
                value={ollamaPullName}
                onChange={(e) => onPullNameChange(e.target.value)}
                placeholder={t("ollamaPullPlaceholder")}
                className="h-8 rounded-lg border-border/60 bg-popover/40 text-xs font-mono"
              />
              <Button
                type="button"
                size="xs"
                className={cn(cardActionSecondary)}
                disabled={!ollamaPullName.trim() || !!ollamaActingName}
                onClick={onPullModel}
              >
                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.play}</span>
                {ollamaActingName.startsWith("pull:") ? t("working") : t("ollamaPullAction")}
              </Button>
            </div>
          </div>

          {ollamaStatus?.installed === false && (
            <EmptyState
              icon={I.layers}
              title={t("ollamaNotInstalledTitle")}
              description={t("ollamaInstallHint")}
              code="bash scripts/setup-ai.sh --install"
            />
          )}

          {ollamaStatus?.installed && !ollamaStatus.running && (
            <EmptyState
              icon={I.layers}
              title={t("ollamaNotRunningTitle")}
              description={t("ollamaStartHint")}
              code="ollama serve"
            />
          )}
        </CardContent>
      </Card>

      {ollamaStatus?.running && (
        <Card className="py-0">
          <CardContent className="py-0">
            {ollamaModels.length === 0 ? (
              <div className="px-4 py-6">
                <EmptyState
                  icon={I.layers}
                  title={t("ollamaModelsEmptyTitle")}
                  description={t("ollamaModelsEmptyDesc")}
                  code="ollama pull qwen2.5:7b"
                />
              </div>
            ) : (
              <ScrollArea className="max-h-[520px]">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>{t("name")}</TableHead>
                      <TableHead>{t("size")}</TableHead>
                      <TableHead>{t("description")}</TableHead>
                      <TableHead>{t("modifiedAt")}</TableHead>
                      <TableHead className="text-right">{t("actions")}</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {ollamaModels.map((model) => {
                      const details = [model.family, model.parameter_size, model.quantization_level]
                        .filter(Boolean)
                        .join(" · ")
                      const actingDelete = ollamaActingName === `delete:${model.name}`
                      return (
                        <TableRow key={model.name}>
                          <TableCell className="max-w-[420px] truncate font-mono text-xs">{model.name}</TableCell>
                          <TableCell className="text-xs text-muted-foreground">{model.size_human}</TableCell>
                          <TableCell className="max-w-[420px] truncate text-xs text-muted-foreground">{details || "-"}</TableCell>
                          <TableCell className="max-w-[260px] truncate text-xs text-muted-foreground">{model.modified_at || "-"}</TableCell>
                          <TableCell className="text-right">
                            <div className="flex justify-end gap-2">
                              <Button
                                type="button"
                                variant="outline"
                                size="xs"
                                className={cn(cardActionOutline)}
                                onClick={() => navigator.clipboard.writeText(`ollama run ${model.name}`)}
                              >
                                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.copy}</span>
                                {t("copy")}
                              </Button>
                              <Button
                                type="button"
                                variant="outline"
                                size="xs"
                                className={cn(cardActionDanger)}
                                disabled={!!ollamaActingName}
                                onClick={() => onDeleteModel(model.name)}
                              >
                                <span className={cn("mr-1", iconStroke, "[&_svg]:size-3")}>{I.trash}</span>
                                {actingDelete ? t("working") : t("ollamaDeleteAction")}
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
      )}
    </TabsContent>
  )
}
