import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";

// Mock Tauri before importing components that use stores
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import { McpServerList } from "@/components/mcp/McpServerList";
import { useMcpStore } from "@/stores/mcpStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type { McpServerInfo } from "@/types/mcp";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const makeServer = (overrides: Partial<McpServerInfo> = {}): McpServerInfo => ({
  id: "mcp-1",
  name: "Test MCP Server",
  command: "/usr/local/bin/mcp-server",
  args: ["--stdio"],
  env: {},
  enabled: true,
  status: "connected",
  transport: "stdio",
  toolCount: 5,
  ...overrides,
});

function resetStore() {
  useMcpStore.setState({
    servers: [],
    loading: false,
    availableTools: [],
    serverLogs: {},
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
describe("McpServerList", () => {
  const defaultProps = {
    selectedServerId: null,
    onSelectServer: vi.fn(),
    onEditServer: vi.fn(),
  };

  beforeEach(() => {
    resetStore();
    resetTauriMocks();
    vi.clearAllMocks();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("shows loading state", () => {
    useMcpStore.setState({ loading: true });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("Loading MCP servers...")).toBeInTheDocument();
  });

  it("shows empty state when no servers configured", () => {
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("No MCP servers configured")).toBeInTheDocument();
    expect(
      screen.getByText("Add a server to connect external tools."),
    ).toBeInTheDocument();
  });

  it("renders server name", () => {
    useMcpStore.setState({
      servers: [makeServer({ name: "My Awesome MCP" })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("My Awesome MCP")).toBeInTheDocument();
  });

  it("renders Connected badge for connected servers", () => {
    useMcpStore.setState({
      servers: [makeServer({ status: "connected" })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("Connected")).toBeInTheDocument();
  });

  it("renders Disconnected badge for disconnected servers", () => {
    useMcpStore.setState({
      servers: [makeServer({ status: "disconnected" })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("Disconnected")).toBeInTheDocument();
  });

  it("renders Error badge for error servers", () => {
    useMcpStore.setState({
      servers: [makeServer({ status: "error" })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("Error")).toBeInTheDocument();
  });

  it("renders Starting badge for starting servers", () => {
    useMcpStore.setState({
      servers: [makeServer({ status: "starting" })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("Starting")).toBeInTheDocument();
  });

  it("displays tool count badge when server has tools", () => {
    useMcpStore.setState({
      servers: [makeServer({ toolCount: 7 })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("7 tools")).toBeInTheDocument();
  });

  it("does not display tool count badge when toolCount is 0", () => {
    useMcpStore.setState({
      servers: [makeServer({ toolCount: 0 })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.queryByText(/tools/)).not.toBeInTheDocument();
  });

  it("displays command and args", () => {
    useMcpStore.setState({
      servers: [makeServer({ command: "/usr/bin/mcp", args: ["--port", "3000"] })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("/usr/bin/mcp --port 3000")).toBeInTheDocument();
  });

  it("renders multiple servers", () => {
    useMcpStore.setState({
      servers: [
        makeServer({ id: "mcp-1", name: "Server One" }),
        makeServer({ id: "mcp-2", name: "Server Two", status: "disconnected" }),
      ],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByText("Server One")).toBeInTheDocument();
    expect(screen.getByText("Server Two")).toBeInTheDocument();
  });

  it("shows stop button for connected servers", () => {
    useMcpStore.setState({
      servers: [makeServer({ status: "connected" })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByLabelText("Stop server")).toBeInTheDocument();
    expect(screen.queryByLabelText("Start server")).not.toBeInTheDocument();
  });

  it("shows start button for disconnected servers", () => {
    useMcpStore.setState({
      servers: [makeServer({ status: "disconnected" })],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByLabelText("Start server")).toBeInTheDocument();
    expect(screen.queryByLabelText("Stop server")).not.toBeInTheDocument();
  });

  it("shows edit and remove buttons", () => {
    useMcpStore.setState({
      servers: [makeServer()],
    });
    render(<McpServerList {...defaultProps} />);

    expect(screen.getByLabelText("Edit server")).toBeInTheDocument();
    expect(screen.getByLabelText("Remove server")).toBeInTheDocument();
  });

  it("calls onSelectServer when clicking a server row", () => {
    const onSelectServer = vi.fn();
    useMcpStore.setState({
      servers: [makeServer({ id: "mcp-42" })],
    });
    render(
      <McpServerList
        selectedServerId={null}
        onSelectServer={onSelectServer}
        onEditServer={vi.fn()}
      />,
    );

    // Click on the server name to select
    fireEvent.click(screen.getByText("Test MCP Server"));

    expect(onSelectServer).toHaveBeenCalledWith("mcp-42");
  });

  it("calls onSelectServer(null) when clicking already selected server", () => {
    const onSelectServer = vi.fn();
    useMcpStore.setState({
      servers: [makeServer({ id: "mcp-42" })],
    });
    render(
      <McpServerList
        selectedServerId="mcp-42"
        onSelectServer={onSelectServer}
        onEditServer={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByText("Test MCP Server"));

    expect(onSelectServer).toHaveBeenCalledWith(null);
  });

  it("calls onEditServer when clicking edit button", () => {
    const onEditServer = vi.fn();
    const server = makeServer({ id: "mcp-1" });
    useMcpStore.setState({ servers: [server] });

    render(
      <McpServerList
        selectedServerId={null}
        onSelectServer={vi.fn()}
        onEditServer={onEditServer}
      />,
    );

    fireEvent.click(screen.getByLabelText("Edit server"));

    expect(onEditServer).toHaveBeenCalledWith(server);
  });

  it("disables start button when server status is starting", () => {
    useMcpStore.setState({
      servers: [makeServer({ status: "starting" })],
    });
    render(<McpServerList {...defaultProps} />);

    const startBtn = screen.getByLabelText("Start server");
    expect(startBtn).toBeDisabled();
  });
});
