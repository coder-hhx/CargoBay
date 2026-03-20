import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@/lib/tauri";
import { listen } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Send, X } from "lucide-react";
import type { ContainerLogEvent } from "@/types/container";

interface TerminalViewProps {
  containerId: string;
  onClose?: () => void;
}

interface TerminalLine {
  id: number;
  content: string;
  stream: "stdout" | "stderr" | "input";
  timestamp: string;
}

let lineCounter = 0;

/**
 * Container exec terminal.
 * Provides a command input and scrolling output display.
 * Subscribes to container log events for real-time output.
 */
export function TerminalView({ containerId, onClose }: TerminalViewProps) {
  const [lines, setLines] = useState<TerminalLine[]>([]);
  const [command, setCommand] = useState("");
  const [isExecuting, setIsExecuting] = useState(false);
  const [commandHistory, setCommandHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-scroll to bottom
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [lines]);

  // Subscribe to container log events
  useEffect(() => {
    let cleanup: (() => void) | null = null;

    const setup = async () => {
      const unlisten = await listen<ContainerLogEvent>(
        `container:log:${containerId}`,
        (event) => {
          setLines((prev) => [
            ...prev,
            {
              id: ++lineCounter,
              content: event.line,
              stream: event.stream,
              timestamp: event.timestamp,
            },
          ]);
        },
      );
      cleanup = unlisten;
    };

    void setup();
    return () => {
      if (cleanup !== null) cleanup();
    };
  }, [containerId]);

  const executeCommand = useCallback(
    async (cmd: string) => {
      if (cmd.trim().length === 0) return;

      // Add input line
      setLines((prev) => [
        ...prev,
        {
          id: ++lineCounter,
          content: `$ ${cmd}`,
          stream: "input" as const,
          timestamp: new Date().toISOString(),
        },
      ]);

      // Update command history
      setCommandHistory((prev) => [...prev, cmd]);
      setHistoryIndex(-1);
      setCommand("");
      setIsExecuting(true);

      try {
        const result = await invoke<string>("container_exec", {
          id: containerId,
          command: cmd,
        });
        if (result.length > 0) {
          setLines((prev) => [
            ...prev,
            {
              id: ++lineCounter,
              content: result,
              stream: "stdout" as const,
              timestamp: new Date().toISOString(),
            },
          ]);
        }
      } catch (err) {
        setLines((prev) => [
          ...prev,
          {
            id: ++lineCounter,
            content: `Error: ${String(err)}`,
            stream: "stderr" as const,
            timestamp: new Date().toISOString(),
          },
        ]);
      } finally {
        setIsExecuting(false);
        inputRef.current?.focus();
      }
    },
    [containerId],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter" && !isExecuting) {
        void executeCommand(command);
        return;
      }

      // History navigation
      if (e.key === "ArrowUp" && commandHistory.length > 0) {
        e.preventDefault();
        const newIndex = historyIndex === -1 ? commandHistory.length - 1 : Math.max(0, historyIndex - 1);
        setHistoryIndex(newIndex);
        setCommand(commandHistory[newIndex]);
        return;
      }

      if (e.key === "ArrowDown" && commandHistory.length > 0) {
        e.preventDefault();
        if (historyIndex === -1) return;
        const newIndex = historyIndex + 1;
        if (newIndex >= commandHistory.length) {
          setHistoryIndex(-1);
          setCommand("");
        } else {
          setHistoryIndex(newIndex);
          setCommand(commandHistory[newIndex]);
        }
      }
    },
    [command, isExecuting, executeCommand, commandHistory, historyIndex],
  );

  return (
    <div className="flex h-full flex-col rounded-lg border border-border bg-background">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <span className="text-xs font-medium text-muted-foreground">
          Terminal — {containerId.slice(0, 12)}
        </span>
        {onClose !== undefined && (
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={onClose}
            aria-label="Close terminal"
          >
            <X className="h-3.5 w-3.5" />
          </Button>
        )}
      </div>

      {/* Output area */}
      <ScrollArea className="flex-1">
        <div className="p-3 font-mono text-xs">
          {lines.map((line) => (
            <div
              key={line.id}
              className={cn(
                "whitespace-pre-wrap break-all leading-5",
                line.stream === "stderr" && "text-destructive",
                line.stream === "input" && "text-primary font-medium",
                line.stream === "stdout" && "text-foreground",
              )}
            >
              {line.content}
            </div>
          ))}
          <div ref={bottomRef} />
        </div>
      </ScrollArea>

      {/* Command input */}
      <div className="flex items-center gap-2 border-t border-border px-3 py-2">
        <span className="text-xs font-medium text-primary">$</span>
        <Input
          ref={inputRef}
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Enter command..."
          className="h-7 flex-1 border-none bg-transparent font-mono text-xs shadow-none focus-visible:ring-0"
          disabled={isExecuting}
          autoFocus
        />
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={() => void executeCommand(command)}
          disabled={isExecuting || command.trim().length === 0}
          aria-label="Execute command"
        >
          <Send className="h-3 w-3" />
        </Button>
      </div>
    </div>
  );
}
