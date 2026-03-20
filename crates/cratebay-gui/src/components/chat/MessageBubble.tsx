import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";
import { User, Bot } from "lucide-react";
import { Streamdown } from "streamdown";
import type { ChatMessage } from "@/types/chat";
import { AgentThinking } from "./AgentThinking";
import { ToolCallCard } from "./ToolCallCard";

interface MessageBubbleProps {
  message: ChatMessage;
  isThinking?: boolean;
  thinkingContent?: string;
}

/**
 * Single message bubble with Streamdown rendering for assistant messages.
 * Includes agent thinking display and tool call cards.
 */
export function MessageBubble({ message, isThinking, thinkingContent }: MessageBubbleProps) {
  const isUser = message.role === "user";
  const isAssistant = message.role === "assistant";
  const isStreaming = message.status === "streaming";
  const streamdownRef = useRef<HTMLDivElement>(null);

  // Apply streaming-friendly class to Streamdown container
  useEffect(() => {
    if (streamdownRef.current !== null && isStreaming) {
      streamdownRef.current.scrollIntoView({ behavior: "smooth", block: "end" });
    }
  }, [message.content, isStreaming]);

  return (
    <div
      className={cn(
        "flex gap-3",
        isUser ? "flex-row-reverse" : "flex-row",
      )}
    >
      {/* Avatar */}
      <div
        className={cn(
          "flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-full text-xs",
          isUser
            ? "bg-primary text-background"
            : "bg-muted text-muted-foreground",
        )}
      >
        {isUser ? <User className="h-4 w-4" /> : <Bot className="h-4 w-4" />}
      </div>

      {/* Message content */}
      <div
        className={cn(
          "max-w-[80%] rounded-lg px-3.5 py-2.5 text-sm",
          isUser
            ? "bg-primary text-background"
            : "bg-card border border-border text-foreground",
        )}
      >
        {/* Role label */}
        <p
          className={cn(
            "mb-1 text-[10px] font-medium uppercase tracking-wider",
            isUser ? "text-background/70" : "text-muted-foreground",
          )}
        >
          {message.role}
        </p>

        {/* Agent thinking / reasoning */}
        {isAssistant && message.reasoning !== undefined && (
          <AgentThinking
            content={message.reasoning}
            isActive={isThinking ?? false}
          />
        )}
        {isAssistant && isThinking === true && thinkingContent !== undefined && message.reasoning === undefined && (
          <AgentThinking
            content={thinkingContent}
            isActive
          />
        )}

        {/* Tool call cards */}
        {message.toolCalls !== undefined && message.toolCalls.length > 0 && (
          <div className="my-1">
            {message.toolCalls.map((tc) => (
              <ToolCallCard key={tc.id} toolCall={tc} />
            ))}
          </div>
        )}

        {/* Content — use Streamdown for assistant, plain text for user */}
        {isAssistant ? (
          <div ref={streamdownRef} className="leading-relaxed">
            <Streamdown mode={isStreaming ? "streaming" : "static"}>
              {message.content}
            </Streamdown>
          </div>
        ) : (
          <div className="whitespace-pre-wrap break-words leading-relaxed">
            {message.content}
          </div>
        )}

        {/* Error indicator */}
        {message.status === "error" && (
          <p className="mt-1.5 text-xs text-destructive">
            Failed to send message.
          </p>
        )}
      </div>
    </div>
  );
}
