import { useState } from "react";
import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import { useAppStore } from "@/stores/appStore";
import { SlidePanel } from "@/components/common/SlidePanel";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { ContainerLogs } from "./ContainerLogs";
import { ContainerMonitoring } from "./ContainerMonitoring";
import { TerminalView } from "./TerminalView";
import { Play, Square, Trash2, Copy, Check } from "lucide-react";

/**
 * Container detail slide panel — opens from the right side.
 * Shows: overview info, ports, logs, terminal.
 * Matches the reference project's SlidePanel pattern.
 */
export function ContainerDetail() {
  const selectedContainerId = useContainerStore((s) => s.selectedContainerId);
  const containers = useContainerStore((s) => s.containers);
  const selectContainer = useContainerStore((s) => s.selectContainer);

  const container = containers.find((c) => c.id === selectedContainerId) ?? null;

  return (
    <SlidePanel
      isOpen={container !== null}
      onClose={() => selectContainer(null)}
      title={container?.name}
    >
      {container !== null && <DetailContent container={container} />}
    </SlidePanel>
  );
}

function DetailContent({ container }: { container: ContainerInfo }) {
  const { t } = useI18n();
  const { startContainer, stopContainer, deleteContainer } = useContainerStore();
  const { addNotification } = useAppStore();
  const isRunning = container.status === "running" || container.status === "paused";
  const [operating, setOperating] = useState(false);

  const handleStart = async () => {
    setOperating(true);
    try {
      await startContainer(container.id);
      addNotification({
        type: "success",
        title: "容器已启动",
        message: `${container.name} 启动成功`,
        dismissable: true,
      });
    } catch (error) {
      addNotification({
        type: "error",
        title: "启动失败",
        message: error instanceof Error ? error.message : "未知错误",
        dismissable: true,
      });
    } finally {
      setOperating(false);
    }
  };

  const handleStop = async () => {
    setOperating(true);
    try {
      await stopContainer(container.id);
      addNotification({
        type: "success",
        title: "容器已停止",
        message: `${container.name} 停止成功`,
        dismissable: true,
      });
    } catch (error) {
      addNotification({
        type: "error",
        title: "停止失败",
        message: error instanceof Error ? error.message : "未知错误",
        dismissable: true,
      });
    } finally {
      setOperating(false);
    }
  };

  const handleDelete = async () => {
    const confirmed = confirm(`确定要删除容器 "${container.name}" 吗？`);
    if (!confirmed) return;

    setOperating(true);
    try {
      await deleteContainer(container.id);
      addNotification({
        type: "success",
        title: "容器已删除",
        message: `${container.name} 已删除`,
        dismissable: true,
      });
    } catch (error) {
      addNotification({
        type: "error",
        title: "删除失败",
        message: error instanceof Error ? error.message : "未知错误",
        dismissable: true,
      });
    } finally {
      setOperating(false);
    }
  }

  return (
    <div className="flex flex-col gap-6 p-5">
      {/* Action buttons bar */}
      <ActionButtonBar
        isRunning={isRunning}
        operating={operating}
        onStart={handleStart}
        onStop={handleStop}
        onDelete={handleDelete}
      />

      {/* Status badge */}
      <StatusBadge status={container.status} />

      {/* Overview section */}
      <section>
        <SectionTitle>{t("containers", "overview") ?? "概览"}</SectionTitle>
        <div className="flex flex-col gap-4">
          <CopyableField label="ID" value={container.shortId} />
          <DetailField label={t("containers", "image")}>{container.image}</DetailField>
          <DetailField label={t("containers", "created")}>
            {formatRelativeTime(container.createdAt)}
          </DetailField>
          <DetailField label={t("containers", "template")}>{container.labels?.["com.cratebay.template_id"] || "—"}</DetailField>
        </div>
      </section>

      {/* Specs section (limits) */}
      <section>
        <SectionTitle>规格</SectionTitle>
        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-md border border-border bg-muted/30 p-3">
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground mb-1">
              {t("containers", "cpuCores")}
            </div>
            <div className="text-lg font-semibold text-foreground">
              {container.cpuCores !== undefined ? `${container.cpuCores}` : "—"}
            </div>
            <div className="text-[10px] text-muted-foreground">核心</div>
          </div>
          <div className="rounded-md border border-border bg-muted/30 p-3">
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground mb-1">
              {t("containers", "memoryMb")}
            </div>
            <div className="text-lg font-semibold text-foreground">
              {container.memoryMb !== undefined ? `${container.memoryMb}` : "—"}
            </div>
            <div className="text-[10px] text-muted-foreground">MB</div>
          </div>
        </div>
      </section>

      {/* Monitoring section (usage) */}
      <section>
        <SectionTitle>Monitoring</SectionTitle>
        <ContainerMonitoring
          containerId={container.id}
          cpuCores={container.cpuCores}
          memoryMb={container.memoryMb}
          enabled={isRunning}
        />
      </section>

      {/* Ports section */}
      {container.ports.length > 0 && (
        <section>
          <SectionTitle>{t("containers", "ports") ?? "端口映射"}</SectionTitle>
          <div className="space-y-2">
            {container.ports.map((port) => (
              <div
                key={`${port.hostPort}-${port.containerPort}`}
                className="flex items-center justify-between rounded-md border border-border bg-muted/30 px-3 py-2"
              >
                <div className="flex items-center gap-2">
                  <span className="font-mono text-sm font-medium text-foreground">
                    {port.hostPort}:{port.containerPort}
                  </span>
                  <span className={cn(
                    "inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase",
                    port.protocol === "tcp"
                      ? "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-200"
                      : "bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-200"
                  )}>
                    {port.protocol}
                  </span>
                </div>
                <button
                  onClick={() => {
                    navigator.clipboard.writeText(`${port.hostPort}:${port.containerPort}`);
                  }}
                  className="flex h-6 w-6 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                  title="复制端口"
                >
                  <Copy className="h-3.5 w-3.5" />
                </button>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Logs section */}
      <section>
        <SectionTitle>{t("containers", "logs") ?? "日志"}</SectionTitle>
        <ContainerLogs containerId={container.id} />
      </section>

      {/* Terminal section */}
      <section>
        <SectionTitle>{t("containers", "terminal") ?? "终端"}</SectionTitle>
        {isRunning ? (
          <TerminalView containerId={container.id} />
        ) : (
          <div className="text-sm text-muted-foreground">Container is not running.</div>
        )}
      </section>
    </div>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
      {children}
    </h3>
  );
}

function DetailField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-[10px] uppercase tracking-wider text-muted-foreground">{label}</span>
      <span className="truncate text-sm font-medium text-foreground">{children}</span>
    </div>
  );
}

function StatusBadge({ status }: { status: ContainerInfo["status"] }) {
  const { t } = useI18n();
  type DisplayStatus = "running" | "stopped" | "creating" | "error";

  const displayStatus: DisplayStatus = (() => {
    switch (status) {
      case "running":
      case "paused":
        return "running";
      case "creating":
      case "restarting":
      case "removing":
        return "creating";
      case "dead":
        return "error";
      case "stopped":
      case "created":
      case "exited":
        return "stopped";
      default:
        return "error";
    }
  })();

  const variants: Record<DisplayStatus, { labelKey: DisplayStatus; dotClass: string; badgeClass: string }> = {
    running: {
      labelKey: "running",
      dotClass: "bg-emerald-400",
      badgeClass: "border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
    },
    stopped: {
      labelKey: "stopped",
      dotClass: "bg-zinc-400",
      badgeClass: "border-zinc-400/30 bg-zinc-400/10 text-zinc-500",
    },
    creating: {
      labelKey: "creating",
      dotClass: "bg-yellow-400 animate-pulse",
      badgeClass: "border-yellow-500/30 bg-yellow-500/10 text-yellow-600 dark:text-yellow-400",
    },
    error: {
      labelKey: "error",
      dotClass: "bg-red-400",
      badgeClass: "border-red-500/30 bg-red-500/10 text-red-600 dark:text-red-400",
    },
  };

  const variant = variants[displayStatus];

  return (
    <Badge
      variant="outline"
      className={cn(
        "inline-flex w-fit items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium",
        variant.badgeClass,
      )}
    >
      <span className={cn("inline-block h-2 w-2 rounded-full", variant.dotClass)} />
      {t("containers", variant.labelKey)}
    </Badge>
  );
}

function formatRelativeTime(isoString: string): string {
  try {
    const date = new Date(isoString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    const diffHour = Math.floor(diffMs / 3600000);
    const diffDay = Math.floor(diffMs / 86400000);

    if (diffMin < 1) return "刚刚";
    if (diffMin < 60) return `${diffMin} 分钟前`;
    if (diffHour < 24) return `${diffHour} 小时前`;
    if (diffDay < 7) return `${diffDay} 天前`;
    return date.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
  } catch {
    return "—";
  }
}

function ActionButtonBar({
  isRunning,
  operating,
  onStart,
  onStop,
  onDelete,
}: {
  isRunning: boolean;
  operating: boolean;
  onStart: () => Promise<void>;
  onStop: () => Promise<void>;
  onDelete: () => Promise<void>;
}) {
  return (
    <div className="flex flex-wrap gap-2 rounded-lg border border-border bg-muted/30 p-3">
      {isRunning ? (
        <Button
          size="sm"
          variant="outline"
          onClick={onStop}
          disabled={operating}
          className="flex items-center gap-2"
        >
          <Square className="h-3.5 w-3.5" />
          停止
        </Button>
      ) : (
        <Button
          size="sm"
          variant="outline"
          onClick={onStart}
          disabled={operating}
          className="flex items-center gap-2"
        >
          <Play className="h-3.5 w-3.5" />
          启动
        </Button>
      )}

      <Button
        size="sm"
        variant="outline"
        onClick={onDelete}
        disabled={operating}
        className="flex items-center gap-2 text-destructive hover:text-destructive"
      >
        <Trash2 className="h-3.5 w-3.5" />
        删除
      </Button>
    </div>
  );
}

function CopyableField({
  label,
  value,
  onCopySuccess,
}: {
  label: string;
  value: string | React.ReactNode;
  onCopySuccess?: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const isString = typeof value === "string";

  const handleCopy = async () => {
    if (!isString) return;
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      onCopySuccess?.();
      setTimeout(() => setCopied(false), 2000);
    } catch {
      console.error("Failed to copy");
    }
  };

  return (
    <div className="flex items-start justify-between gap-2">
      <div className="flex flex-col gap-0.5 flex-1">
        <span className="text-[10px] uppercase tracking-wider text-muted-foreground">{label}</span>
        <span className="truncate font-mono text-sm text-foreground">{value}</span>
      </div>
      {isString && (
        <button
          onClick={handleCopy}
          className="mt-5 flex h-6 w-6 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          title="复制"
        >
          {copied ? (
            <Check className="h-3.5 w-3.5 text-emerald-500" />
          ) : (
            <Copy className="h-3.5 w-3.5" />
          )}
        </button>
      )}
    </div>
  );
}
