import { useAppStore } from "@/stores/appStore";
import { cn } from "@/lib/utils";
import { APP_VERSION } from "@/lib/constants";

export function StatusBar() {
  const dockerConnected = useAppStore((s) => s.dockerConnected);
  const runtimeStatus = useAppStore((s) => s.runtimeStatus);

  // Derive a single unified engine status
  const { color, label, pulse } = getEngineStatus(dockerConnected, runtimeStatus);

  return (
    <footer className="flex h-7 flex-shrink-0 items-center justify-between border-t border-border px-4 text-[11px] text-muted-foreground">
      {/* Left: unified engine status */}
      <div className="flex items-center gap-1.5">
        <GlowDot color={color} pulse={pulse} />
        <span>{label}</span>
      </div>

      {/* Right: version */}
      <span className="tabular-nums">v{APP_VERSION}</span>
    </footer>
  );
}

/**
 * Derive a single status from docker + runtime states.
 */
function getEngineStatus(
  dockerConnected: boolean,
  runtimeStatus: "starting" | "running" | "stopped" | "error",
): { color: "green" | "red" | "yellow" | "gray"; label: string; pulse: boolean } {
  // Docker is connected via built-in runtime
  if (dockerConnected) {
    return { color: "green", label: "引擎就绪", pulse: false };
  }
  // Builtin runtime is trying to start
  if (runtimeStatus === "starting") {
    return { color: "yellow", label: "启动中…", pulse: true };
  }
  if (runtimeStatus === "error") {
    return { color: "red", label: "引擎异常", pulse: false };
  }
  return { color: "gray", label: "未连接", pulse: false };
}

/**
 * Glowing dot indicator with CSS box-shadow glow effect.
 */
function GlowDot({ color, pulse }: { color: "green" | "red" | "yellow" | "gray"; pulse?: boolean }) {
  const styles: Record<typeof color, string> = {
    green: "bg-emerald-400 shadow-[0_0_6px_2px_rgba(52,211,153,0.5)]",
    red: "bg-red-400 shadow-[0_0_6px_2px_rgba(248,113,113,0.5)]",
    yellow: "bg-yellow-400 shadow-[0_0_6px_2px_rgba(250,204,21,0.5)]",
    gray: "bg-zinc-400 shadow-none",
  };

  return (
    <span
      className={cn(
        "inline-block h-2 w-2 rounded-full",
        styles[color],
        pulse && "animate-pulse",
      )}
    />
  );
}
