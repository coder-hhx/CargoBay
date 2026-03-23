/**
 * useAgent — Agent lifecycle management hook.
 *
 * Creates and manages a CrateBay Agent instance, subscribing to agent events
 * and bridging them to the chatStore and workflowStore.
 */

import { useEffect, useRef, useCallback } from "react";
import type { Agent, AgentEvent } from "@mariozechner/pi-agent-core";
import type { Model } from "@mariozechner/pi-ai";
import { createCrateBayAgent, createUserMessage } from "@/lib/agent";
import { useChatStore } from "@/stores/chatStore";
import { useWorkflowStore } from "@/stores/workflowStore";
import { getToolLabel, getToolRiskLevel } from "@/tools";
import { useMcpToolSync } from "@/hooks/useMcpToolSync";

interface UseAgentOptions {
  /** LLM provider ID */
  providerId: string | null;
  /** LLM model */
  model: Model<string> | null;
  /** Callback when confirmation is needed for destructive operations */
  onConfirmationRequired?: (
    toolName: string,
    args: unknown,
    riskLevel: string,
  ) => Promise<boolean>;
}

interface UseAgentReturn {
  /** Send a user message to the agent */
  sendMessage: (text: string) => Promise<void>;
  /** Abort the current agent run */
  abort: () => void;
  /** Whether the agent is currently processing */
  isRunning: boolean;
}

/**
 * Hook that manages the Agent lifecycle and bridges events to Zustand stores.
 *
 * Usage:
 * ```tsx
 * const { sendMessage, abort, isRunning } = useAgent({
 *   providerId: "openai",
 *   model: selectedModel,
 * });
 *
 * // Send a message
 * await sendMessage("Create a Node.js container");
 *
 * // Abort
 * abort();
 * ```
 */
export function useAgent(options: UseAgentOptions): UseAgentReturn {
  const { providerId, model, onConfirmationRequired } = options;
  const agentRef = useRef<Agent | null>(null);
  const isRunningRef = useRef(false);

  // Store references (avoid re-subscribing on every render)
  const chatStoreRef = useRef(useChatStore.getState());
  const workflowStoreRef = useRef(useWorkflowStore.getState());

  useEffect(() => {
    const unsubChat = useChatStore.subscribe((state) => {
      chatStoreRef.current = state;
    });
    const unsubWorkflow = useWorkflowStore.subscribe((state) => {
      workflowStoreRef.current = state;
    });
    return () => {
      unsubChat();
      unsubWorkflow();
    };
  }, []);

  // Create/recreate agent when provider or model changes
  useEffect(() => {
    if (!providerId || !model) {
      agentRef.current = null;
      return;
    }

    const agent = createCrateBayAgent({
      providerId,
      model,
      onConfirmationRequired,
    });

    // Subscribe to agent events and bridge to stores
    const unsubscribe = agent.subscribe((event: AgentEvent) => {
      const chat = chatStoreRef.current;
      const workflow = workflowStoreRef.current;
      const sessionId = chat.activeSessionId;
      if (!sessionId) return;

      switch (event.type) {
        case "agent_start": {
          workflow.setAgentStatus("thinking");
          chat.setAgentThinking(true);
          break;
        }

        case "message_start": {
          // Create a placeholder assistant message for streaming
          const msgId = `msg-${Date.now()}`;
          chat.addMessage(sessionId, {
            id: msgId,
            sessionId,
            role: "assistant",
            content: "",
            timestamp: new Date().toISOString(),
            status: "streaming",
          });
          chat.setStreaming(true, msgId);
          break;
        }

        case "message_update": {
          // Update streaming content
          if (event.assistantMessageEvent.type === "text_delta") {
            const streamingId = chat.streamingMessageId;
            if (streamingId) {
              chat.appendStreamChunk(sessionId, streamingId, event.assistantMessageEvent.delta);
            }
          }
          if (event.assistantMessageEvent.type === "thinking_delta") {
            chat.setAgentThinking(true, event.assistantMessageEvent.delta);
          }
          break;
        }

        case "message_end": {
          const streamingId = chat.streamingMessageId;
          if (streamingId) {
            chat.updateMessage(sessionId, streamingId, { status: "complete" });
          }
          chat.setStreaming(false);
          chat.setAgentThinking(false);
          break;
        }

        case "tool_execution_start": {
          workflow.setAgentStatus("executing");
          workflow.setCurrentToolExecution({
            id: event.toolCallId,
            toolName: event.toolName,
            toolLabel: getToolLabel(event.toolName),
            parameters: event.args as Record<string, unknown>,
            status: "running",
            startedAt: new Date().toISOString(),
            riskLevel: getToolRiskLevel(event.toolName),
          });
          break;
        }

        case "tool_execution_end": {
          workflow.setCurrentToolExecution(null);
          workflow.addToHistory({
            id: event.toolCallId,
            toolName: event.toolName,
            toolLabel: getToolLabel(event.toolName),
            parameters: {},
            status: event.isError ? "error" : "success",
            result: event.result,
            startedAt: new Date().toISOString(),
            completedAt: new Date().toISOString(),
            riskLevel: getToolRiskLevel(event.toolName),
          });
          break;
        }

        case "agent_end": {
          workflow.setAgentStatus("idle");
          chat.setAgentThinking(false);
          chat.setStreaming(false);
          isRunningRef.current = false;
          break;
        }
      }
    });

    agentRef.current = agent;

    return () => {
      unsubscribe();
      agent.abort();
      agentRef.current = null;
    };
  }, [providerId, model, onConfirmationRequired]);

  // Sync MCP tools dynamically when servers connect/disconnect
  useMcpToolSync(agentRef.current);

  const sendMessage = useCallback(
    async (text: string) => {
      const chat = chatStoreRef.current;
      const sessionId = chat.activeSessionId;
      if (!sessionId) return;
      if (isRunningRef.current) return;

      // Add user message to store
      chat.addMessage(sessionId, {
        id: `msg-${Date.now()}`,
        sessionId,
        role: "user",
        content: text,
        timestamp: new Date().toISOString(),
        status: "complete",
      });

      const agent = agentRef.current;
      if (!agent) {
        chat.addMessage(sessionId, {
          id: `msg-${Date.now()}-assistant`,
          sessionId,
          role: "assistant",
          content:
            "Please configure an LLM provider and model in Settings before sending messages.",
          timestamp: new Date().toISOString(),
          status: "complete",
        });
        return;
      }

      isRunningRef.current = true;

      try {
        await agent.prompt(createUserMessage(text));
      } catch (err) {
        console.error("[Agent] Prompt failed:", err);
        workflowStoreRef.current.setAgentStatus("error");
        isRunningRef.current = false;
      }
    },
    [],
  );

  const abort = useCallback(() => {
    agentRef.current?.abort();
    isRunningRef.current = false;
    workflowStoreRef.current.setAgentStatus("idle");
    chatStoreRef.current.setStreaming(false);
    chatStoreRef.current.setAgentThinking(false);
  }, []);

  return {
    sendMessage,
    abort,
    isRunning: isRunningRef.current,
  };
}
