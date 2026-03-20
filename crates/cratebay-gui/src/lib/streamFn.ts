/**
 * CrateBay LLM Stream Function.
 *
 * Custom StreamFn that routes LLM calls through the Tauri/Rust backend.
 * API keys never leave the backend process.
 *
 * Flow: pi-agent-core → streamFn → Tauri invoke → Rust llm_proxy_stream
 *       → Rust emits Tauri events → streamFn yields AssistantMessageEvents
 */

import {
  type AssistantMessage,
  type Context,
  type Model,
  type SimpleStreamOptions,
  createAssistantMessageEventStream,
} from "@mariozechner/pi-ai";
import type { StreamFn } from "@mariozechner/pi-agent-core";
import type { LlmStreamEvent } from "@/types/agent";
import { invoke, listen } from "@/lib/tauri";

/**
 * Create a StreamFn that proxies LLM calls through the Tauri backend.
 *
 * @param providerId - The provider ID configured in settings
 * @param modelId - Override model ID (if different from model.id)
 */
export function createStreamFn(providerId: string, modelId?: string): StreamFn {
  return function streamFn(
    model: Model<string>,
    context: Context,
    options?: SimpleStreamOptions,
  ) {
    const stream = createAssistantMessageEventStream();
    const channelId = crypto.randomUUID();
    const resolvedModelId = modelId ?? model.id;

    // Convert context messages to the ChatMessage format expected by Rust
    const messages = [
      // System prompt as a system message
      ...(context.systemPrompt
        ? [{ role: "system", content: context.systemPrompt }]
        : []),
      // Conversation messages
      ...context.messages.map((msg) => {
        if (msg.role === "user") {
          const content = typeof msg.content === "string"
            ? msg.content
            : msg.content
                .filter((c): c is { type: "text"; text: string } => c.type === "text")
                .map((c) => c.text)
                .join("\n");
          return { role: "user" as const, content };
        }
        if (msg.role === "assistant") {
          const textParts = msg.content
            .filter((c): c is { type: "text"; text: string } => c.type === "text")
            .map((c) => c.text);
          const toolCalls = msg.content
            .filter((c): c is { type: "toolCall"; id: string; name: string; arguments: Record<string, unknown> } =>
              c.type === "toolCall",
            )
            .map((tc) => ({
              id: tc.id,
              type: "function" as const,
              function: { name: tc.name, arguments: JSON.stringify(tc.arguments) },
            }));
          return {
            role: "assistant" as const,
            content: textParts.join("\n"),
            tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
          };
        }
        if (msg.role === "toolResult") {
          const textParts = msg.content
            .filter((c): c is { type: "text"; text: string } => c.type === "text")
            .map((c) => c.text);
          return {
            role: "tool" as const,
            content: textParts.join("\n"),
            tool_call_id: msg.toolCallId,
          };
        }
        return { role: "user" as const, content: "" };
      }),
    ];

    // Build tool definitions for the Rust backend
    const toolDefs = context.tools?.map((t) => ({
      type: "function" as const,
      function: {
        name: t.name,
        description: t.description,
        parameters: t.parameters,
      },
    }));

    // Build LLM options
    const llmOptions: Record<string, unknown> = {};
    if (options?.temperature !== undefined) {
      llmOptions.temperature = options.temperature;
    }
    if (options?.maxTokens !== undefined) {
      llmOptions.max_tokens = options.maxTokens;
    }
    if (options?.reasoning !== undefined) {
      llmOptions.reasoning_effort = options.reasoning;
    }
    if (toolDefs !== undefined && toolDefs.length > 0) {
      llmOptions.tools = toolDefs;
    }

    // Track accumulated state for the partial AssistantMessage
    let accumulatedText = "";
    const accumulatedToolCalls: Array<{
      id: string;
      name: string;
      arguments: Record<string, unknown>;
    }> = [];

    function buildPartialMessage(): AssistantMessage {
      const content: AssistantMessage["content"] = [];
      if (accumulatedText.length > 0) {
        content.push({ type: "text", text: accumulatedText });
      }
      for (const tc of accumulatedToolCalls) {
        content.push({
          type: "toolCall",
          id: tc.id,
          name: tc.name,
          arguments: tc.arguments,
        });
      }
      return {
        role: "assistant",
        content,
        api: model.api,
        provider: model.provider,
        model: resolvedModelId,
        usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } },
        stopReason: "stop",
        timestamp: Date.now(),
      };
    }

    // Start async processing
    void (async () => {
      let unlisten: (() => void) | undefined;

      try {
        // Set up Tauri event listener before starting the stream
        unlisten = await listen<LlmStreamEvent>(
          `llm:stream:${channelId}`,
          (event) => {
            switch (event.type) {
              case "Token": {
                if (accumulatedText.length === 0) {
                  // First text token → emit text_start then start
                  const partial = buildPartialMessage();
                  stream.push({ type: "start", partial });
                  stream.push({ type: "text_start", contentIndex: 0, partial });
                }
                accumulatedText += event.content;
                const partial = buildPartialMessage();
                stream.push({
                  type: "text_delta",
                  contentIndex: 0,
                  delta: event.content,
                  partial,
                });
                break;
              }
              case "ToolCall": {
                // Parse tool call arguments
                let parsedArgs: Record<string, unknown> = {};
                try {
                  parsedArgs = JSON.parse(event.arguments) as Record<string, unknown>;
                } catch {
                  // Keep empty parsedArgs on parse failure
                }

                const toolCall = {
                  id: event.id,
                  name: event.name,
                  arguments: parsedArgs,
                };
                accumulatedToolCalls.push(toolCall);

                const contentIndex = (accumulatedText.length > 0 ? 1 : 0) + accumulatedToolCalls.length - 1;
                const partial = buildPartialMessage();

                stream.push({ type: "toolcall_start", contentIndex, partial });
                stream.push({
                  type: "toolcall_end",
                  contentIndex,
                  toolCall: {
                    type: "toolCall",
                    id: toolCall.id,
                    name: toolCall.name,
                    arguments: toolCall.arguments,
                  },
                  partial,
                });
                break;
              }
              case "Done": {
                // Emit text_end if we had text content
                if (accumulatedText.length > 0) {
                  const partial = buildPartialMessage();
                  stream.push({
                    type: "text_end",
                    contentIndex: 0,
                    content: accumulatedText,
                    partial,
                  });
                }

                // Build final message with usage stats
                const finalMessage = buildPartialMessage();
                finalMessage.usage = {
                  input: event.usage.prompt_tokens,
                  output: event.usage.completion_tokens,
                  cacheRead: 0,
                  cacheWrite: 0,
                  totalTokens: event.usage.total_tokens,
                  cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
                };
                finalMessage.stopReason = accumulatedToolCalls.length > 0 ? "toolUse" : "stop";

                stream.push({ type: "done", reason: finalMessage.stopReason as "stop" | "toolUse", message: finalMessage });
                break;
              }
              case "Error": {
                const errorMessage = buildPartialMessage();
                errorMessage.stopReason = "error";
                errorMessage.errorMessage = event.message;
                stream.push({ type: "error", reason: "error", error: errorMessage });
                break;
              }
            }
          },
        );

        // Invoke the Tauri command to start streaming
        await invoke("llm_proxy_stream", {
          channel_id: channelId,
          provider_id: providerId,
          model_id: resolvedModelId,
          messages,
          options: Object.keys(llmOptions).length > 0 ? llmOptions : undefined,
        });
      } catch (err) {
        // Emit error if invoke itself fails
        const errorMessage = buildPartialMessage();
        errorMessage.stopReason = "error";
        errorMessage.errorMessage = String(err);

        // Ensure we emit start if we haven't yet
        if (accumulatedText.length === 0 && accumulatedToolCalls.length === 0) {
          stream.push({ type: "start", partial: errorMessage });
        }
        stream.push({ type: "error", reason: "error", error: errorMessage });
      } finally {
        // Cleanup is handled after the stream ends via unlisten
        // We store it for potential abort cleanup
        if (options?.signal) {
          options.signal.addEventListener("abort", () => {
            unlisten?.();
            void invoke("llm_proxy_cancel", { channel_id: channelId }).catch(() => {
              // Ignore cancel errors
            });
          });
        }
      }
    })();

    return stream;
  };
}
