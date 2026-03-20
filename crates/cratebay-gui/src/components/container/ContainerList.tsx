import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Play, Square, Trash2, Eye } from "lucide-react";

export function ContainerList() {
  const filteredContainers = useContainerStore((s) => s.filteredContainers);
  const loading = useContainerStore((s) => s.loading);
  const containers = filteredContainers();

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
        Loading containers...
      </div>
    );
  }

  if (containers.length === 0) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
        No containers found. Create one to get started.
      </div>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border text-left text-xs text-muted-foreground">
            <th className="px-4 py-2 font-medium">Name</th>
            <th className="px-4 py-2 font-medium">Image</th>
            <th className="px-4 py-2 font-medium">Status</th>
            <th className="px-4 py-2 font-medium">CPU</th>
            <th className="px-4 py-2 font-medium">Memory</th>
            <th className="px-4 py-2 text-right font-medium">Actions</th>
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
  const startContainer = useContainerStore((s) => s.startContainer);
  const stopContainer = useContainerStore((s) => s.stopContainer);
  const deleteContainer = useContainerStore((s) => s.deleteContainer);
  const selectContainer = useContainerStore((s) => s.selectContainer);
  const isRunning = container.status === "running";

  return (
    <tr className="border-b border-border transition-colors hover:bg-muted/30">
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
            aria-label="View details"
          >
            <Eye className="h-3.5 w-3.5" />
          </Button>
          {isRunning ? (
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={() => void stopContainer(container.id)}
              aria-label="Stop container"
            >
              <Square className="h-3.5 w-3.5" />
            </Button>
          ) : (
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={() => void startContainer(container.id)}
              aria-label="Start container"
            >
              <Play className="h-3.5 w-3.5" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => void deleteContainer(container.id)}
            aria-label="Delete container"
          >
            <Trash2 className="h-3.5 w-3.5 text-destructive" />
          </Button>
        </div>
      </td>
    </tr>
  );
}

function StatusBadge({ status }: { status: ContainerInfo["status"] }) {
  const variants: Record<typeof status, { label: string; className: string }> = {
    running: {
      label: "Running",
      className: "border-success/30 bg-success/10 text-success",
    },
    stopped: {
      label: "Stopped",
      className: "border-muted bg-muted/50 text-muted-foreground",
    },
    creating: {
      label: "Creating",
      className: "border-yellow-600/30 bg-yellow-600/10 text-yellow-500",
    },
    error: {
      label: "Error",
      className: "border-destructive/30 bg-destructive/10 text-destructive",
    },
  };

  const variant = variants[status];

  return (
    <Badge variant="outline" className={cn("text-[10px]", variant.className)}>
      {variant.label}
    </Badge>
  );
}
