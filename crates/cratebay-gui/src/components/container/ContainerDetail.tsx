import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
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
              <DialogDescription>Container ID: {container.id}</DialogDescription>
            </DialogHeader>

            <div className="flex flex-col gap-4 py-4">
              <DetailRow label="Status">
                <StatusBadge status={container.status} />
              </DetailRow>
              <DetailRow label="Image">{container.image}</DetailRow>
              <DetailRow label="Template">{container.templateId}</DetailRow>
              <DetailRow label="CPU">{container.cpuCores} cores</DetailRow>
              <DetailRow label="Memory">{container.memoryMb} MB</DetailRow>
              <DetailRow label="Created">{new Date(container.createdAt).toLocaleString()}</DetailRow>
              <DetailRow label="Short ID">{container.shortId}</DetailRow>
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
  const variants: Record<typeof status, { label: string; className: string }> = {
    running: { label: "Running", className: "border-success/30 bg-success/10 text-success" },
    stopped: { label: "Stopped", className: "border-muted bg-muted/50 text-muted-foreground" },
    creating: { label: "Creating", className: "border-yellow-600/30 bg-yellow-600/10 text-yellow-500" },
    error: { label: "Error", className: "border-destructive/30 bg-destructive/10 text-destructive" },
  };
  const variant = variants[status];
  return (
    <Badge variant="outline" className={cn("text-[10px]", variant.className)}>
      {variant.label}
    </Badge>
  );
}
