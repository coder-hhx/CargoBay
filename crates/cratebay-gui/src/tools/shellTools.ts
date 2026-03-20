/**
 * Shell execution tools for the CrateBay Agent.
 *
 * Wraps Tauri shell commands as AgentTools for pi-agent-core.
 * Shell commands execute inside containers — no direct host access.
 */

import { Type } from "@sinclair/typebox";
import type { AgentTool, AgentToolResult, AgentToolUpdateCallback } from "@mariozechner/pi-agent-core";
import { invoke } from "@/lib/tauri";

// --- Parameter Schemas ---

const ShellExecParams = Type.Object({
  containerId: Type.String({ description: "Container ID or name" }),
  command: Type.String({
    description: "Shell command to execute (runs via /bin/sh -c)",
  }),
  timeout: Type.Optional(
    Type.Number({
      description: "Timeout in seconds (default: 30, max: 300)",
      minimum: 1,
      maximum: 300,
    }),
  ),
});

// --- Helper ---

function textResult(text: string): AgentToolResult<undefined> {
  return {
    content: [{ type: "text", text }],
    details: undefined,
  };
}

// --- Tool Definitions ---

export const shellExecTool: AgentTool<typeof ShellExecParams> = {
  name: "shell_exec",
  label: "Shell Execute",
  description:
    "Execute a shell command inside a container using /bin/sh -c. " +
    "Returns stdout, stderr, and exit code. " +
    "Use this for running build commands, installing packages, or system operations. " +
    "Optionally specify a timeout (default: 30s, max: 300s).",
  parameters: ShellExecParams,
  execute: async (
    _toolCallId,
    params,
    _signal?: AbortSignal,
    onUpdate?: AgentToolUpdateCallback<{ status: string; message: string }>,
  ) => {
    onUpdate?.({
      content: [{ type: "text", text: `Executing: ${params.command}` }],
      details: { status: "running", message: `Executing: ${params.command}` },
    });

    const result = await invoke<{
      exit_code: number;
      stdout: string;
      stderr: string;
    }>("container_exec", {
      id: params.containerId,
      cmd: ["/bin/sh", "-c", params.command],
      timeout: params.timeout,
    });

    onUpdate?.({
      content: [{ type: "text", text: `Command finished with exit code ${result.exit_code}` }],
      details: { status: "completed", message: `Command finished with exit code ${result.exit_code}` },
    });

    const parts: string[] = [];
    if (result.stdout) {
      parts.push(`**stdout:**\n\`\`\`\n${result.stdout}\n\`\`\``);
    }
    if (result.stderr) {
      parts.push(`**stderr:**\n\`\`\`\n${result.stderr}\n\`\`\``);
    }
    parts.push(`**exit code:** ${result.exit_code}`);

    return textResult(parts.join("\n\n"));
  },
};

/**
 * All shell tools exported as an array for the tool registry.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const shellTools: AgentTool<any>[] = [
  shellExecTool,
];
