/**
 * Chat/message type definitions for CrateBay.
 *
 * Matches frontend-spec.md §4.2 — chatStore types.
 */

/**
 * A chat session containing messages.
 */
export interface ChatSession {
  id: string;
  title: string;
  createdAt: string; // ISO 8601
  updatedAt: string;
  messageCount: number;
}

/**
 * A single chat message.
 */
export interface ChatMessage {
  id: string;
  sessionId: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  timestamp: string;
  status: "sending" | "streaming" | "complete" | "error";
  toolCalls?: ToolCallInfo[];
  reasoning?: string; // agent thinking/reasoning content
  metadata?: Record<string, unknown>;
}

/**
 * Information about a tool call within a message.
 */
export interface ToolCallInfo {
  id: string;
  toolName: string;
  toolLabel: string;
  parameters: Record<string, unknown>;
  result?: unknown;
  status: "pending" | "running" | "success" | "error";
  error?: string;
  startedAt?: string;
  completedAt?: string;
}

/**
 * LLM streaming token event from the backend.
 */
export interface LlmTokenEvent {
  sessionId: string;
  token: string;
  done: boolean;
}
