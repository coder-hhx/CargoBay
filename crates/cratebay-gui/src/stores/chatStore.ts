import { create } from "zustand";
import { invoke } from "@/lib/tauri";

/** Stable empty reference to avoid re-renders from Zustand selectors */
const EMPTY_MESSAGES: never[] = [];

interface ChatSession {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
  lastMessagePreview?: string;
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

/**
 * Backend response type for conversation_list.
 * Matches api-spec.md ConversationSummary.
 */
interface ConversationSummary {
  id: string;
  title: string;
  message_count: number;
  created_at: string;
  updated_at: string;
  last_message_preview: string | null;
}

/**
 * Backend response type for conversation_create / conversation_get_messages.
 * Matches api-spec.md ConversationDetail.
 */
interface ConversationDetail {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  messages: BackendChatMessage[];
}

/**
 * Backend ChatMessage shape (snake_case).
 * Matches api-spec.md ChatMessage.
 */
interface BackendChatMessage {
  role: string;
  content: string;
  tool_calls?: { id: string; name: string; arguments: string }[];
  tool_call_id?: string;
}

/**
 * Backend SaveMessageRequest shape.
 * Matches api-spec.md SaveMessageRequest.
 */
interface SaveMessageRequest {
  role: string;
  content: string;
  tool_calls?: { id: string; name: string; arguments: string }[];
  tool_call_id?: string;
  model?: string;
  provider_id?: string;
}

interface ChatState {
  // Sessions
  sessions: ChatSession[];
  activeSessionId: string | null;
  sessionsLoaded: boolean;
  loadSessions: () => Promise<void>;
  createSession: () => Promise<ChatSession>;
  deleteSession: (id: string) => Promise<void>;
  setActiveSession: (id: string) => Promise<void>;
  updateSessionTitle: (id: string, title: string) => Promise<void>;

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

/**
 * Convert a backend ConversationSummary to our frontend ChatSession type.
 */
function summaryToSession(s: ConversationSummary): ChatSession {
  return {
    id: s.id,
    title: s.title,
    createdAt: s.created_at,
    updatedAt: s.updated_at,
    messageCount: s.message_count,
    lastMessagePreview: s.last_message_preview ?? undefined,
  };
}

/**
 * Convert a backend ChatMessage to our frontend ChatMessage type.
 */
function backendToFrontendMessage(msg: BackendChatMessage, sessionId: string, index: number): ChatMessage {
  return {
    id: `persisted-${index}-${Date.now()}`,
    sessionId,
    role: msg.role as ChatMessage["role"],
    content: msg.content,
    timestamp: new Date().toISOString(),
    status: "complete",
    toolCalls: msg.tool_calls?.map((tc) => ({
      id: tc.id,
      toolName: tc.name,
      toolLabel: tc.name,
      parameters: JSON.parse(tc.arguments || "{}") as Record<string, unknown>,
      status: "success" as const,
    })),
  };
}

/**
 * Build a SaveMessageRequest from a frontend ChatMessage.
 */
function frontendToSaveRequest(msg: ChatMessage): SaveMessageRequest {
  const req: SaveMessageRequest = {
    role: msg.role,
    content: msg.content,
  };
  if (msg.toolCalls && msg.toolCalls.length > 0) {
    req.tool_calls = msg.toolCalls.map((tc) => ({
      id: tc.id,
      name: tc.toolName,
      arguments: JSON.stringify(tc.parameters),
    }));
  }
  if (msg.metadata?.toolCallId) {
    req.tool_call_id = msg.metadata.toolCallId as string;
  }
  if (msg.metadata?.model) {
    req.model = msg.metadata.model as string;
  }
  if (msg.metadata?.providerId) {
    req.provider_id = msg.metadata.providerId as string;
  }
  return req;
}

export const useChatStore = create<ChatState>()((set, get) => ({
  // Sessions
  sessions: [],
  activeSessionId: null,
  sessionsLoaded: false,

  loadSessions: async () => {
    try {
      const summaries = await invoke<ConversationSummary[]>("conversation_list", {
        limit: 100,
        offset: 0,
      });
      const sessions = summaries.map(summaryToSession);
      const firstId = sessions.length > 0 ? sessions[0].id : null;
      set({ sessions, sessionsLoaded: true, activeSessionId: firstId });
      // Load messages for the first session
      if (firstId) {
        await get().setActiveSession(firstId);
      }
    } catch {
      // Non-Tauri env: mark as loaded with empty list
      set({ sessionsLoaded: true });
    }
  },

  createSession: async () => {
    try {
      const detail = await invoke<ConversationDetail>("conversation_create", {
        title: "New Chat",
      });
      const session: ChatSession = {
        id: detail.id,
        title: detail.title,
        createdAt: detail.created_at,
        updatedAt: detail.updated_at,
        messageCount: 0,
      };
      set((state) => ({
        sessions: [session, ...state.sessions],
        activeSessionId: session.id,
        messages: { ...state.messages, [session.id]: [] },
      }));
      return session;
    } catch {
      // Fallback for non-Tauri development
      const session: ChatSession = {
        id: `session-${++sessionCounter}-${Date.now()}`,
        title: "New Chat",
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        messageCount: 0,
      };
      set((state) => ({
        sessions: [session, ...state.sessions],
        activeSessionId: session.id,
        messages: { ...state.messages, [session.id]: [] },
      }));
      return session;
    }
  },

  deleteSession: async (id) => {
    // Optimistically remove from UI
    set((state) => {
      const { [id]: _, ...remainingMessages } = state.messages;
      void _;
      return {
        sessions: state.sessions.filter((s) => s.id !== id),
        activeSessionId: state.activeSessionId === id ? null : state.activeSessionId,
        messages: remainingMessages,
      };
    });
    try {
      await invoke("conversation_delete", { id });
    } catch {
      // Non-Tauri env or backend error — UI already updated
    }
  },

  setActiveSession: async (id) => {
    set({ activeSessionId: id });

    // Load messages from backend if not already in memory
    const existing = get().messages[id];
    if (existing && existing.length > 0) return;

    try {
      const detail = await invoke<ConversationDetail>("conversation_get_messages", { id });
      const messages = detail.messages.map((msg, i) => backendToFrontendMessage(msg, id, i));
      set((state) => ({
        messages: { ...state.messages, [id]: messages },
      }));
    } catch {
      // Non-Tauri env — initialize empty message list
      set((state) => ({
        messages: { ...state.messages, [id]: state.messages[id] ?? [] },
      }));
    }
  },

  updateSessionTitle: async (id, title) => {
    // Optimistically update UI
    set((state) => ({
      sessions: state.sessions.map((s) =>
        s.id === id ? { ...s, title, updatedAt: new Date().toISOString() } : s,
      ),
    }));
    try {
      await invoke("conversation_update_title", { sessionId: id, title });
    } catch {
      // Non-Tauri env — UI already updated
    }
  },

  // Messages
  messages: {},
  addMessage: (sessionId, message) => {
    // Synchronously update UI for responsiveness
    set((state) => ({
      messages: {
        ...state.messages,
        [sessionId]: [...(state.messages[sessionId] ?? EMPTY_MESSAGES), message],
      },
    }));

    // Asynchronously persist to backend (fire-and-forget)
    void invoke("conversation_save_message", {
      sessionId,
      message: frontendToSaveRequest(message),
    }).catch(() => {
      // Non-Tauri env or backend error — message still visible in UI
    });
  },

  updateMessage: (sessionId, messageId, patch) =>
    set((state) => ({
      messages: {
        ...state.messages,
        [sessionId]: (state.messages[sessionId] ?? EMPTY_MESSAGES).map((m) =>
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
        [sessionId]: (state.messages[sessionId] ?? EMPTY_MESSAGES).map((m) =>
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
