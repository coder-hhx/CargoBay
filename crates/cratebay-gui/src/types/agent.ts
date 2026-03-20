/**
 * Agent-related type definitions for CrateBay.
 *
 * Re-exports and extends types from @mariozechner/pi-agent-core and @mariozechner/pi-ai.
 */

// Re-export core agent types
export type {
  AgentEvent,
  AgentMessage,
  AgentState,
  AgentTool,
  AgentToolResult,
  AgentToolUpdateCallback,
  StreamFn,
  ThinkingLevel,
} from "@mariozechner/pi-agent-core";

export type {
  AssistantMessage,
  ImageContent,
  Message,
  Model,
  TextContent,
  ToolCall,
  ToolResultMessage,
  Usage,
  UserMessage,
} from "@mariozechner/pi-ai";

/**
 * Risk levels for tool operations.
 */
export type RiskLevel = "low" | "medium" | "high" | "critical";

/**
 * LLM stream event payload from Rust backend.
 * Matches the `LlmStreamEvent` enum in api-spec.md §4.2.
 */
export type LlmStreamEvent =
  | { type: "Token"; content: string }
  | { type: "ToolCall"; id: string; name: string; arguments: string }
  | { type: "Done"; usage: UsageStats }
  | { type: "Error"; message: string };

/**
 * Token usage statistics from the LLM proxy.
 */
export interface UsageStats {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

/**
 * Supported API formats for LLM providers.
 * Matches Rust `ApiFormat` enum.
 */
export type ApiFormat = "anthropic" | "openai_responses" | "openai_completions";
