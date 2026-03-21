/**
 * ChatPage — Default page: Chat-First AI interface.
 *
 * Integrates pi-agent-core via useAgent hook:
 * - Initializes agent with streamFn, tools, system prompt
 * - User messages → chatStore.addMessage → agent.processMessage
 * - Subscribes to agent events for streaming, tool calls, workflow state
 * - Renders ConfirmDialog when workflowStore.pendingConfirmation exists
 *
 * @see agent-spec.md §2 for Agent integration details
 * @see frontend-spec.md §5.1 for page layout specification
 */

import { useCallback, useEffect, useMemo } from "react";
import { MessageList } from "@/components/chat/MessageList";
import { ChatInput } from "@/components/chat/ChatInput";
import { ConfirmDialog } from "@/components/chat/ConfirmDialog";
import { useAgent } from "@/hooks/useAgent";
import { useChatStore } from "@/stores/chatStore";
import { useWorkflowStore } from "@/stores/workflowStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type { Model, Api } from "@mariozechner/pi-ai";
import type { ApiFormat } from "@/types/settings";

/**
 * Map CrateBay's ApiFormat to pi-ai's Api type string.
 */
function apiFormatToApi(format: ApiFormat): Api {
  switch (format) {
    case "anthropic":
      return "anthropic-messages";
    case "openai_responses":
      return "openai-responses";
    case "openai_completions":
      return "openai-completions";
  }
}

/**
 * Build a pi-ai Model object from settings store data.
 * Returns null if required information is missing.
 */
function buildModel(
  activeModelId: string | null,
  activeProviderId: string | null,
  providers: { id: string; name: string; apiBase: string; apiFormat: ApiFormat }[],
  models: Record<string, { id: string; name: string; supportsReasoning: boolean }[]>,
): Model<Api> | null {
  if (activeModelId === null || activeProviderId === null) return null;

  const provider = providers.find((p) => p.id === activeProviderId);
  if (provider === undefined) return null;

  const providerModels = models[activeProviderId];
  if (providerModels === undefined) return null;

  const modelInfo = providerModels.find((m) => m.id === activeModelId);
  if (modelInfo === undefined) return null;

  return {
    id: modelInfo.id,
    name: modelInfo.name,
    api: apiFormatToApi(provider.apiFormat),
    provider: provider.name.toLowerCase(),
    baseUrl: provider.apiBase,
    reasoning: modelInfo.supportsReasoning,
    input: ["text"],
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
    contextWindow: 128000,
    maxTokens: 4096,
  };
}

export function ChatPage() {
  // Chat store
  const activeSessionId = useChatStore((s) => s.activeSessionId);
  const createSession = useChatStore((s) => s.createSession);
  const loadSessions = useChatStore((s) => s.loadSessions);
  const sessionsLoaded = useChatStore((s) => s.sessionsLoaded);
  const isStreaming = useChatStore((s) => s.isStreaming);

  // Settings store
  const activeProviderId = useSettingsStore((s) => s.activeProviderId);
  const activeModelId = useSettingsStore((s) => s.activeModelId);
  const providers = useSettingsStore((s) => s.providers);
  const models = useSettingsStore((s) => s.models);
  const confirmDestructiveOps = useSettingsStore((s) => s.settings.confirmDestructiveOps);

  // Workflow store (for confirmation dialog)
  const pendingConfirmation = useWorkflowStore((s) => s.pendingConfirmation);
  const resolveConfirmation = useWorkflowStore((s) => s.resolveConfirmation);

  // Build the pi-ai Model from settings
  const model = useMemo(
    () => buildModel(activeModelId, activeProviderId, providers, models),
    [activeModelId, activeProviderId, providers, models],
  );

  // Confirmation callback for destructive operations
  const handleConfirmationRequired = useCallback(
    async (toolName: string, args: unknown, riskLevel: string): Promise<boolean> => {
      // Only ask for confirmation if the setting is enabled, or if risk is high/critical
      if (riskLevel !== "high" && riskLevel !== "critical" && !confirmDestructiveOps) {
        return true;
      }

      const workflow = useWorkflowStore.getState();
      return workflow.requestConfirmation({
        toolName,
        toolLabel: toolName,
        description: `The agent wants to execute ${toolName} with the provided parameters.`,
        riskLevel: riskLevel as "medium" | "high" | "critical",
        parameters: typeof args === "object" && args !== null ? (args as Record<string, unknown>) : {},
        consequences: riskLevel === "high" || riskLevel === "critical"
          ? [`This is a ${riskLevel}-risk operation that may have significant effects.`]
          : [],
      });
    },
    [confirmDestructiveOps],
  );

  // Initialize agent via hook
  const { sendMessage, abort, isRunning } = useAgent({
    providerId: activeProviderId,
    model,
    onConfirmationRequired: handleConfirmationRequired,
  });

  // Load persisted sessions on mount
  useEffect(() => {
    if (!sessionsLoaded) {
      void loadSessions();
    }
  }, [sessionsLoaded, loadSessions]);

  // Auto-create a session if none exists (after sessions are loaded)
  useEffect(() => {
    if (sessionsLoaded && activeSessionId === null) {
      void createSession();
    }
  }, [sessionsLoaded, activeSessionId, createSession]);

  // Handle sending messages from ChatInput
  const handleSendMessage = useCallback(
    (text: string) => {
      void sendMessage(text);
    },
    [sendMessage],
  );

  // Handle stop/abort
  const handleStop = useCallback(() => {
    abort();
  }, [abort]);

  return (
    <div className="flex h-full flex-col">
      <MessageList />
      <ChatInput
        onSend={handleSendMessage}
        onStop={handleStop}
        disabled={isRunning || isStreaming}
      />

      {/* Confirmation dialog for destructive operations */}
      {pendingConfirmation !== null && (
        <ConfirmDialog
          request={pendingConfirmation}
          onConfirm={() => resolveConfirmation(pendingConfirmation.id, true)}
          onCancel={() => resolveConfirmation(pendingConfirmation.id, false)}
        />
      )}
    </div>
  );
}
