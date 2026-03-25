/**
 * Settings type definitions for CrateBay.
 *
 * Matches frontend-spec.md §4.5 — settingsStore types.
 */

/**
 * Supported API format types (matches Rust ApiFormat enum).
 */
export type ApiFormat = "anthropic" | "openai_responses" | "openai_completions";

/**
 * LLM provider information.
 */
export interface LlmProviderInfo {
  id: string;
  name: string;
  apiBase: string; // Base URL (e.g., "https://api.openai.com")
  apiFormat: ApiFormat; // Determines request structure and available options
  hasApiKey: boolean; // true if key exists in backend (key value never exposed)
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
}

/**
 * Request payload for creating a new LLM provider.
 */
export interface LlmProviderCreateRequest {
  name: string;
  apiBase: string;
  apiKey: string; // Plaintext, encrypted on backend
  apiFormat: ApiFormat;
}

/**
 * Request payload for updating an existing LLM provider.
 */
export interface LlmProviderUpdateRequest {
  name?: string;
  apiBase?: string;
  apiKey?: string; // If provided, re-encrypts the key
  apiFormat?: ApiFormat;
  enabled?: boolean;
}

/**
 * LLM model information.
 */
export interface LlmModelInfo {
  id: string; // Model ID from API (e.g., "gpt-4o")
  providerId: string;
  name: string;
  isEnabled: boolean; // User toggle state
  supportsReasoning: boolean; // Whether model supports reasoning effort
}

/**
 * Result from testing provider connectivity.
 */
export interface ProviderTestResult {
  success: boolean;
  latencyMs: number;
  model: string;
  error: string | null;
}

/**
 * Application settings.
 */
export interface AppSettings {
  language: "en" | "zh-CN";
  theme: "dark" | "light" | "system";
  sendOnEnter: boolean;
  showAgentThinking: boolean;
  maxConversationHistory: number;
  containerDefaultTtlHours: number;
  confirmDestructiveOps: boolean;
  reasoningEffort: "low" | "medium" | "high"; // Global reasoning effort preference
  registryMirrors: string[]; // Docker registry mirror URLs
  runtimeHttpProxy: string;
  runtimeHttpProxyBridge: boolean;
  runtimeHttpProxyBindHost: string;
  runtimeHttpProxyBindPort: number;
  runtimeHttpProxyGuestHost: string;
  /** When true, fall back to Colima / Docker Desktop if built-in runtime is unavailable. */
  allowExternalDocker: boolean;
}

/**
 * Built-in registry mirrors for China mainland users.
 * Users can add/remove custom mirrors in settings.
 */
export const DEFAULT_REGISTRY_MIRRORS: string[] = [
  "docker.1ms.run",
  "docker.xuanyuan.me",
];
