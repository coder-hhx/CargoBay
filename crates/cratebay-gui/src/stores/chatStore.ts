import { create } from "zustand";

interface ChatSession {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
}

interface ToolCallInfo {
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

interface ChatMessage {
  id: string;
  sessionId: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  timestamp: string;
  status: "sending" | "streaming" | "complete" | "error";
  toolCalls?: ToolCallInfo[];
  reasoning?: string;
  metadata?: Record<string, unknown>;
}

interface ChatState {
  // Sessions
  sessions: ChatSession[];
  activeSessionId: string | null;
  createSession: () => ChatSession;
  deleteSession: (id: string) => void;
  setActiveSession: (id: string) => void;

  // Messages
  messages: Record<string, ChatMessage[]>;
  addMessage: (sessionId: string, message: ChatMessage) => void;
  updateMessage: (sessionId: string, messageId: string, patch: Partial<ChatMessage>) => void;

  // Streaming
  isStreaming: boolean;
  streamingMessageId: string | null;
  appendStreamChunk: (sessionId: string, messageId: string, chunk: string) => void;
  setStreaming: (streaming: boolean, messageId?: string) => void;

  // Agent state
  agentThinking: boolean;
  agentThinkingContent: string | null;
  setAgentThinking: (thinking: boolean, content?: string) => void;

  // Input
  inputDraft: string;
  setInputDraft: (draft: string) => void;
  mentionQuery: string | null;
  setMentionQuery: (query: string | null) => void;
}

let sessionCounter = 0;

export const useChatStore = create<ChatState>()((set) => ({
  // Sessions
  sessions: [],
  activeSessionId: null,
  createSession: () => {
    const session: ChatSession = {
      id: `session-${++sessionCounter}-${Date.now()}`,
      title: "New Chat",
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      messageCount: 0,
    };
    set((state) => ({
      sessions: [...state.sessions, session],
      activeSessionId: session.id,
      messages: { ...state.messages, [session.id]: [] },
    }));
    return session;
  },
  deleteSession: (id) =>
    set((state) => {
      const { [id]: _, ...remainingMessages } = state.messages;
      void _;
      return {
        sessions: state.sessions.filter((s) => s.id !== id),
        activeSessionId: state.activeSessionId === id ? null : state.activeSessionId,
        messages: remainingMessages,
      };
    }),
  setActiveSession: (id) => set({ activeSessionId: id }),

  // Messages
  messages: {},
  addMessage: (sessionId, message) =>
    set((state) => ({
      messages: {
        ...state.messages,
        [sessionId]: [...(state.messages[sessionId] ?? []), message],
      },
    })),
  updateMessage: (sessionId, messageId, patch) =>
    set((state) => ({
      messages: {
        ...state.messages,
        [sessionId]: (state.messages[sessionId] ?? []).map((m) =>
          m.id === messageId ? { ...m, ...patch } : m,
        ),
      },
    })),

  // Streaming
  isStreaming: false,
  streamingMessageId: null,
  appendStreamChunk: (sessionId, messageId, chunk) =>
    set((state) => ({
      messages: {
        ...state.messages,
        [sessionId]: (state.messages[sessionId] ?? []).map((m) =>
          m.id === messageId ? { ...m, content: m.content + chunk } : m,
        ),
      },
    })),
  setStreaming: (streaming, messageId) =>
    set({
      isStreaming: streaming,
      streamingMessageId: messageId ?? null,
    }),

  // Agent state
  agentThinking: false,
  agentThinkingContent: null,
  setAgentThinking: (thinking, content) =>
    set({
      agentThinking: thinking,
      agentThinkingContent: content ?? null,
    }),

  // Input
  inputDraft: "",
  setInputDraft: (draft) => set({ inputDraft: draft }),
  mentionQuery: null,
  setMentionQuery: (query) => set({ mentionQuery: query }),
}));
