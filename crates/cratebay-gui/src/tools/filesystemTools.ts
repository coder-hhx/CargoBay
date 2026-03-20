/**
 * Filesystem tools for the CrateBay Agent.
 *
 * Wraps Tauri filesystem commands as AgentTools for pi-agent-core.
 * All file operations are scoped to containers — no direct host filesystem access.
 */

import { Type } from "@sinclair/typebox";
import type { AgentTool, AgentToolResult } from "@mariozechner/pi-agent-core";
import { invoke } from "@/lib/tauri";

// --- Parameter Schemas ---

const FileReadParams = Type.Object({
  containerId: Type.String({ description: "Container ID or name" }),
  path: Type.String({ description: "Absolute file path inside the container" }),
});

const FileWriteParams = Type.Object({
  containerId: Type.String({ description: "Container ID or name" }),
  path: Type.String({ description: "Absolute file path inside the container" }),
  content: Type.String({ description: "File content to write" }),
});

const FileListParams = Type.Object({
  containerId: Type.String({ description: "Container ID or name" }),
  path: Type.Optional(
    Type.String({
      description: "Directory path inside the container (defaults to '/')",
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

export const fileReadTool: AgentTool<typeof FileReadParams> = {
  name: "file_read",
  label: "Read File",
  description:
    "Read the contents of a file from a container. " +
    "Returns the full file content as text. " +
    "Use file_list first to confirm the path exists.",
  parameters: FileReadParams,
  execute: async (_toolCallId, params) => {
    const result = await invoke<{
      content: string;
      size_bytes: number;
      path: string;
    }>("container_file_read", {
      id: params.containerId,
      path: params.path,
    });

    if (!result.content && result.content !== "") {
      throw new Error(`Failed to read file: ${params.path}`);
    }

    return textResult(
      `**File: ${result.path}** (${result.size_bytes} bytes)\n\`\`\`\n${result.content}\n\`\`\``,
    );
  },
};

export const fileWriteTool: AgentTool<typeof FileWriteParams> = {
  name: "file_write",
  label: "Write File",
  description:
    "Write content to a file inside a container. " +
    "Creates the file if it doesn't exist, overwrites if it does. " +
    "Parent directories must exist.",
  parameters: FileWriteParams,
  execute: async (_toolCallId, params) => {
    await invoke("container_file_write", {
      id: params.containerId,
      path: params.path,
      content: params.content,
    });

    return textResult(
      `File **${params.path}** written successfully (${params.content.length} characters).`,
    );
  },
};

export const fileListTool: AgentTool<typeof FileListParams> = {
  name: "file_list",
  label: "List Files",
  description:
    "List files and directories at a path inside a container. " +
    "Returns names, types (file/directory), and sizes. " +
    "Defaults to the root directory '/' if no path is specified.",
  parameters: FileListParams,
  execute: async (_toolCallId, params) => {
    const entries = await invoke<Array<{
      name: string;
      entry_type: "file" | "directory" | "symlink" | "other";
      size_bytes: number | null;
      modified_at: string | null;
    }>>("container_file_list", {
      id: params.containerId,
      path: params.path ?? "/",
    });

    if (!Array.isArray(entries) || entries.length === 0) {
      return textResult(`Directory **${params.path ?? "/"}** is empty.`);
    }

    const lines = entries.map((e) => {
      const icon = e.entry_type === "directory" ? "[dir]" : "[file]";
      const size = e.size_bytes !== null ? ` (${e.size_bytes} bytes)` : "";
      return `  ${icon} ${e.name}${size}`;
    });

    return textResult(
      `**${params.path ?? "/"}** — ${entries.length} entries:\n${lines.join("\n")}`,
    );
  },
};

/**
 * All filesystem tools exported as an array for the tool registry.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const filesystemTools: AgentTool<any>[] = [
  fileReadTool,
  fileWriteTool,
  fileListTool,
];
