import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export function ContainerDetail() {
  const { t } = useI18n();
  const selectedContainerId = useContainerStore((s) => s.selectedContainerId);
  const containers = useContainerStore((s) => s.containers);
  const selectContainer = useContainerStore((s) => s.selectContainer);

  const container = containers.find((c) => c.id === selectedContainerId) ?? null;

  return (
    <Dialog
      open={container !== null}
      onOpenChange={(open) => { if (!open) selectContainer(null); }}
    >
      <DialogContent className="sm:max-w-lg">
        {container !== null && (
          <>
            <DialogHeader>
              <DialogTitle>{container.name}</DialogTitle>
              <DialogDescription>{t("containers", "containerId")}: {container.id}</DialogDescription>
            </DialogHeader>

            <div className="flex flex-col gap-4 py-4">
              <DetailRow label={t("containers", "status")}>
                <StatusBadge status={container.status} />
              </DetailRow>
              <DetailRow label={t("containers", "image")}>{container.image}</DetailRow>
              <DetailRow label={t("containers", "template")}>{container.templateId}</DetailRow>
              <DetailRow label={t("containers", "cpu")}>{container.cpuCores} cores</DetailRow>
              <DetailRow label={t("containers", "memory")}>{container.memoryMb} MB</DetailRow>
              <DetailRow label={t("containers", "created")}>{new Date(container.createdAt).toLocaleString()}</DetailRow>
              <DetailRow label={t("containers", "shortId")}>{container.shortId}</DetailRow>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

function DetailRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between text-sm">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-foreground">{children}</span>
    </div>
  );
}

function StatusBadge({ status }: { status: ContainerInfo["status"] }) {
  const { t } = useI18n();
  const variants: Record<typeof status, { labelKey: "running" | "stopped" | "creating" | "error"; className: string }> = {
    running: { labelKey: "running", className: "border-success/30 bg-success/10 text-success" },
    stopped: { labelKey: "stopped", className: "border-muted bg-muted/50 text-muted-foreground" },
    creating: { labelKey: "creating", className: "border-yellow-600/30 bg-yellow-600/10 text-yellow-500" },
    error: { labelKey: "error", className: "border-destructive/30 bg-destructive/10 text-destructive" },
  };
  const variant = variants[status];
  return (
    <Badge variant="outline" className={cn("text-[10px]", variant.className)}>
      {t("containers", variant.labelKey)}
    </Badge>
  );
}
