import { describe, it, expect, beforeEach, vi } from "vitest";
import { useAppStore } from "@/stores/appStore";
import { useChatStore } from "@/stores/chatStore";

// Mock the Tauri invoke wrapper so that invoke always throws,
// forcing settingsStore to use its mock/fallback code paths.
vi.mock("@/lib/tauri", () => ({
  invoke: vi.fn(() => Promise.reject(new Error("Tauri not available in test"))),
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

// Import settingsStore AFTER mocking @/lib/tauri
import { useSettingsStore } from "@/stores/settingsStore";

// ---------------------------------------------------------------------------
// appStore
// ---------------------------------------------------------------------------
describe("appStore", () => {
  beforeEach(() => {
    // Reset to initial state
    useAppStore.setState({
      currentPage: "chat",
      theme: "dark",
      sidebarOpen: true,
      sidebarWidth: 260,
      dockerConnected: false,
      runtimeStatus: "stopped",
      notifications: [],
    });
  });

  it("sets currentPage correctly", () => {
    useAppStore.getState().setCurrentPage("settings");
    expect(useAppStore.getState().currentPage).toBe("settings");

    useAppStore.getState().setCurrentPage("containers");
    expect(useAppStore.getState().currentPage).toBe("containers");
  });

  it("toggles theme between dark and light", () => {
    expect(useAppStore.getState().theme).toBe("dark");

    useAppStore.getState().toggleTheme();
    expect(useAppStore.getState().theme).toBe("light");

    useAppStore.getState().toggleTheme();
    expect(useAppStore.getState().theme).toBe("dark");
  });

  it("toggles sidebar open/close", () => {
    expect(useAppStore.getState().sidebarOpen).toBe(true);

    useAppStore.getState().toggleSidebar();
    expect(useAppStore.getState().sidebarOpen).toBe(false);

    useAppStore.getState().toggleSidebar();
    expect(useAppStore.getState().sidebarOpen).toBe(true);
  });

  it("sets sidebarWidth", () => {
    useAppStore.getState().setSidebarWidth(320);
    expect(useAppStore.getState().sidebarWidth).toBe(320);
  });

  it("sets docker connection status", () => {
    useAppStore.getState().setDockerConnected(true);
    expect(useAppStore.getState().dockerConnected).toBe(true);
  });

  it("sets runtime status", () => {
    useAppStore.getState().setRuntimeStatus("running");
    expect(useAppStore.getState().runtimeStatus).toBe("running");
  });

  it("adds and dismisses notifications", () => {
    useAppStore.getState().addNotification({
      type: "info",
      title: "Test notification",
      dismissable: true,
    });
    expect(useAppStore.getState().notifications).toHaveLength(1);
    expect(useAppStore.getState().notifications[0].title).toBe("Test notification");

    const id = useAppStore.getState().notifications[0].id;
    useAppStore.getState().dismissNotification(id);
    expect(useAppStore.getState().notifications).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// chatStore
// ---------------------------------------------------------------------------
describe("chatStore", () => {
  beforeEach(() => {
    useChatStore.setState({
      sessions: [],
      activeSessionId: null,
      sessionsLoaded: false,
      messages: {},
      isStreaming: false,
      streamingMessageId: null,
      agentThinking: false,
      agentThinkingContent: null,
      inputDraft: "",
    });
  });

  it("creates a new session", async () => {
    const session = await useChatStore.getState().createSession();
    expect(session.id).toBeDefined();
    expect(session.title).toBe("New Chat");
    expect(useChatStore.getState().sessions).toHaveLength(1);
    expect(useChatStore.getState().activeSessionId).toBe(session.id);
  });

  it("deletes a session and clears activeSessionId if needed", async () => {
    const session = await useChatStore.getState().createSession();
    expect(useChatStore.getState().activeSessionId).toBe(session.id);

    await useChatStore.getState().deleteSession(session.id);
    expect(useChatStore.getState().sessions).toHaveLength(0);
    expect(useChatStore.getState().activeSessionId).toBeNull();
  });

  it("adds messages to a session", async () => {
    const session = await useChatStore.getState().createSession();
    useChatStore.getState().addMessage(session.id, {
      id: "msg-1",
      sessionId: session.id,
      role: "user",
      content: "Hello",
      timestamp: new Date().toISOString(),
      status: "complete",
    });

    const msgs = useChatStore.getState().messages[session.id];
    expect(msgs).toHaveLength(1);
    expect(msgs[0].content).toBe("Hello");
  });

  it("updates an existing message", async () => {
    const session = await useChatStore.getState().createSession();
    useChatStore.getState().addMessage(session.id, {
      id: "msg-1",
      sessionId: session.id,
      role: "assistant",
      content: "Hi",
      timestamp: new Date().toISOString(),
      status: "streaming",
    });

    useChatStore.getState().updateMessage(session.id, "msg-1", {
      content: "Hi there!",
      status: "complete",
    });

    const msg = useChatStore.getState().messages[session.id][0];
    expect(msg.content).toBe("Hi there!");
    expect(msg.status).toBe("complete");
  });

  it("appends stream chunks to a message", async () => {
    const session = await useChatStore.getState().createSession();
    useChatStore.getState().addMessage(session.id, {
      id: "msg-1",
      sessionId: session.id,
      role: "assistant",
      content: "",
      timestamp: new Date().toISOString(),
      status: "streaming",
    });

    useChatStore.getState().appendStreamChunk(session.id, "msg-1", "Hello ");
    useChatStore.getState().appendStreamChunk(session.id, "msg-1", "world");

    const msg = useChatStore.getState().messages[session.id][0];
    expect(msg.content).toBe("Hello world");
  });

  it("sets streaming state", () => {
    useChatStore.getState().setStreaming(true, "msg-42");
    expect(useChatStore.getState().isStreaming).toBe(true);
    expect(useChatStore.getState().streamingMessageId).toBe("msg-42");

    useChatStore.getState().setStreaming(false);
    expect(useChatStore.getState().isStreaming).toBe(false);
    expect(useChatStore.getState().streamingMessageId).toBeNull();
  });

  it("sets input draft", () => {
    useChatStore.getState().setInputDraft("test input");
    expect(useChatStore.getState().inputDraft).toBe("test input");
  });

  it("sets agent thinking state", () => {
    useChatStore.getState().setAgentThinking(true, "Analyzing...");
    expect(useChatStore.getState().agentThinking).toBe(true);
    expect(useChatStore.getState().agentThinkingContent).toBe("Analyzing...");
  });
});

// ---------------------------------------------------------------------------
// settingsStore
// ---------------------------------------------------------------------------
describe("settingsStore", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      providers: [],
      activeProviderId: null,
      providersLoading: false,
      models: {},
      activeModelId: null,
      modelsLoading: {},
      settings: {
        language: "en",
        theme: "dark",
        sendOnEnter: true,
        showAgentThinking: true,
        maxConversationHistory: 50,
        containerDefaultTtlHours: 8,
        confirmDestructiveOps: true,
        reasoningEffort: "medium",
        registryMirrors: [],
        runtimeHttpProxy: "",
        runtimeHttpProxyBridge: false,
        runtimeHttpProxyBindHost: "0.0.0.0",
        runtimeHttpProxyBindPort: 3128,
        runtimeHttpProxyGuestHost: "192.168.64.1",
        allowExternalDocker: false,
      },
    });
  });

  it("creates a provider (mock fallback)", async () => {
    const provider = await useSettingsStore.getState().createProvider({
      name: "TestProvider",
      apiBase: "https://api.example.com",
      apiKey: "sk-test",
      apiFormat: "openai_completions",
    });

    expect(provider.name).toBe("TestProvider");
    expect(provider.apiBase).toBe("https://api.example.com");
    expect(provider.hasApiKey).toBe(true);
    expect(useSettingsStore.getState().providers).toHaveLength(1);
    expect(useSettingsStore.getState().activeProviderId).toBe(provider.id);
  });

  it("deletes a provider", async () => {
    const provider = await useSettingsStore.getState().createProvider({
      name: "ToDelete",
      apiBase: "https://api.example.com",
      apiKey: "",
      apiFormat: "anthropic",
    });
    expect(useSettingsStore.getState().providers).toHaveLength(1);

    await useSettingsStore.getState().deleteProvider(provider.id);
    expect(useSettingsStore.getState().providers).toHaveLength(0);
    expect(useSettingsStore.getState().activeProviderId).toBeNull();
  });

  it("updates settings", async () => {
    await useSettingsStore.getState().updateSettings({
      reasoningEffort: "high",
      sendOnEnter: false,
    });

    const settings = useSettingsStore.getState().settings;
    expect(settings.reasoningEffort).toBe("high");
    expect(settings.sendOnEnter).toBe(false);
    // Other settings should remain unchanged
    expect(settings.language).toBe("en");
  });

  it("sets active provider", () => {
    useSettingsStore.getState().setActiveProvider("provider-42");
    expect(useSettingsStore.getState().activeProviderId).toBe("provider-42");
  });

  it("sets active model", () => {
    useSettingsStore.getState().setActiveModel("model-1");
    expect(useSettingsStore.getState().activeModelId).toBe("model-1");
  });
});
