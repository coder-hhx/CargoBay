import type { Translations } from "@/types/i18n";

const en: Translations = {
  common: {
    confirm: "Confirm",
    cancel: "Cancel",
    save: "Save",
    delete: "Delete",
    loading: "Loading...",
    error: "An error occurred",
  },
  chat: {
    newSession: "New Chat",
    placeholder: "Type a message... (@ to mention, Shift+Enter for new line)",
    sendButton: "Send",
    thinking: "Thinking...",
    toolExecuting: "Executing tool...",
  },
  containers: {
    title: "Containers",
    create: "Create Container",
    start: "Start",
    stop: "Stop",
    delete: "Delete",
    noContainers: "No containers found. Create one to get started.",
  },
  mcp: {
    title: "MCP Servers",
    addServer: "Add Server",
    connected: "Connected",
    disconnected: "Disconnected",
  },
  settings: {
    title: "Settings",
    general: "General",
    providers: "LLM Providers",
    advanced: "Advanced",
    language: "Language",
    theme: "Theme",
  },
};

export default en;
