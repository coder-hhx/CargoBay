import { useState } from "react";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import type { ToolCallInfo } from "@/types/chat";
import {
  CheckCircle,
  XCircle,
  Loader2,
  Clock,
  ChevronDown,
  ChevronRight,
  RotateCcw,
} from "lucide-react";
import { Button } from "@/components/ui/button";

interface ToolCallCardProps {
  toolCall: ToolCallInfo;
  onRetry?: (toolCall: ToolCallInfo) => void;
}

/**
 * Inline card showing tool execution status within the message flow.
 *
 * States:
 * - pending: Muted card, "Preparing..."
 * - running: Animated border, spinner, "Executing..."
 * - success: Success border, check icon, result preview
 * - error: Destructive border, error message
 */
export function ToolCallCard({ toolCall, onRetry }: ToolCallCardProps) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);

  const statusConfig = getStatusConfig(toolCall.status, t);
  const duration = getDuration(toolCall.startedAt, toolCall.completedAt);

  return (
    <div
      className={cn(
        "my-2 rounded-lg border bg-card text-sm",
        statusConfig.borderClass,
        toolCall.status === "running" && "animate-pulse",
      )}
    >
      {/* Header */}
      <button
        type="button"
        onClick={() => setExpanded((prev) => !prev)}
        className="flex w-full items-center gap-2 px-3 py-2"
      >
        {/* Status icon */}
        <statusConfig.Icon className={cn("h-4 w-4 flex-shrink-0", statusConfig.iconClass)} />

        {/* Tool name */}
        <span className="flex-1 text-left font-medium text-foreground">
          {toolCall.toolLabel}
        </span>

        {/* Duration */}
        {duration !== null && (
          <span className="text-xs text-muted-foreground">{duration}</span>
        )}

        {/* Status text */}
        <span className={cn("text-xs", statusConfig.textClass)}>{statusConfig.label}</span>

        {/* Expand chevron */}
        {expanded ? (
          <ChevronDown className="h-3 w-3 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-3 w-3 text-muted-foreground" />
        )}
      </button>

      {/* Expanded details */}
      {expanded && (
        <div className="border-t border-border px-3 py-2 text-xs">
          {/* Parameters */}
          <div className="mb-2">
            <p className="mb-1 font-medium text-muted-foreground">{t("common", "parameters")}:</p>
            <pre className="overflow-x-auto rounded bg-muted/50 p-2 font-mono text-muted-foreground">
              {JSON.stringify(toolCall.parameters, null, 2)}
            </pre>
          </div>

          {/* Result */}
          {toolCall.status === "success" && toolCall.result !== undefined && (
            <div className="mb-2">
              <p className="mb-1 font-medium text-muted-foreground">{t("common", "result")}:</p>
              <pre className="overflow-x-auto rounded bg-muted/50 p-2 font-mono text-muted-foreground">
                {typeof toolCall.result === "string"
                  ? toolCall.result
                  : JSON.stringify(toolCall.result, null, 2)}
              </pre>
            </div>
          )}

          {/* Error */}
          {toolCall.status === "error" && toolCall.error !== undefined && (
            <div className="mb-2">
              <p className="mb-1 font-medium text-destructive">{t("common", "error")}:</p>
              <pre className="overflow-x-auto rounded bg-destructive/10 p-2 font-mono text-destructive">
                {toolCall.error}
              </pre>
            </div>
          )}

          {/* Retry button for failed tools */}
          {toolCall.status === "error" && onRetry !== undefined && (
            <Button
              size="sm"
              variant="outline"
              onClick={() => onRetry(toolCall)}
              className="mt-1"
            >
              <RotateCcw className="mr-1 h-3 w-3" />
              {t("common", "retry")}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}

interface StatusConfig {
  Icon: React.ComponentType<{ className?: string }>;
  iconClass: string;
  borderClass: string;
  textClass: string;
  label: string;
}

type TranslateFn = <K extends keyof import("@/types/i18n").Translations, S extends keyof import("@/types/i18n").Translations[K]>(
  namespace: K,
  key: S,
) => string;

function getStatusConfig(status: ToolCallInfo["status"], t: TranslateFn): StatusConfig {
  switch (status) {
    case "pending":
      return {
        Icon: Clock,
        iconClass: "text-muted-foreground",
        borderClass: "border-border",
        textClass: "text-muted-foreground",
        label: t("chat", "toolPreparing"),
      };
    case "running":
      return {
        Icon: Loader2,
        iconClass: "text-primary animate-spin",
        borderClass: "border-primary/50",
        textClass: "text-primary",
        label: t("chat", "toolExecuting"),
      };
    case "success":
      return {
        Icon: CheckCircle,
        iconClass: "text-success",
        borderClass: "border-success/50",
        textClass: "text-success",
        label: t("chat", "toolCompleted"),
      };
    case "error":
      return {
        Icon: XCircle,
        iconClass: "text-destructive",
        borderClass: "border-destructive/50",
        textClass: "text-destructive",
        label: t("chat", "toolFailed"),
      };
  }
}

function getDuration(startedAt?: string, completedAt?: string): string | null {
  if (startedAt === undefined || completedAt === undefined) return null;
  const ms = new Date(completedAt).getTime() - new Date(startedAt).getTime();
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}
