import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useAppStore } from "@/stores/appStore";

// Mock child pages to avoid deep rendering issues (e.g. infinite update loops)
vi.mock("@/pages/ChatPage", () => ({
  ChatPage: () => <div data-testid="page-chat">ChatPage</div>,
}));
vi.mock("@/pages/ContainersPage", () => ({
  ContainersPage: () => <div data-testid="page-containers">ContainersPage</div>,
}));
vi.mock("@/pages/McpPage", () => ({
  McpPage: () => <div data-testid="page-mcp">McpPage</div>,
}));
vi.mock("@/pages/SettingsPage", () => ({
  SettingsPage: () => <div data-testid="page-settings">SettingsPage</div>,
}));

// Mock @tauri-apps/api to avoid native module errors in test environment
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Import App after mocks
import App from "../App";

describe("App", () => {
  beforeEach(() => {
    useAppStore.setState({
      currentPage: "chat",
      sidebarOpen: true,
      sidebarWidth: 260,
      dockerConnected: false,
      runtimeStatus: "stopped",
      theme: "dark",
    });
  });

  it("renders the application within AppLayout", () => {
    render(<App />);
    // App name appears in Sidebar logo section
    const elements = screen.getAllByText(/CrateBay/i);
    expect(elements.length).toBeGreaterThanOrEqual(1);
  });

  it("renders the version number", () => {
    render(<App />);
    const versionElements = screen.getAllByText(/v2\.0\.0/);
    expect(versionElements.length).toBeGreaterThanOrEqual(1);
  });

  it("renders the default Chat page", () => {
    render(<App />);
    expect(screen.getByTestId("page-chat")).toBeInTheDocument();
  });

  it("renders navigation sidebar with all pages", () => {
    render(<App />);
    expect(screen.getByText("Containers")).toBeInTheDocument();
    expect(screen.getByText("MCP")).toBeInTheDocument();
    const settingsElements = screen.getAllByText("Settings");
    expect(settingsElements.length).toBeGreaterThanOrEqual(1);
  });
});
