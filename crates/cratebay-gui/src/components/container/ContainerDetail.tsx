import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import { SlidePanel } from "@/components/common/SlidePanel";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

/**
 * Container detail slide panel — opens from the right side.
 * Shows: overview info, ports, mock logs, mock terminal.
 * Matches the reference project's SlidePanel pattern.
 */
export function ContainerDetail() {
  const { t } = useI18n();
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
  const isRunning = container.status === "running";

  return (
    <div className="flex flex-col gap-6 p-5">
      {/* Status badge */}
      <StatusBadge status={container.status} />

      {/* Overview section */}
      <section>
        <SectionTitle>{t("containers", "overview") ?? "概览"}</SectionTitle>
        <div className="grid grid-cols-2 gap-x-6 gap-y-3">
          <DetailField label="ID">{container.shortId}</DetailField>
          <DetailField label={t("containers", "image")}>{container.image}</DetailField>
          <DetailField label={t("containers", "created")}>
            {formatRelativeTime(container.createdAt)}
          </DetailField>
          <DetailField label={t("containers", "template")}>{container.labels?.["com.cratebay.template_id"] || "—"}</DetailField>
          {isRunning && (
            <>
              <DetailField label={t("containers", "cpu")}>{container.cpuCores} cores</DetailField>
              <DetailField label={t("containers", "memory")}>{container.memoryMb} MB</DetailField>
            </>
          )}
        </div>
      </section>

      {/* Ports section */}
      {container.ports.length > 0 && (
        <section>
          <SectionTitle>{t("containers", "ports") ?? "端口映射"}</SectionTitle>
          <div className="flex flex-wrap gap-2">
            {container.ports.map((port) => (
              <span
                key={`${port.hostPort}-${port.containerPort}`}
                className="inline-flex items-center rounded-md border border-border bg-muted/50 px-2 py-1 font-mono text-xs text-foreground"
              >
                {port.hostPort}:{port.containerPort}/{port.protocol}
              </span>
            ))}
          </div>
        </section>
      )}

      {/* Logs section */}
      <section>
        <SectionTitle>{t("containers", "logs") ?? "日志"}</SectionTitle>
        <LogViewer containerId={container.id} isRunning={isRunning} />
      </section>

      {/* Terminal section */}
      {isRunning && (
        <section>
          <SectionTitle>{t("containers", "terminal") ?? "终端"}</SectionTitle>
          <TerminalPreview />
        </section>
      )}
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
  const variants: Record<typeof status, { labelKey: "running" | "stopped" | "creating" | "error"; dotClass: string; badgeClass: string }> = {
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

  const variant = variants[status];

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

/**
 * Mock log viewer — shows simulated container logs.
 * In production, this would stream real logs from `container_logs` command.
 */
function LogViewer({ containerId: _containerId, isRunning }: { containerId: string; isRunning: boolean }) {
  const mockLogs = isRunning
    ? [
        { time: "16:05:32", level: "INFO", text: "Server started on 0.0.0.0:8000" },
        { time: "16:05:33", level: "INFO", text: "Application startup complete." },
        { time: "16:06:12", level: "INFO", text: "GET /api/health 200 OK 2ms" },
        { time: "16:06:15", level: "INFO", text: "GET /api/todos 200 OK 5ms" },
        { time: "16:07:01", level: "WARN", text: "Slow query detected: 250ms" },
        { time: "16:07:22", level: "INFO", text: "POST /api/todos 201 Created 12ms" },
        { time: "16:08:05", level: "INFO", text: "GET /api/todos/1 200 OK 3ms" },
        { time: "16:08:30", level: "ERROR", text: "Connection pool exhausted, retrying..." },
        { time: "16:08:31", level: "INFO", text: "Connection pool recovered." },
      ]
    : [
        { time: "—", level: "INFO", text: "Container is stopped. No logs available." },
      ];

  const levelColors: Record<string, string> = {
    INFO: "text-cyan-500",
    WARN: "text-yellow-500",
    ERROR: "text-red-500",
    DEBUG: "text-zinc-400",
  };

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-zinc-950 p-3">
      <div className="max-h-64 overflow-y-auto font-mono text-xs leading-5">
        {mockLogs.map((log, i) => (
          <div key={i} className="flex gap-2 whitespace-nowrap">
            <span className="text-zinc-500">{log.time}</span>
            <span className={cn("font-semibold", levelColors[log.level] ?? "text-zinc-400")}>
              [{log.level}]
            </span>
            <span className="text-zinc-300">{log.text}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

/**
 * Mock terminal preview — shows a simulated terminal prompt.
 * In production, this would connect to `container_exec_stream`.
 */
function TerminalPreview() {
  return (
    <div className="overflow-hidden rounded-lg border border-border bg-zinc-950 p-4">
      <div className="min-h-[120px] font-mono text-xs leading-6">
        <div>
          <span className="text-emerald-400">root@container</span>
          <span className="text-zinc-400">:</span>
          <span className="text-blue-400">~</span>
          <span className="text-zinc-400">$ </span>
          <span className="text-zinc-300">ls -la</span>
        </div>
        <div className="text-zinc-500">total 32</div>
        <div className="text-zinc-500">drwxr-xr-x  4 root root 4096 Mar 22 08:00 .</div>
        <div className="text-zinc-500">drwxr-xr-x  1 root root 4096 Mar 22 08:00 ..</div>
        <div className="text-zinc-500">-rw-r--r--  1 root root  220 Mar 22 08:00 .bash_logout</div>
        <div className="mt-1">
          <span className="text-emerald-400">root@container</span>
          <span className="text-zinc-400">:</span>
          <span className="text-blue-400">~</span>
          <span className="text-zinc-400">$ </span>
          <span className="animate-pulse text-zinc-300">_</span>
        </div>
      </div>
    </div>
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
