import { useEffect } from "react";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useChatStore } from "@/stores/chatStore";
import { useMcpStore } from "@/stores/mcpStore";
import { invoke, listen } from "@/lib/tauri";
import { AppLayout } from "@/components/layout/AppLayout";
import { ChatPage } from "@/pages/ChatPage";
import { ContainersPage } from "@/pages/ContainersPage";
import { McpPage } from "@/pages/McpPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { ImagesPage } from "@/pages/ImagesPage";

/** Backend DockerStatus shape (matches api-spec.md). */
interface DockerStatusResponse {
  connected: boolean;
  version?: string | null;
  api_version?: string | null;
  os?: string | null;
  arch?: string | null;
  source: string;
  socket_path?: string | null;
}

/** Backend RuntimeStatusInfo shape (matches api-spec.md). */
interface RuntimeStatusResponse {
  state: string;
  platform: string;
  cpu_cores: number;
  memory_mb: number;
  disk_gb: number;
  docker_responsive: boolean;
  uptime_seconds: number | null;
  resource_usage?: unknown;
}

/** Payload emitted by runtime:health Tauri event (from start_health_monitor). */
interface RuntimeHealthPayload {
  /** Serde-serialized RuntimeState enum: "None" | "Ready" | {"Error":"msg"} etc. */
  runtime_state: string | Record<string, string>;
  docker_responsive: boolean;
  docker_version: string | null;
  uptime_seconds: number | null;
  last_check: string;
}

/**
 * Map backend runtime state to the AppState runtimeStatus union.
 *
 * Two formats are accepted:
 * - String from runtime_status command (format_runtime_state output):
 *   "none", "provisioning", "ready", "stopped", "error: ..."
 * - Serde enum from runtime:health event (RuntimeState):
 *   "None", "Ready", "Starting", {"Error": "msg"}, etc.
 */
function mapRuntimeState(state: string | Record<string, string>): "starting" | "running" | "stopped" | "error" {
  // Handle serde enum object form: {"Error": "message"}
  if (typeof state === "object" && state !== null) {
    if ("Error" in state) return "error";
    return "stopped";
  }
  const s = state.toLowerCase();
  if (s === "ready") return "running";
  if (s === "starting" || s === "provisioning") return "starting";
  if (s.startsWith("error")) return "error";
  return "stopped";
}

/**
 * Query Docker and Runtime status on startup.
 * Updates appStore with initial values.
 */
async function initRuntimeStatus() {
  try {
    const dockerStatus = await invoke<DockerStatusResponse>("docker_status");
    useAppStore.getState().setDockerConnected(dockerStatus.connected);
  } catch {
    // Docker status command unavailable — keep default (disconnected)
  }

  try {
    const rtStatus = await invoke<RuntimeStatusResponse>("runtime_status");
    useAppStore.getState().setRuntimeStatus(mapRuntimeState(rtStatus.state));
    // Also update docker connection from runtime's docker_responsive check
    useAppStore.getState().setDockerConnected(rtStatus.docker_responsive);
  } catch {
    // Runtime status command unavailable — keep default (stopped)
  }
}

function App() {
  const currentPage = useAppStore((s) => s.currentPage);
  const theme = useSettingsStore((s) => s.settings.theme);

  // Initialize app state on mount
  useEffect(() => {
    // Load persisted settings (language, theme, etc.)
    void useSettingsStore.getState().fetchSettings();
    // Load persisted sessions
    void useChatStore.getState().loadSessions();
    // Load MCP servers
    void useMcpStore.getState().fetchServers();

    // Query initial Docker & Runtime status
    void initRuntimeStatus();
  }, []);

  // Listen for runtime:health events from backend (emitted every 30s)
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    void listen<RuntimeHealthPayload>(
      "runtime:health",
      (payload) => {
        useAppStore.getState().setRuntimeStatus(mapRuntimeState(payload.runtime_state));
        useAppStore.getState().setDockerConnected(payload.docker_responsive);
      },
    ).then((unsub) => {
      unlisten = unsub;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  // Listen for docker:connected event (emitted after runtime auto-start succeeds)
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    void listen<boolean>(
      "docker:connected",
      () => {
        // Runtime just started Docker — refresh status
        void initRuntimeStatus();
      },
    ).then((unsub) => {
      unlisten = unsub;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  // Sync theme with DOM whenever settings.theme changes
  useEffect(() => {
    const isDark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    document.documentElement.classList.toggle("dark", isDark);
    document.documentElement.style.colorScheme = isDark ? "dark" : "light";

    // Listen for OS theme changes when in "system" mode
    if (theme === "system") {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = (e: MediaQueryListEvent) => {
        document.documentElement.classList.toggle("dark", e.matches);
        document.documentElement.style.colorScheme = e.matches ? "dark" : "light";
      };
      mq.addEventListener("change", handler);
      return () => mq.removeEventListener("change", handler);
    }
  }, [theme]);

  return (
    <AppLayout>
      {currentPage === "chat" && <ChatPage />}
      {currentPage === "containers" && <ContainersPage />}
      {currentPage === "images" && <ImagesPage />}
      {currentPage === "mcp" && <McpPage />}
      {currentPage === "settings" && <SettingsPage />}
    </AppLayout>
  );
}

export default App;
