import { useEffect, useCallback } from "react";
import { useAppStore } from "@/stores/appStore";
import { cn } from "@/lib/utils";
import { X, AlertCircle, CheckCircle, Info, AlertTriangle } from "lucide-react";

const ICON_MAP = {
  info: Info,
  success: CheckCircle,
  warning: AlertTriangle,
  error: AlertCircle,
} as const;

const STYLE_MAP = {
  info: "border-blue-500/30 bg-blue-500/10 text-blue-400",
  success: "border-emerald-500/30 bg-emerald-500/10 text-emerald-400",
  warning: "border-yellow-500/30 bg-yellow-500/10 text-yellow-400",
  error: "border-red-500/30 bg-red-500/10 text-red-400",
} as const;

/**
 * Global toast notification system.
 * Renders notifications from appStore in the bottom-right corner.
 * Auto-dismisses after 6 seconds (errors: 10 seconds).
 */
export function ToastContainer() {
  const notifications = useAppStore((s) => s.notifications);
  const dismissNotification = useAppStore((s) => s.dismissNotification);

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[9999] flex flex-col-reverse gap-2">
      {notifications.slice(-5).map((n) => (
        <ToastItem
          key={n.id}
          id={n.id}
          type={n.type}
          title={n.title}
          message={n.message}
          dismissable={n.dismissable}
          onDismiss={dismissNotification}
        />
      ))}
    </div>
  );
}

interface ToastItemProps {
  id: string;
  type: "info" | "success" | "warning" | "error";
  title: string;
  message?: string;
  dismissable: boolean;
  onDismiss: (id: string) => void;
}

function ToastItem({ id, type, title, message, dismissable, onDismiss }: ToastItemProps) {
  const handleDismiss = useCallback(() => onDismiss(id), [id, onDismiss]);

  // Auto-dismiss
  useEffect(() => {
    const delay = type === "error" ? 10000 : 6000;
    const timer = setTimeout(handleDismiss, delay);
    return () => clearTimeout(timer);
  }, [handleDismiss, type]);

  const Icon = ICON_MAP[type];

  return (
    <div
      className={cn(
        "pointer-events-auto flex w-80 items-start gap-3 rounded-lg border p-3 shadow-lg backdrop-blur-sm",
        "animate-in slide-in-from-right-full fade-in duration-300",
        STYLE_MAP[type],
      )}
    >
      <Icon className="mt-0.5 h-4 w-4 flex-shrink-0" />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-foreground">{title}</p>
        {message && (
          <p className="mt-0.5 text-xs text-muted-foreground line-clamp-3">{message}</p>
        )}
      </div>
      {dismissable && (
        <button
          onClick={handleDismiss}
          className="flex-shrink-0 rounded p-0.5 text-muted-foreground transition-colors hover:text-foreground"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}
