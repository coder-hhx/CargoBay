import { useState } from "react";
import { cn } from "@/lib/utils";
import { ChevronDown, ChevronRight, Brain } from "lucide-react";

interface AgentThinkingProps {
  content: string;
  isActive: boolean; // true while agent is still thinking
}

/**
 * Displays the agent's reasoning chain in a collapsible panel.
 * Default open during thinking, collapsed after completion.
 * Monospace font for reasoning text.
 */
export function AgentThinking({ content, isActive }: AgentThinkingProps) {
  const [collapsed, setCollapsed] = useState(false);

  // When active, default to expanded
  const isOpen = isActive ? !collapsed : !collapsed;

  return (
    <div
      className={cn(
        "my-2 rounded-md border-l-2 bg-muted/40",
        isActive ? "border-l-primary" : "border-l-muted-foreground/40",
      )}
    >
      {/* Header — clickable to toggle */}
      <button
        type="button"
        onClick={() => setCollapsed((prev) => !prev)}
        className="flex w-full items-center gap-2 px-3 py-2 text-xs text-muted-foreground hover:text-foreground"
      >
        {isOpen ? (
          <ChevronDown className="h-3 w-3" />
        ) : (
          <ChevronRight className="h-3 w-3" />
        )}
        <Brain className="h-3 w-3" />
        <span className="font-medium">
          {isActive ? "Thinking" : "Reasoning"}
        </span>
        {isActive && <ThinkingDots />}
      </button>

      {/* Content */}
      {isOpen && (
        <div className="px-3 pb-3">
          <pre className="whitespace-pre-wrap break-words font-mono text-xs leading-relaxed text-muted-foreground">
            {content}
          </pre>
        </div>
      )}
    </div>
  );
}

/**
 * Animated ellipsis while the agent is thinking.
 */
function ThinkingDots() {
  return (
    <span className="inline-flex gap-0.5">
      <span className="h-1 w-1 animate-bounce rounded-full bg-muted-foreground [animation-delay:0ms]" />
      <span className="h-1 w-1 animate-bounce rounded-full bg-muted-foreground [animation-delay:150ms]" />
      <span className="h-1 w-1 animate-bounce rounded-full bg-muted-foreground [animation-delay:300ms]" />
    </span>
  );
}
