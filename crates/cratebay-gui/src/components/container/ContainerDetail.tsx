import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import { SlidePanel } from "@/components/common/SlidePanel";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { ContainerLogs } from "./ContainerLogs";
import { ContainerMonitoring } from "./ContainerMonitoring";
import { TerminalView } from "./TerminalView";

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
  const isRunning = container.status === "running" || container.status === "paused";

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
        </div>
      </section>

      {/* Specs section (limits) */}
      <section>
        <SectionTitle>Specs</SectionTitle>
        <div className="grid grid-cols-2 gap-x-6 gap-y-3">
          <DetailField label={t("containers", "cpuCores")}>
            {container.cpuCores !== undefined ? `${container.cpuCores} cores` : "—"}
          </DetailField>
          <DetailField label={t("containers", "memoryMb")}>
            {container.memoryMb !== undefined ? `${container.memoryMb} MB` : "—"}
          </DetailField>
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
