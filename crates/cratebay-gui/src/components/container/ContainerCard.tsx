import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import type { ContainerInfo } from "@/types/container";
import { useContainerStore } from "@/stores/containerStore";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Play,
  Square,
  Trash2,
  Terminal,
  ScrollText,
  Eye,
} from "lucide-react";

interface ContainerCardProps {
  container: ContainerInfo;
  onOpenTerminal?: (containerId: string) => void;
}

/**
 * SVG sparkline for resource usage trends.
 */
function Sparkline({ data, color }: { data: number[]; color: string }) {
  if (!data || data.length === 0) return null;
  const max = Math.max(...data, 1);
  const w = 60;
  const h = 24;
  const points = data
    .map((v, i) => `${(i / (data.length - 1)) * w},${h - (v / max) * h}`)
    .join(" ");
  return (
    <svg className="h-6 w-full" viewBox={`0 0 ${w} ${h}`}>
      <polyline
        points={points}
        stroke={color}
        fill="none"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/**
 * Single container card for grid view.
 * Shows name, status badge, image, ports, resource stats with progress bars and sparklines, and action buttons.
 */
export function ContainerCard({ container, onOpenTerminal }: ContainerCardProps) {
  const { t } = useI18n();
  const startContainer = useContainerStore((s) => s.startContainer);
  const stopContainer = useContainerStore((s) => s.stopContainer);
  const deleteContainer = useContainerStore((s) => s.deleteContainer);
  const selectContainer = useContainerStore((s) => s.selectContainer);
  const isRunning = container.status === "running";
  const isStopped = container.status === "stopped";
  const isCreating = container.status === "creating";

  // Mock resource percentages based on available data
  // In production, these would come from real-time stats
  const cpuPercent = isRunning ? Math.min(Math.round((container.cpuCores / 8) * 100), 100) : 0;
  const memPercent = isRunning ? Math.min(Math.round((container.memoryMb / 8192) * 100), 100) : 0;

  // Mock sparkline data (in production, this would come from historical stats)
  const cpuHistory = isRunning
    ? Array.from({ length: 10 }, () => Math.floor(Math.random() * cpuPercent + 5))
    : [];
  const memHistory = isRunning
    ? Array.from({ length: 10 }, () => Math.floor(Math.random() * memPercent + 3))
    : [];

  return (
    <div
      className={cn(
        "rounded-xl border bg-card p-4 transition-all hover:border-primary",
        isRunning && "border-success/30",
        container.status === "error" && "border-destructive/30",
        isCreating && "border-yellow-500/30",
        isStopped && "border-border",
      )}
    >
      {/* Header: name + status badge */}
      <div className="mb-2 flex items-start justify-between">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-semibold text-foreground">
            {container.name}
          </h3>
          <p className="mt-0.5 truncate font-mono text-xs text-muted-foreground">
            {container.image}
          </p>
        </div>
        <StatusBadge status={container.status} />
      </div>

      {/* Port tags */}
      {container.ports.length > 0 && (
        <div className="mb-3 flex flex-wrap gap-1">
          {container.ports.map((port) => (
            <span
              key={`${port.hostPort}-${port.containerPort}`}
              className="inline-flex items-center rounded-md border border-border bg-muted/50 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
            >
              {port.hostPort}:{port.containerPort}
            </span>
          ))}
        </div>
      )}

      {/* Resource stats: 4-column grid */}
      {isRunning && (
        <div className="mb-3 grid grid-cols-4 gap-2">
          {/* CPU % + bar */}
          <div className="flex flex-col gap-1">
            <span className="text-[10px] text-muted-foreground">{t("containers", "cpu")}</span>
            <span className="text-xs font-medium text-foreground">{cpuPercent}%</span>
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary transition-all"
                style={{ width: `${cpuPercent}%` }}
              />
            </div>
          </div>

          {/* Memory % + bar */}
          <div className="flex flex-col gap-1">
            <span className="text-[10px] text-muted-foreground">{t("containers", "memory")}</span>
            <span className="text-xs font-medium text-foreground">{memPercent}%</span>
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-accent transition-all"
                style={{ width: `${memPercent}%` }}
              />
            </div>
          </div>

          {/* CPU Sparkline */}
          <div className="flex flex-col gap-1">
            <span className="text-[10px] text-muted-foreground">{t("containers", "cpu")}</span>
            <Sparkline data={cpuHistory} color="hsl(var(--primary))" />
          </div>

          {/* Memory Sparkline */}
          <div className="flex flex-col gap-1">
            <span className="text-[10px] text-muted-foreground">{t("containers", "memory")}</span>
            <Sparkline data={memHistory} color="hsl(var(--accent))" />
          </div>
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center gap-1 border-t border-border pt-3">
        {isRunning ? (
          <>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 gap-1 px-2 text-xs"
              onClick={() => void stopContainer(container.id)}
            >
              <Square className="h-3.5 w-3.5" />
              {t("containers", "stop")}
            </Button>
            {onOpenTerminal !== undefined && (
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1 px-2 text-xs"
                onClick={() => onOpenTerminal(container.id)}
              >
                <Terminal className="h-3.5 w-3.5" />
                {t("containers", "terminal")}
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              className="h-7 gap-1 px-2 text-xs"
              onClick={() => selectContainer(container.id)}
            >
              <ScrollText className="h-3.5 w-3.5" />
              {t("containers", "logs")}
            </Button>
          </>
        ) : isStopped ? (
          <Button
            variant="ghost"
            size="sm"
            className="h-7 gap-1 px-2 text-xs"
            onClick={() => void startContainer(container.id)}
          >
            <Play className="h-3.5 w-3.5" />
            {t("containers", "start")}
          </Button>
        ) : isCreating ? (
          <Button variant="ghost" size="sm" className="h-7 px-2 text-xs" disabled>
            {t("containers", "creating")}...
          </Button>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            className="h-7 gap-1 px-2 text-xs"
            onClick={() => selectContainer(container.id)}
          >
            <Eye className="h-3.5 w-3.5" />
            {t("common", "details")}
          </Button>
        )}

        <Button
          variant="ghost"
          size="sm"
          className="ml-auto h-7 px-2 text-xs text-destructive hover:text-destructive"
          onClick={() => void deleteContainer(container.id)}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: ContainerInfo["status"] }) {
  const { t } = useI18n();
  const variants: Record<typeof status, { labelKey: "running" | "stopped" | "creating" | "error"; dotClass: string; textClass: string }> = {
    running: {
      labelKey: "running",
      dotClass: "bg-success",
      textClass: "text-success",
    },
    stopped: {
      labelKey: "stopped",
      dotClass: "bg-muted-foreground",
      textClass: "text-muted-foreground",
    },
    creating: {
      labelKey: "creating",
      dotClass: "bg-yellow-500 animate-pulse",
      textClass: "text-yellow-500",
    },
    error: {
      labelKey: "error",
      dotClass: "bg-destructive",
      textClass: "text-destructive",
    },
  };

  const variant = variants[status];

  return (
    <Badge
      variant="outline"
      className={cn(
        "flex items-center gap-1.5 border-transparent bg-transparent px-0 text-[10px] font-medium",
        variant.textClass,
      )}
    >
      <span className={cn("inline-block h-1.5 w-1.5 rounded-full", variant.dotClass)} />
      {t("containers", variant.labelKey)}
    </Badge>
  );
}
