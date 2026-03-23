import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ChatInput } from "@/components/chat/ChatInput";
import { MessageBubble } from "@/components/chat/MessageBubble";
import { useChatStore } from "@/stores/chatStore";
import { useAppStore } from "@/stores/appStore";

// Mock @tauri-apps/api to avoid native module errors
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ---------------------------------------------------------------------------
// ChatInput
// ---------------------------------------------------------------------------
describe("ChatInput", () => {
  beforeEach(() => {
    useChatStore.setState({
      sessions: [],
      activeSessionId: null,
      messages: {},
      isStreaming: false,
      streamingMessageId: null,
      inputDraft: "",
    });
    useAppStore.setState({
      sidebarOpen: true,
    });
  });

  it("renders textarea and send button", () => {
    render(<ChatInput />);

    expect(
      screen.getByPlaceholderText(/Type a message/),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Send message")).toBeInTheDocument();
  });

  it("updates input draft when typing", () => {
    render(<ChatInput />);
    const textarea = screen.getByPlaceholderText(/Type a message/);

    fireEvent.change(textarea, { target: { value: "Hello world" } });
    expect(useChatStore.getState().inputDraft).toBe("Hello world");
  });

  it("sends message on button click", () => {
    const handleSend = vi.fn();
    // Pre-fill input draft
    useChatStore.setState({ inputDraft: "Test message" });
    render(<ChatInput onSend={handleSend} />);

    const sendBtn = screen.getByLabelText("Send message");
    fireEvent.click(sendBtn);

    // onSend callback should be called with the trimmed message
    expect(handleSend).toHaveBeenCalledWith("Test message");
    // Input should be cleared after sending
    expect(useChatStore.getState().inputDraft).toBe("");
  });

  it("sends message on Enter key press", () => {
    const handleSend = vi.fn();
    useChatStore.setState({ inputDraft: "Enter test" });
    render(<ChatInput onSend={handleSend} />);

    const textarea = screen.getByPlaceholderText(/Type a message/);
    fireEvent.keyDown(textarea, { key: "Enter", shiftKey: false });

    expect(handleSend).toHaveBeenCalledWith("Enter test");
    expect(useChatStore.getState().inputDraft).toBe("");
  });

  it("does not send on Shift+Enter (allows new line)", () => {
    useChatStore.setState({ inputDraft: "Multi line" });
    render(<ChatInput />);

    const textarea = screen.getByPlaceholderText(/Type a message/);
    fireEvent.keyDown(textarea, { key: "Enter", shiftKey: true });

    // Draft should NOT be cleared (message not sent)
    expect(useChatStore.getState().inputDraft).toBe("Multi line");
  });

  it("disables send button when input is empty", () => {
    render(<ChatInput />);
    const sendBtn = screen.getByLabelText("Send message");
    expect(sendBtn).toBeDisabled();
  });

  it("shows stop button when streaming", () => {
    useChatStore.setState({ isStreaming: true, streamingMessageId: "msg-1" });
    render(<ChatInput />);
    expect(screen.getByLabelText("Stop generating")).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// MessageBubble
// ---------------------------------------------------------------------------
describe("MessageBubble", () => {
  it("renders user message content", () => {
    render(
      <MessageBubble
        message={{
          id: "msg-1",
          sessionId: "s-1",
          role: "user",
          content: "Hello there",
          timestamp: new Date().toISOString(),
          status: "complete",
        }}
      />,
    );

    expect(screen.getByText("Hello there")).toBeInTheDocument();
  });

  it("renders assistant message content", () => {
    render(
      <MessageBubble
        message={{
          id: "msg-2",
          sessionId: "s-1",
          role: "assistant",
          content: "Hi, how can I help?",
          timestamp: new Date().toISOString(),
          status: "complete",
        }}
      />,
    );

    expect(screen.getByText("Hi, how can I help?")).toBeInTheDocument();
  });

  it("shows error indicator for error status", () => {
    render(
      <MessageBubble
        message={{
          id: "msg-3",
          sessionId: "s-1",
          role: "user",
          content: "Broken message",
          timestamp: new Date().toISOString(),
          status: "error",
        }}
      />,
    );

    expect(screen.getByText("Failed to send message.")).toBeInTheDocument();
  });

  it("renders Streamdown in streaming mode for streaming assistant message", () => {
    const { container } = render(
      <MessageBubble
        message={{
          id: "msg-4",
          sessionId: "s-1",
          role: "assistant",
          content: "Thinking...",
          timestamp: new Date().toISOString(),
          status: "streaming",
        }}
      />,
    );

    // The content should be rendered (Streamdown renders the text)
    expect(container.textContent).toContain("Thinking...");
  });

  it("does not show error indicator for complete messages", () => {
    render(
      <MessageBubble
        message={{
          id: "msg-5",
          sessionId: "s-1",
          role: "assistant",
          content: "Done!",
          timestamp: new Date().toISOString(),
          status: "complete",
        }}
      />,
    );

    // No error message should be shown for complete status
    expect(screen.queryByText("Failed to send message.")).toBeNull();
  });
});
