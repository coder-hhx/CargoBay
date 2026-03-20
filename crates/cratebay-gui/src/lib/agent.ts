/**
 * CrateBay Agent Engine.
 *
 * Wraps @mariozechner/pi-agent-core Agent class with CrateBay-specific
 * configuration: custom streamFn (Tauri proxy), built-in tools, system prompt,
 * and confirmation flow for destructive operations.
 */

import { Agent, type AgentOptions } from "@mariozechner/pi-agent-core";
import type { AgentTool, AgentMessage, AgentEvent } from "@mariozechner/pi-agent-core";
import type { Message, Model, UserMessage } from "@mariozechner/pi-ai";
import { createStreamFn } from "@/lib/streamFn";
import { buildSystemPrompt } from "@/lib/systemPrompt";
import { allTools, getToolRiskLevel } from "@/tools";

/**
 * Configuration for creating a CrateBay agent instance.
 */
export interface CrateBayAgentConfig {
  /** LLM provider ID (from settingsStore) */
  providerId: string;
  /** LLM model configuration */
  model: Model<string>;
  /** System prompt override */
  systemPrompt?: string;
  /** Additional tools beyond the built-in set */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  extraTools?: AgentTool<any>[];
  /** Confirmation callback for destructive operations */
  onConfirmationRequired?: (
    toolName: string,
    args: unknown,
    riskLevel: string,
  ) => Promise<boolean>;
}

/**
 * Convert AgentMessage[] to LLM-compatible Message[] for the agent loop.
 *
 * This is the `convertToLlm` callback required by pi-agent-core.
 * It filters out custom message types and passes through standard LLM messages.
 */
function defaultConvertToLlm(messages: AgentMessage[]): Message[] {
  return messages.filter(
    (m): m is Message =>
      typeof m === "object" &&
      m !== null &&
      "role" in m &&
      (m.role === "user" || m.role === "assistant" || m.role === "toolResult"),
  );
}

/**
 * Context transform: prune old messages to fit within token budget.
 *
 * Simple strategy: keep the most recent messages up to a rough token estimate.
 * System prompt is always included (handled by pi-agent-core separately).
 */
async function defaultTransformContext(
  messages: AgentMessage[],
): Promise<AgentMessage[]> {
  const MAX_MESSAGES = 50;
  if (messages.length <= MAX_MESSAGES) return messages;

  // Keep the most recent messages
  return messages.slice(-MAX_MESSAGES);
}

/**
 * Create a configured CrateBay Agent instance.
 *
 * @param config - Agent configuration
 * @returns Configured Agent instance ready for use
 */
export function createCrateBayAgent(config: CrateBayAgentConfig): Agent {
  const tools = [...allTools, ...(config.extraTools ?? [])];
  const systemPrompt = config.systemPrompt ?? buildSystemPrompt(tools);

  const agentOptions: AgentOptions = {
    initialState: {
      systemPrompt,
      model: config.model,
      tools,
      thinkingLevel: "medium",
    },
    streamFn: createStreamFn(config.providerId),
    convertToLlm: defaultConvertToLlm,
    transformContext: defaultTransformContext,
    toolExecution: "sequential",
    beforeToolCall: async (context) => {
      const riskLevel = getToolRiskLevel(context.toolCall.name);

      // High-risk operations require confirmation
      if (riskLevel === "high" || riskLevel === "critical") {
        if (config.onConfirmationRequired) {
          const approved = await config.onConfirmationRequired(
            context.toolCall.name,
            context.args,
            riskLevel,
          );
          if (!approved) {
            return {
              block: true,
              reason: `User cancelled ${context.toolCall.name} operation.`,
            };
          }
        }
      }

      return undefined;
    },
  };

  return new Agent(agentOptions);
}

/**
 * Helper to create a user message for sending to the agent.
 */
export function createUserMessage(text: string): UserMessage {
  return {
    role: "user",
    content: text,
    timestamp: Date.now(),
  };
}

// Re-export types for convenience
export type { Agent, AgentEvent, AgentMessage };
