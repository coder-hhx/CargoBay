/**
 * Golden File Tests — testing-spec §4.3
 *
 * Verifies that tool output formatting remains stable across code changes.
 * Golden data is stored in ./golden-files/tool-outputs.json.
 *
 * How to update golden files:
 *   1. Modify the JSON in golden-files/tool-outputs.json
 *   2. Run: pnpm test src/__tests__/goldenFile.test.ts
 *   3. Verify changes are intentional, then commit
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";
import goldenData from "./golden-files/tool-outputs.json";

// Mock Tauri before importing tools
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import { containerListTool, containerCreateTool, containerExecTool, containerLogsTool, containerInspectTool } from "@/tools/containerTools";
import { dockerStatusTool } from "@/tools/systemTools";
import { mcpListToolsTool } from "@/tools/mcpTools";

// ---------------------------------------------------------------------------
// Types for golden file data
// ---------------------------------------------------------------------------

interface GoldenEntry {
  description: string;
  input: Record<string, unknown>;
  mockResponse: unknown;
  expectedOutput: {
    exact?: string;
    containsAll?: string[];
  };
}

type GoldenDataMap = Record<string, GoldenEntry>;

const golden: GoldenDataMap = goldenData as GoldenDataMap;

/**
 * Extract text from a tool result.
 */
function extractText(result: { content: Array<{ type: string; text?: string }> }): string {
  return result.content
    .filter((c) => c.type === "text" && c.text)
    .map((c) => c.text!)
    .join("\n");
}

// ---------------------------------------------------------------------------
// Golden File Test Suite
// ---------------------------------------------------------------------------

describe("Golden File Tests — tool output stability", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  // --- container_list ---

  it("container_list: two containers", async () => {
    const entry = golden["container-list-two"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await containerListTool.execute("golden-1", entry.input);
    const text = extractText(result);

    for (const expected of entry.expectedOutput.containsAll!) {
      expect(text).toContain(expected);
    }
  });

  it("container_list: empty", async () => {
    const entry = golden["container-list-empty"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await containerListTool.execute("golden-2", entry.input);
    const text = extractText(result);

    expect(text).toBe(entry.expectedOutput.exact!);
  });

  // --- container_create ---

  it("container_create: success", async () => {
    const entry = golden["container-create-success"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await containerCreateTool.execute("golden-3", entry.input as {
      name: string;
      image: string;
      cpu_cores?: number;
      memory_mb?: number;
    });
    const text = extractText(result);

    for (const expected of entry.expectedOutput.containsAll!) {
      expect(text).toContain(expected);
    }
  });

  // --- container_exec ---

  it("container_exec: successful command", async () => {
    const entry = golden["container-exec-success"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await containerExecTool.execute("golden-4", entry.input as {
      id: string;
      cmd: string[];
    });
    const text = extractText(result);

    for (const expected of entry.expectedOutput.containsAll!) {
      expect(text).toContain(expected);
    }
  });

  it("container_exec: non-zero exit code with stderr", async () => {
    const entry = golden["container-exec-nonzero"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await containerExecTool.execute("golden-5", entry.input as {
      id: string;
      cmd: string[];
    });
    const text = extractText(result);

    for (const expected of entry.expectedOutput.containsAll!) {
      expect(text).toContain(expected);
    }
  });

  // --- container_logs ---

  it("container_logs: mixed stdout/stderr", async () => {
    const entry = golden["container-logs-output"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await containerLogsTool.execute("golden-6", entry.input as {
      id: string;
    });
    const text = extractText(result);

    for (const expected of entry.expectedOutput.containsAll!) {
      expect(text).toContain(expected);
    }
  });

  // --- container_inspect ---

  it("container_inspect: full details", async () => {
    const entry = golden["container-inspect-full"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await containerInspectTool.execute("golden-7", entry.input as {
      containerId: string;
    });
    const text = extractText(result);

    for (const expected of entry.expectedOutput.containsAll!) {
      expect(text).toContain(expected);
    }
  });

  // --- docker_status ---

  it("docker_status: connected", async () => {
    const entry = golden["docker-status-connected"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await dockerStatusTool.execute("golden-8", entry.input);
    const text = extractText(result);

    for (const expected of entry.expectedOutput.containsAll!) {
      expect(text).toContain(expected);
    }
  });

  it("docker_status: disconnected", async () => {
    const entry = golden["docker-status-disconnected"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await dockerStatusTool.execute("golden-9", entry.input);
    const text = extractText(result);

    for (const expected of entry.expectedOutput.containsAll!) {
      expect(text).toContain(expected);
    }
  });

  // --- mcp_list_tools ---

  it("mcp_list_tools: no servers", async () => {
    const entry = golden["mcp-list-tools-empty"];
    mockInvoke.mockResolvedValueOnce(entry.mockResponse);

    const result = await mcpListToolsTool.execute("golden-10", entry.input);
    const text = extractText(result);

    for (const expected of entry.expectedOutput.containsAll!) {
      expect(text).toContain(expected);
    }
  });
});
