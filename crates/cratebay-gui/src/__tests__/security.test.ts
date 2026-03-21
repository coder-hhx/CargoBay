/**
 * Security Tests — API Key Leakage Prevention & MCP Input Validation.
 *
 * Covers testing-spec.md §7.3 (API Key Leakage Prevention)
 * and §7.4 Security Checklist (API Key + MCP categories).
 *
 * These tests verify that:
 * 1. The settingsStore never exposes raw API key values in state
 * 2. API keys are only sent to the Rust backend via Tauri invoke
 * 3. LlmProviderInfo uses hasApiKey (boolean), not the actual key
 * 4. MCP tool calls pass parameters directly to the backend (no frontend injection surface)
 * 5. Frontend source code contains no hardcoded API key patterns
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import * as fs from "node:fs";
import * as path from "node:path";

// Mock the Tauri invoke/listen wrappers
const mockInvoke = vi.fn<(...args: unknown[]) => Promise<unknown>>(
  () => Promise.reject(new Error("Tauri not available in test")),
);
vi.mock("@/lib/tauri", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

// Import stores AFTER mocking
import { useSettingsStore } from "@/stores/settingsStore";
import { useMcpStore } from "@/stores/mcpStore";

// ---------------------------------------------------------------------------
// §7.3 — API Key Leakage Prevention
// ---------------------------------------------------------------------------
describe("API Key Security", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockRejectedValue(new Error("Tauri not available in test"));

    // Reset settingsStore
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
      },
    });
  });

  it("LlmProviderInfo type uses hasApiKey boolean, not raw key", () => {
    // Seed a provider into the store
    useSettingsStore.setState({
      providers: [
        {
          id: "provider-1",
          name: "TestProvider",
          apiBase: "https://api.example.com",
          apiFormat: "openai_completions",
          hasApiKey: true,
          enabled: true,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ],
    });

    const state = useSettingsStore.getState();

    // Verify provider has hasApiKey (boolean), NOT the actual key value
    for (const provider of state.providers) {
      expect(provider).toHaveProperty("hasApiKey");
      expect(typeof provider.hasApiKey).toBe("boolean");

      // The provider object must NOT have an "apiKey" property
      expect(provider).not.toHaveProperty("apiKey");
    }
  });

  it("settingsStore state snapshot contains no API key strings", () => {
    // Create a provider with a known API key through the mock fallback path
    useSettingsStore.setState({
      providers: [
        {
          id: "provider-1",
          name: "TestProvider",
          apiBase: "https://api.example.com",
          apiFormat: "openai_completions",
          hasApiKey: true,
          enabled: true,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ],
    });

    // Serialize the full store state and assert no key-like patterns
    const stateJson = JSON.stringify(useSettingsStore.getState());
    expect(stateJson).not.toMatch(/sk-[a-zA-Z0-9]{20,}/);
    expect(stateJson).not.toMatch(/key-[a-zA-Z0-9]{20,}/);
    expect(stateJson).not.toContain("sk-test-key");
  });

  it("createProvider stores only hasApiKey flag, not the raw key", async () => {
    const provider = await useSettingsStore.getState().createProvider({
      name: "NewProvider",
      apiBase: "https://api.example.com",
      apiKey: "sk-super-secret-key-1234567890",
      apiFormat: "anthropic",
    });

    // The returned provider should have hasApiKey: true
    expect(provider.hasApiKey).toBe(true);
    // But it should NOT contain the raw key
    expect(provider).not.toHaveProperty("apiKey");
    expect(JSON.stringify(provider)).not.toContain("sk-super-secret-key");

    // The full store state should also not contain the raw key
    const stateJson = JSON.stringify(useSettingsStore.getState());
    expect(stateJson).not.toContain("sk-super-secret-key");
  });

  it("saveApiKey sends key to backend and never stores locally", async () => {
    // Seed a provider first
    useSettingsStore.setState({
      providers: [
        {
          id: "provider-1",
          name: "TestProvider",
          apiBase: "https://api.example.com",
          apiFormat: "openai_completions",
          hasApiKey: false,
          enabled: true,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ],
    });

    // Save an API key
    await useSettingsStore.getState().saveApiKey("provider-1", "sk-secret-value-99999");

    // The invoke should have been called with the key (sent to backend)
    expect(mockInvoke).toHaveBeenCalledWith("api_key_save", {
      providerId: "provider-1",
      apiKey: "sk-secret-value-99999",
    });

    // The key value must NOT be stored in the Zustand state
    const stateJson = JSON.stringify(useSettingsStore.getState());
    expect(stateJson).not.toContain("sk-secret-value-99999");

    // The provider should have hasApiKey updated to true (in mock fallback)
    const provider = useSettingsStore.getState().providers.find((p) => p.id === "provider-1");
    expect(provider?.hasApiKey).toBe(true);
  });

  it("deleteApiKey clears hasApiKey flag without exposing key", async () => {
    useSettingsStore.setState({
      providers: [
        {
          id: "provider-1",
          name: "TestProvider",
          apiBase: "https://api.example.com",
          apiFormat: "openai_completions",
          hasApiKey: true,
          enabled: true,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ],
    });

    await useSettingsStore.getState().deleteApiKey("provider-1");

    expect(mockInvoke).toHaveBeenCalledWith("api_key_delete", {
      providerId: "provider-1",
    });

    const provider = useSettingsStore.getState().providers.find((p) => p.id === "provider-1");
    expect(provider?.hasApiKey).toBe(false);
  });

  it("hasApiKey helper returns correct boolean from provider state", () => {
    useSettingsStore.setState({
      providers: [
        {
          id: "p-with-key",
          name: "WithKey",
          apiBase: "https://api.example.com",
          apiFormat: "openai_completions",
          hasApiKey: true,
          enabled: true,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
        {
          id: "p-without-key",
          name: "WithoutKey",
          apiBase: "https://api.example.com",
          apiFormat: "anthropic",
          hasApiKey: false,
          enabled: true,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ],
    });

    expect(useSettingsStore.getState().hasApiKey("p-with-key")).toBe(true);
    expect(useSettingsStore.getState().hasApiKey("p-without-key")).toBe(false);
    expect(useSettingsStore.getState().hasApiKey("nonexistent")).toBe(false);
  });

  it("updateProvider with apiKey does not store key in state", async () => {
    useSettingsStore.setState({
      providers: [
        {
          id: "provider-1",
          name: "TestProvider",
          apiBase: "https://api.example.com",
          apiFormat: "openai_completions",
          hasApiKey: false,
          enabled: true,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ],
    });

    const updated = await useSettingsStore
      .getState()
      .updateProvider("provider-1", {
        name: "Updated",
        apiKey: "sk-new-secret-key-abcdefg",
      });

    // hasApiKey should be true since we provided a key
    expect(updated.hasApiKey).toBe(true);
    // The raw key should NOT be in the returned object or state
    expect(updated).not.toHaveProperty("apiKey");
    expect(JSON.stringify(useSettingsStore.getState())).not.toContain("sk-new-secret-key");
  });
});

// ---------------------------------------------------------------------------
// MCP Input Validation — JSON-RPC Injection Prevention (Frontend Side)
// ---------------------------------------------------------------------------
describe("MCP Input Validation", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockRejectedValue(new Error("Tauri not available in test"));

    useMcpStore.setState({
      servers: [],
      loading: false,
      availableTools: [],
      serverLogs: {},
    });
  });

  it("callTool passes parameters directly to Tauri invoke (backend handles validation)", async () => {
    // Make invoke succeed for this test
    mockInvoke.mockResolvedValueOnce({ result: "ok" });

    const result = await useMcpStore.getState().callTool(
      "server-1",
      "test_tool",
      { param1: "value1", param2: 42 },
    );

    // Verify invoke was called with correct parameters
    expect(mockInvoke).toHaveBeenCalledWith("mcp_client_call_tool", {
      serverId: "server-1",
      toolName: "test_tool",
      arguments: { param1: "value1", param2: 42 },
    });
    expect(result).toEqual({ result: "ok" });
  });

  it("callTool rejects with descriptive error when backend fails", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("Invalid tool name"));

    await expect(
      useMcpStore.getState().callTool("server-1", "bad_tool", {}),
    ).rejects.toThrow("MCP tool call failed: Error: Invalid tool name");
  });

  it("callTool does not modify or sanitize arguments (backend responsibility)", async () => {
    mockInvoke.mockResolvedValueOnce({ result: "processed" });

    // Pass potentially dangerous arguments — the frontend should NOT modify them.
    // Validation/sanitization is the backend's responsibility.
    const dangerousArgs = {
      "jsonrpc": "2.0",
      "__proto__": { "polluted": true },
      "constructor": "Object",
      "path": "../../../etc/passwd",
      "command": "rm -rf /",
    };

    await useMcpStore.getState().callTool("server-1", "test_tool", dangerousArgs);

    // The arguments should be passed through exactly as-is to the backend
    expect(mockInvoke).toHaveBeenCalledWith("mcp_client_call_tool", {
      serverId: "server-1",
      toolName: "test_tool",
      arguments: dangerousArgs,
    });
  });

  it("callTool handles empty arguments", async () => {
    mockInvoke.mockResolvedValueOnce(null);

    await useMcpStore.getState().callTool("server-1", "no_args_tool", {});

    expect(mockInvoke).toHaveBeenCalledWith("mcp_client_call_tool", {
      serverId: "server-1",
      toolName: "no_args_tool",
      arguments: {},
    });
  });
});

// ---------------------------------------------------------------------------
// Static Analysis — Source Code Scanning for Hardcoded Keys
// ---------------------------------------------------------------------------
describe("Source Code Security Scan", () => {
  const SRC_DIR = path.resolve(__dirname, "..");

  /**
   * Recursively collect all .ts and .tsx files under a directory,
   * excluding node_modules, __tests__, __mocks__, and .test. files.
   */
  function collectSourceFiles(dir: string): string[] {
    const files: string[] = [];
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (
          entry.name === "node_modules" ||
          entry.name === "__tests__" ||
          entry.name === "__mocks__" ||
          entry.name === "e2e" ||
          entry.name === ".git"
        ) {
          continue;
        }
        files.push(...collectSourceFiles(fullPath));
      } else if (
        (entry.name.endsWith(".ts") || entry.name.endsWith(".tsx")) &&
        !entry.name.includes(".test.")
      ) {
        files.push(fullPath);
      }
    }
    return files;
  }

  it("no hardcoded API key patterns in source files", () => {
    const sourceFiles = collectSourceFiles(SRC_DIR);
    expect(sourceFiles.length).toBeGreaterThan(0);

    // Patterns that indicate hardcoded API keys
    const keyPatterns = [
      /sk-[a-zA-Z0-9]{20,}/,           // OpenAI-style key
      /key-[a-zA-Z0-9]{20,}/,           // Generic key pattern
      /AKIA[0-9A-Z]{16}/,               // AWS access key
      /-----BEGIN (RSA |EC |DSA )?PRIVATE KEY-----/, // PEM private key
    ];

    const violations: string[] = [];

    for (const filePath of sourceFiles) {
      const content = fs.readFileSync(filePath, "utf-8");
      const relativePath = path.relative(SRC_DIR, filePath);

      for (const pattern of keyPatterns) {
        if (pattern.test(content)) {
          violations.push(`${relativePath} matches ${pattern.source}`);
        }
      }
    }

    expect(violations).toEqual([]);
  });

  it("no API key values stored in localStorage/sessionStorage calls", () => {
    const sourceFiles = collectSourceFiles(SRC_DIR);

    const storagePatterns = [
      /localStorage\.setItem\s*\([^)]*(?:apiKey|api_key|secret|token)[^)]*\)/i,
      /sessionStorage\.setItem\s*\([^)]*(?:apiKey|api_key|secret|token)[^)]*\)/i,
    ];

    const violations: string[] = [];

    for (const filePath of sourceFiles) {
      const content = fs.readFileSync(filePath, "utf-8");
      const relativePath = path.relative(SRC_DIR, filePath);

      for (const pattern of storagePatterns) {
        if (pattern.test(content)) {
          violations.push(`${relativePath}: stores key-like value in browser storage`);
        }
      }
    }

    expect(violations).toEqual([]);
  });

  it("LlmProviderInfo interface does NOT have apiKey field", () => {
    // Read the types/settings.ts file and verify the interface definition
    const settingsTypePath = path.resolve(SRC_DIR, "types", "settings.ts");
    const content = fs.readFileSync(settingsTypePath, "utf-8");

    // Extract the LlmProviderInfo interface body
    const interfaceMatch = content.match(
      /export interface LlmProviderInfo\s*\{([^}]+)\}/,
    );
    expect(interfaceMatch).not.toBeNull();

    const interfaceBody = interfaceMatch![1];

    // The interface should have hasApiKey but NOT apiKey (without the "has" prefix)
    expect(interfaceBody).toContain("hasApiKey");
    // Check that there's no standalone "apiKey" field (not preceded by "has")
    const fieldLines = interfaceBody.split("\n").map((l) => l.trim());
    const apiKeyFields = fieldLines.filter(
      (line) => /^\s*apiKey\s*[?:]/.test(line),
    );
    expect(apiKeyFields).toHaveLength(0);
  });

  it("settingsStore does not persist API keys in Zustand state", () => {
    const storePath = path.resolve(SRC_DIR, "stores", "settingsStore.ts");
    const content = fs.readFileSync(storePath, "utf-8");

    // The store should use "hasApiKey" pattern, not store raw keys
    // Check that there's no zustand persist middleware storing API keys
    expect(content).not.toMatch(/persist\s*\(/); // No Zustand persist (keys would leak to localStorage)

    // The store's state interface should use hasApiKey, not apiKey
    expect(content).toContain("hasApiKey");

    // saveApiKey should invoke the backend, not store locally
    expect(content).toContain('invoke("api_key_save"');
  });

  it("streamFn does not include API keys in Tauri invoke parameters", () => {
    const streamFnPath = path.resolve(SRC_DIR, "lib", "streamFn.ts");
    const content = fs.readFileSync(streamFnPath, "utf-8");

    // The streamFn should NOT include apiKey in invoke parameters
    // It should only pass: channel_id, provider_id, model_id, messages, options
    expect(content).not.toMatch(/apiKey|api_key/i);

    // Verify the invoke call uses provider_id (backend looks up the key)
    expect(content).toContain("provider_id: providerId");

    // Verify the file explicitly documents that keys don't leave the backend
    expect(content).toContain("API keys never leave the backend");
  });
});
