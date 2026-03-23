import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Play, Square, Trash2, Eye } from "lucide-react";

export function ContainerList() {
  const { t } = useI18n();
  const filteredContainers = useContainerStore((s) => s.filteredContainers);
  const loading = useContainerStore((s) => s.loading);
  const containers = filteredContainers();

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
        {t("containers", "loadingContainers")}
      </div>
    );
  }

  if (containers.length === 0) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
        {t("containers", "noContainersHint")}
      </div>
    );
  }

  return (
    <div className="overflow-x-auto" data-testid="container-list">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border text-left text-xs text-muted-foreground">
            <th className="px-4 py-2 font-medium">{t("containers", "name")}</th>
            <th className="px-4 py-2 font-medium">{t("containers", "image")}</th>
            <th className="px-4 py-2 font-medium">{t("containers", "status")}</th>
            <th className="px-4 py-2 font-medium">{t("containers", "cpu")}</th>
            <th className="px-4 py-2 font-medium">{t("containers", "memory")}</th>
            <th className="px-4 py-2 text-right font-medium">{t("containers", "actions")}</th>
          </tr>
        </thead>
        <tbody>
          {containers.map((container) => (
            <ContainerRow key={container.id} container={container} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ContainerRow({ container }: { container: ContainerInfo }) {
  const { t } = useI18n();
  const startContainer = useContainerStore((s) => s.startContainer);
  const stopContainer = useContainerStore((s) => s.stopContainer);
  const deleteContainer = useContainerStore((s) => s.deleteContainer);
  const selectContainer = useContainerStore((s) => s.selectContainer);
  const isRunning = container.status === "running" || container.status === "paused";

  return (
    <tr className="border-b border-border transition-colors hover:bg-muted/30" data-testid="container-card">
      <td className="px-4 py-2.5">
        <span className="font-medium text-foreground">{container.name}</span>
        <span className="ml-2 text-xs text-muted-foreground">{container.shortId}</span>
      </td>
      <td className="px-4 py-2.5 text-muted-foreground">
        <span className="truncate">{container.image}</span>
      </td>
      <td className="px-4 py-2.5">
        <StatusBadge status={container.status} />
      </td>
      <td className="px-4 py-2.5 text-muted-foreground">{container.cpuCores} cores</td>
      <td className="px-4 py-2.5 text-muted-foreground">{container.memoryMb} MB</td>
      <td className="px-4 py-2.5">
        <div className="flex items-center justify-end gap-1">
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => selectContainer(container.id)}
            aria-label={t("containers", "viewDetails")}
          >
            <Eye className="h-3.5 w-3.5" />
          </Button>
          {isRunning ? (
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={() => void stopContainer(container.id)}
              data-testid="container-stop"
              aria-label={t("containers", "stop")}
            >
              <Square className="h-3.5 w-3.5" />
            </Button>
          ) : (
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={() => void startContainer(container.id)}
              data-testid="container-start"
              aria-label={t("containers", "start")}
            >
              <Play className="h-3.5 w-3.5" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => void deleteContainer(container.id)}
            data-testid="container-delete"
            aria-label={t("containers", "delete")}
          >
            <Trash2 className="h-3.5 w-3.5 text-destructive" />
          </Button>
        </div>
      </td>
    </tr>
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

  const variants: Record<DisplayStatus, { labelKey: DisplayStatus; className: string }> = {
    running: { labelKey: "running", className: "border-success/30 bg-success/10 text-success" },
    stopped: { labelKey: "stopped", className: "border-muted bg-muted/50 text-muted-foreground" },
    creating: { labelKey: "creating", className: "border-yellow-600/30 bg-yellow-600/10 text-yellow-500" },
    error: { labelKey: "error", className: "border-destructive/30 bg-destructive/10 text-destructive" },
  };

  const variant = variants[displayStatus];

  return (
    <Badge variant="outline" className={cn("text-[10px]", variant.className)}>
      {t("containers", variant.labelKey)}
    </Badge>
  );
}
