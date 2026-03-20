import { cn } from "@/lib/utils";
import type { ContainerInfo } from "@/types/container";
import { useContainerStore } from "@/stores/containerStore";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  Play,
  Square,
  Trash2,
  Eye,
  Cpu,
  MemoryStick,
  Terminal,
} from "lucide-react";

interface ContainerCardProps {
  container: ContainerInfo;
  onOpenTerminal?: (containerId: string) => void;
}

/**
 * Single container status card for grid view.
 * Shows name, status, resource info, and inline action buttons.
 */
export function ContainerCard({ container, onOpenTerminal }: ContainerCardProps) {
  const startContainer = useContainerStore((s) => s.startContainer);
  const stopContainer = useContainerStore((s) => s.stopContainer);
  const deleteContainer = useContainerStore((s) => s.deleteContainer);
  const selectContainer = useContainerStore((s) => s.selectContainer);
  const isRunning = container.status === "running";

  return (
    <div
      className={cn(
        "rounded-lg border bg-card p-4 transition-colors hover:bg-card/80",
        isRunning ? "border-success/30" : "border-border",
      )}
    >
      {/* Header: name + status */}
      <div className="mb-3 flex items-start justify-between">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-medium text-foreground">
            {container.name}
          </h3>
          <p className="mt-0.5 truncate text-xs text-muted-foreground">{container.image}</p>
        </div>
        <StatusBadge status={container.status} />
      </div>

      {/* Resource info */}
      <div className="mb-3 flex items-center gap-4 text-xs text-muted-foreground">
        <span className="flex items-center gap-1">
          <Cpu className="h-3 w-3" />
          {container.cpuCores} cores
        </span>
        <span className="flex items-center gap-1">
          <MemoryStick className="h-3 w-3" />
          {container.memoryMb} MB
        </span>
      </div>

      {/* Port mappings */}
      {container.ports.length > 0 && (
        <div className="mb-3 flex flex-wrap gap-1">
          {container.ports.map((port) => (
            <Badge
              key={`${port.hostPort}-${port.containerPort}`}
              variant="outline"
              className="text-[10px]"
            >
              {port.hostPort}:{port.containerPort}/{port.protocol}
            </Badge>
          ))}
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center gap-1 border-t border-border pt-3">
        <TooltipProvider delayDuration={300}>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => selectContainer(container.id)}
                aria-label="View details"
              >
                <Eye className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>View details</TooltipContent>
          </Tooltip>

          {isRunning && onOpenTerminal !== undefined && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => onOpenTerminal(container.id)}
                  aria-label="Open terminal"
                >
                  <Terminal className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Open terminal</TooltipContent>
            </Tooltip>
          )}

          {isRunning ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => void stopContainer(container.id)}
                  aria-label="Stop container"
                >
                  <Square className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Stop</TooltipContent>
            </Tooltip>
          ) : (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => void startContainer(container.id)}
                  aria-label="Start container"
                >
                  <Play className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Start</TooltipContent>
            </Tooltip>
          )}

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => void deleteContainer(container.id)}
                aria-label="Delete container"
                className="ml-auto"
              >
                <Trash2 className="h-3.5 w-3.5 text-destructive" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Delete</TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>
    </div>
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
