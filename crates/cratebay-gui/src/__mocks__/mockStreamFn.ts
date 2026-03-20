/**
 * Mock StreamFn for deterministic Agent testing.
 *
 * Implements testing-spec §4.1: provides a fake streamFn that yields
 * pre-programmed text tokens and tool calls without hitting a real LLM.
 *
 * Usage:
 *   const streamFn = createMockStreamFn([
 *     { content: "Hello!", toolCalls: [{ name: "container_list", arguments: {} }] },
 *     { content: "Here are your containers." },
 *   ]);
 */

import { createAssistantMessageEventStream } from "@mariozechner/pi-ai";
import type {
  AssistantMessage,
  Context,
  Model,
  SimpleStreamOptions,
  Usage,
} from "@mariozechner/pi-ai";
import type { StreamFn } from "@mariozechner/pi-agent-core";

/**
 * A single pre-programmed response for the mock stream.
 */
export interface MockResponse {
  /** Text content to stream (optional — can be tool-only) */
  content?: string;
  /** Tool calls to emit after text content */
  toolCalls?: Array<{
    id?: string;
    name: string;
    arguments: Record<string, unknown>;
  }>;
  /** Simulate an error response */
  error?: string;
}

/**
 * Internal counter for generating unique tool call IDs.
 */
let toolCallIdCounter = 0;

function resetToolCallIdCounter(): void {
  toolCallIdCounter = 0;
}

function nextToolCallId(): string {
  return `tc-mock-${++toolCallIdCounter}`;
}

/**
 * Build a minimal Usage object.
 */
function emptyUsage(): Usage {
  return {
    input: 0,
    output: 0,
    cacheRead: 0,
    cacheWrite: 0,
    totalTokens: 0,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
  };
}

/**
 * Create a deterministic StreamFn that returns pre-programmed responses.
 *
 * Each call to the returned function yields the next response in the array.
 * After exhausting all responses, it wraps around to the beginning.
 *
 * @param responses - Array of MockResponse objects in call order
 * @returns A StreamFn compatible with pi-agent-core's Agent
 */
export function createMockStreamFn(responses: MockResponse[]): StreamFn {
  let responseIndex = 0;
  resetToolCallIdCounter();

  const mockStreamFn: StreamFn = (
    model: Model<string>,
    context: Context,
    _options?: SimpleStreamOptions,
  ) => {
    const stream = createAssistantMessageEventStream();
    const response = responses[responseIndex % responses.length];
    responseIndex++;

    // Build the stream asynchronously
    void (async () => {
      try {
        // Handle error response
        if (response.error) {
          const errorMsg: AssistantMessage = {
            role: "assistant",
            content: [{ type: "text", text: response.error }],
            api: model.api,
            provider: model.provider,
            model: model.id,
            usage: emptyUsage(),
            stopReason: "error",
            errorMessage: response.error,
            timestamp: Date.now(),
          };
          stream.push({ type: "start", partial: errorMsg });
          stream.push({ type: "error", reason: "error", error: errorMsg });
          return;
        }

        const contentParts: AssistantMessage["content"] = [];
        const hasText = response.content !== undefined && response.content.length > 0;
        const toolCalls = response.toolCalls ?? [];

        // Build content array for partial messages
        if (hasText) {
          contentParts.push({ type: "text", text: "" });
        }

        function buildPartial(overrides?: Partial<AssistantMessage>): AssistantMessage {
          return {
            role: "assistant",
            content: [...contentParts],
            api: model.api,
            provider: model.provider,
            model: model.id,
            usage: emptyUsage(),
            stopReason: "stop",
            timestamp: Date.now(),
            ...overrides,
          };
        }

        // Emit start
        stream.push({ type: "start", partial: buildPartial() });

        // Stream text content character by character
        if (hasText && response.content) {
          stream.push({
            type: "text_start",
            contentIndex: 0,
            partial: buildPartial(),
          });

          let accumulated = "";
          for (const char of response.content) {
            accumulated += char;
            contentParts[0] = { type: "text", text: accumulated };
            stream.push({
              type: "text_delta",
              contentIndex: 0,
              delta: char,
              partial: buildPartial(),
            });
          }

          stream.push({
            type: "text_end",
            contentIndex: 0,
            content: accumulated,
            partial: buildPartial(),
          });
        }

        // Emit tool calls
        for (const tc of toolCalls) {
          const toolCallId = tc.id ?? nextToolCallId();
          const toolCall = {
            type: "toolCall" as const,
            id: toolCallId,
            name: tc.name,
            arguments: tc.arguments,
          };
          contentParts.push(toolCall);

          const contentIndex = contentParts.length - 1;

          stream.push({
            type: "toolcall_start",
            contentIndex,
            partial: buildPartial(),
          });
          stream.push({
            type: "toolcall_end",
            contentIndex,
            toolCall,
            partial: buildPartial(),
          });
        }

        // Emit done
        const stopReason = toolCalls.length > 0 ? "toolUse" : "stop";
        const finalMessage = buildPartial({ stopReason });
        stream.push({
          type: "done",
          reason: stopReason as "stop" | "toolUse",
          message: finalMessage,
        });
      } catch (err) {
        const errorMsg = buildErrorMessage(model, String(err));
        stream.push({ type: "error", reason: "error", error: errorMsg });
      }
    })();

    return stream;
  };

  return mockStreamFn;
}

function buildErrorMessage(model: Model<string>, message: string): AssistantMessage {
  return {
    role: "assistant",
    content: [{ type: "text", text: message }],
    api: model.api,
    provider: model.provider,
    model: model.id,
    usage: emptyUsage(),
    stopReason: "error",
    errorMessage: message,
    timestamp: Date.now(),
  };
}

/**
 * Create a mock Model object for testing.
 */
export function createMockModel(): Model<string> {
  return {
    id: "mock-model",
    name: "Mock Model",
    api: "openai-completions",
    provider: "mock",
    baseUrl: "http://localhost:0",
    reasoning: false,
    input: ["text"],
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
    contextWindow: 128000,
    maxTokens: 4096,
  };
}

/**
 * Spy helper: records which responses were consumed and the contexts passed.
 */
export interface MockStreamFnSpy {
  streamFn: StreamFn;
  calls: Array<{
    model: Model<string>;
    context: Context;
    responseIndex: number;
  }>;
}

export function createMockStreamFnWithSpy(responses: MockResponse[]): MockStreamFnSpy {
  const spy: MockStreamFnSpy = {
    streamFn: null as unknown as StreamFn,
    calls: [],
  };

  let responseIndex = 0;
  resetToolCallIdCounter();

  spy.streamFn = (
    model: Model<string>,
    context: Context,
    options?: SimpleStreamOptions,
  ) => {
    const currentIndex = responseIndex;
    spy.calls.push({ model, context, responseIndex: currentIndex });

    // Delegate to a one-shot mock
    const innerFn = createMockStreamFn([responses[currentIndex % responses.length]]);
    responseIndex++;
    return innerFn(model, context, options);
  };

  return spy;
}
