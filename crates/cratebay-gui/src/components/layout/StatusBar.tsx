import { useAppStore } from "@/stores/appStore";
import { cn } from "@/lib/utils";
import { APP_VERSION } from "@/lib/constants";
import { Badge } from "@/components/ui/badge";

export function StatusBar() {
  const dockerConnected = useAppStore((s) => s.dockerConnected);
  const runtimeStatus = useAppStore((s) => s.runtimeStatus);

  return (
    <footer className="flex h-7 flex-shrink-0 items-center justify-between border-t border-border bg-card px-4 text-xs text-muted-foreground">
      {/* Left: status indicators */}
      <div className="flex items-center gap-3">
        {/* Docker status */}
        <div className="flex items-center gap-1.5">
          <span
            className={cn(
              "inline-block h-2 w-2 rounded-full",
              dockerConnected ? "bg-success" : "bg-destructive",
            )}
          />
          <span>Docker {dockerConnected ? "Connected" : "Disconnected"}</span>
        </div>

        {/* Runtime status */}
        <div className="flex items-center gap-1.5">
          <RuntimeStatusBadge status={runtimeStatus} />
        </div>
      </div>

      {/* Right: version */}
      <span>v{APP_VERSION}</span>
    </footer>
  );
}

function RuntimeStatusBadge({
  status,
}: {
  status: "starting" | "running" | "stopped" | "error";
}) {
  const variants: Record<typeof status, { label: string; className: string }> = {
    starting: {
      label: "Starting",
      className: "border-yellow-600/30 bg-yellow-600/10 text-yellow-500",
    },
    running: {
      label: "Running",
      className: "border-success/30 bg-success/10 text-success",
    },
    stopped: {
      label: "Stopped",
      className: "border-muted bg-muted/50 text-muted-foreground",
    },
    error: {
      label: "Error",
      className: "border-destructive/30 bg-destructive/10 text-destructive",
    },
  };

  const variant = variants[status];

  return (
    <Badge variant="outline" className={cn("h-4 px-1.5 text-[10px]", variant.className)}>
      Runtime: {variant.label}
    </Badge>
  );
}
