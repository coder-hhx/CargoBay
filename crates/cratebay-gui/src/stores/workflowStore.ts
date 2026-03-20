import { create } from "zustand";

export interface ToolExecution {
  id: string;
  toolName: string;
  toolLabel: string;
  parameters: Record<string, unknown>;
  status: "pending" | "running" | "success" | "error";
  result?: unknown;
  error?: string;
  startedAt: string;
  completedAt?: string;
  riskLevel: "low" | "medium" | "high" | "critical";
}

export interface ConfirmationRequest {
  id: string;
  toolName: string;
  toolLabel: string;
  description: string;
  riskLevel: "medium" | "high" | "critical";
  parameters: Record<string, unknown>;
  consequences: string[];
}

interface WorkflowState {
  // Agent lifecycle
  agentStatus: "idle" | "thinking" | "executing" | "waiting_confirmation" | "error";
  setAgentStatus: (status: WorkflowState["agentStatus"]) => void;

  // Current tool execution
  currentToolExecution: ToolExecution | null;
  setCurrentToolExecution: (exec: ToolExecution | null) => void;

  // Confirmation flow
  pendingConfirmation: ConfirmationRequest | null;
  requestConfirmation: (req: Omit<ConfirmationRequest, "id">) => Promise<boolean>;
  resolveConfirmation: (id: string, approved: boolean) => void;

  // Execution history (current session)
  executionHistory: ToolExecution[];
  addToHistory: (exec: ToolExecution) => void;
  clearHistory: () => void;

  // Follow-up suggestions
  followUpSuggestions: string[];
  setFollowUpSuggestions: (suggestions: string[]) => void;
}

let confirmationIdCounter = 0;

// Store pending confirmation resolvers outside of Zustand state
// (functions should not be serialized into state)
const pendingResolvers = new Map<string, (approved: boolean) => void>();

export const useWorkflowStore = create<WorkflowState>()((set) => ({
  agentStatus: "idle",
  setAgentStatus: (agentStatus) => set({ agentStatus }),

  currentToolExecution: null,
  setCurrentToolExecution: (exec) => set({ currentToolExecution: exec }),

  pendingConfirmation: null,

  requestConfirmation: (req) => {
    return new Promise<boolean>((resolve) => {
      const id = `confirm-${++confirmationIdCounter}-${Date.now()}`;
      const request: ConfirmationRequest = { ...req, id };
      pendingResolvers.set(id, resolve);
      set({
        pendingConfirmation: request,
        agentStatus: "waiting_confirmation",
      });
    });
  },

  resolveConfirmation: (id, approved) => {
    const resolver = pendingResolvers.get(id);
    if (resolver !== undefined) {
      resolver(approved);
      pendingResolvers.delete(id);
    }
    set({
      pendingConfirmation: null,
      agentStatus: "idle",
    });
  },

  executionHistory: [],
  addToHistory: (exec) =>
    set((state) => ({
      executionHistory: [...state.executionHistory, exec],
    })),
  clearHistory: () => set({ executionHistory: [] }),

  followUpSuggestions: [],
  setFollowUpSuggestions: (suggestions) => set({ followUpSuggestions: suggestions }),
}));
