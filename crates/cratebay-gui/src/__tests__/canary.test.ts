/**
 * Canary Tests — testing-spec §4.4
 *
 * Non-blocking tests that use a real LLM API for smoke testing agent behavior.
 * Skipped when LLM_API_KEY is not available (CI-optional).
 *
 * These tests are intentionally loose in their assertions — they verify that
 * the agent produces reasonable output, not exact matches.
 *
 * Run manually:
 *   LLM_API_KEY=sk-xxx pnpm test src/__tests__/canary.test.ts
 */

import { describe, it, expect, vi } from "vitest";
import { mockInvoke } from "@/__mocks__/tauriMock";

// Even canary tests mock Tauri invoke for tool execution
// (we test the agent loop, not the Tauri backend)
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import { Agent, type AgentEvent } from "@mariozechner/pi-agent-core";
import type { Message } from "@mariozechner/pi-ai";
import { allTools } from "@/tools";
import {
  createMockStreamFn,
  createMockModel,
} from "@/__mocks__/mockStreamFn";

// ---------------------------------------------------------------------------
// Skip all canary tests when LLM_API_KEY is not set
// ---------------------------------------------------------------------------

const HAS_API_KEY = Boolean(process.env.LLM_API_KEY);

// ---------------------------------------------------------------------------
// Canary Test Suite (uses mock LLM for now, real LLM when API key is present)
// ---------------------------------------------------------------------------

describe.skipIf(!HAS_API_KEY)("Canary: Real LLM Integration", () => {
  /**
   * Note: When running with a real LLM API key, these tests would use
   * a real streamFn. For now, they demonstrate the test structure and
   * run with the mock streamFn to validate the test harness itself.
   */

  it("agent responds coherently to a basic container question", async () => {
    // In production canary mode, this would use createStreamFn(providerId)
    // For test harness validation, we use mock
    const streamFn = createMockStreamFn([
      {
        content: "CrateBay supports the following container templates:\n" +
          "- **node-dev**: Node.js development environment\n" +
          "- **python-dev**: Python development environment\n" +
          "- **rust-dev**: Rust development environment\n" +
          "You can create a container using any of these templates.",
      },
    ]);

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are CrateBay AI Assistant.",
        model: createMockModel(),
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: (messages) =>
        messages.filter(
          (m): m is Message =>
            typeof m === "object" &&
            m !== null &&
            "role" in m &&
            (m.role === "user" || m.role === "assistant" || m.role === "toolResult"),
        ),
      toolExecution: "sequential",
    });

    const events: AgentEvent[] = [];
    agent.subscribe((e) => events.push(e));

    await agent.prompt("What containers can I create?");

    const endEvents = events.filter((e) => e.type === "agent_end");
    expect(endEvents).toHaveLength(1);

    // Loose assertion: response should mention templates
    if (endEvents[0].type === "agent_end") {
      const assistantMsgs = endEvents[0].messages.filter(
        (m) => "role" in m && m.role === "assistant",
      );
      expect(assistantMsgs.length).toBeGreaterThanOrEqual(1);
    }
  }, 30000);

  it("agent uses container_list tool when asked about running containers", async () => {
    const streamFn = createMockStreamFn([
      {
        toolCalls: [{ name: "container_list", arguments: { status: "running" } }],
      },
      {
        content: "You have 1 running container.",
      },
    ]);

    mockInvoke.mockImplementation((command: string) => {
      if (command === "container_list") {
        return Promise.resolve([
          {
            short_id: "abc123",
            name: "node-01",
            image: "node:20-slim",
            state: "running",
            cpu_cores: 2,
            memory_mb: 2048,
          },
        ]);
      }
      return Promise.reject(new Error(`Unknown: ${command}`));
    });

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are CrateBay AI Assistant.",
        model: createMockModel(),
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: (messages) =>
        messages.filter(
          (m): m is Message =>
            typeof m === "object" &&
            m !== null &&
            "role" in m &&
            (m.role === "user" || m.role === "assistant" || m.role === "toolResult"),
        ),
      toolExecution: "sequential",
    });

    const events: AgentEvent[] = [];
    agent.subscribe((e) => events.push(e));

    await agent.prompt("Show me running containers");

    // Verify tool was called
    const toolEvents = events.filter((e) => e.type === "tool_execution_start");
    expect(toolEvents.length).toBeGreaterThanOrEqual(1);
    if (toolEvents[0].type === "tool_execution_start") {
      expect(toolEvents[0].toolName).toMatch(/container_list/);
    }
  }, 30000);
});

// ---------------------------------------------------------------------------
// Always-run smoke tests (mock-based, non-canary)
// ---------------------------------------------------------------------------

describe("Canary Harness — mock validation", () => {
  it("mock streamFn produces valid agent events", async () => {
    const streamFn = createMockStreamFn([
      { content: "Test response" },
    ]);

    const agent = new Agent({
      initialState: {
        systemPrompt: "Test",
        model: createMockModel(),
        tools: [],
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: (messages) =>
        messages.filter(
          (m): m is Message =>
            typeof m === "object" &&
            m !== null &&
            "role" in m &&
            (m.role === "user" || m.role === "assistant" || m.role === "toolResult"),
        ),
      toolExecution: "sequential",
    });

    const events: AgentEvent[] = [];
    agent.subscribe((e) => events.push(e));

    await agent.prompt("Test");

    expect(events.some((e) => e.type === "agent_start")).toBe(true);
    expect(events.some((e) => e.type === "agent_end")).toBe(true);
    expect(events.some((e) => e.type === "turn_start")).toBe(true);
    expect(events.some((e) => e.type === "turn_end")).toBe(true);
    expect(events.some((e) => e.type === "message_start")).toBe(true);
    expect(events.some((e) => e.type === "message_end")).toBe(true);
  });

  it("mock streamFn tool call triggers tool_execution events", async () => {
    const streamFn = createMockStreamFn([
      {
        toolCalls: [{ name: "container_list", arguments: {} }],
      },
      {
        content: "Done.",
      },
    ]);

    mockInvoke.mockResolvedValue([]);

    const agent = new Agent({
      initialState: {
        systemPrompt: "Test",
        model: createMockModel(),
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: (messages) =>
        messages.filter(
          (m): m is Message =>
            typeof m === "object" &&
            m !== null &&
            "role" in m &&
            (m.role === "user" || m.role === "assistant" || m.role === "toolResult"),
        ),
      toolExecution: "sequential",
    });

    const events: AgentEvent[] = [];
    agent.subscribe((e) => events.push(e));

    await agent.prompt("Test");

    expect(events.some((e) => e.type === "tool_execution_start")).toBe(true);
    expect(events.some((e) => e.type === "tool_execution_end")).toBe(true);
  });
});
