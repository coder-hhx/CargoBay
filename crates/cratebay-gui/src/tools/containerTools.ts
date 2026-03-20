/**
 * Container management tools for the CrateBay Agent.
 *
 * Wraps Tauri container commands as AgentTools for pi-agent-core.
 */

import { Type } from "@sinclair/typebox";
import type { AgentTool, AgentToolResult } from "@mariozechner/pi-agent-core";
import { invoke } from "@/lib/tauri";

// --- Parameter Schemas ---

const ContainerListParams = Type.Object({
  status: Type.Optional(
    Type.Union([Type.Literal("all"), Type.Literal("running"), Type.Literal("stopped")], {
      description: "Filter by container status. Defaults to 'all'.",
    }),
  ),
});

const ContainerCreateParams = Type.Object({
  name: Type.String({
    description: "Container name (1-64 chars, alphanumeric + hyphens)",
  }),
  image: Type.String({
    description: "Docker image (e.g., 'node:20-alpine', 'python:3.12-slim')",
  }),
  command: Type.Optional(
    Type.String({ description: "Override the default CMD" }),
  ),
  env: Type.Optional(
    Type.Array(Type.String(), {
      description: 'Environment variables as "KEY=VALUE" strings',
    }),
  ),
  ports: Type.Optional(
    Type.Array(
      Type.Object({
        host_port: Type.Number({ description: "Port on host" }),
        container_port: Type.Number({ description: "Port in container" }),
        protocol: Type.Optional(Type.String({ description: '"tcp" or "udp"' })),
      }),
      { description: "Port mappings" },
    ),
  ),
  cpu_cores: Type.Optional(
    Type.Number({ description: "CPU cores (1-16)", minimum: 1, maximum: 16 }),
  ),
  memory_mb: Type.Optional(
    Type.Number({
      description: "Memory in MB (256-65536)",
      minimum: 256,
      maximum: 65536,
    }),
  ),
  template_id: Type.Optional(
    Type.String({ description: "Use a predefined template (node-dev, python-dev, rust-dev)" }),
  ),
});

const ContainerIdParams = Type.Object({
  id: Type.String({ description: "Container ID or name" }),
});

const ContainerStopParams = Type.Object({
  id: Type.String({ description: "Container ID or name" }),
  timeout: Type.Optional(
    Type.Number({ description: "Seconds to wait before force kill (default: 10)" }),
  ),
});

const ContainerExecParams = Type.Object({
  id: Type.String({ description: "Container ID" }),
  cmd: Type.Array(Type.String(), {
    description: 'Command and arguments, e.g., ["ls", "-la"]',
  }),
  working_dir: Type.Optional(
    Type.String({ description: "Working directory inside container" }),
  ),
});

const ContainerInspectParams = Type.Object({
  containerId: Type.String({ description: "Container ID or name to inspect" }),
});

const ContainerLogsParams = Type.Object({
  id: Type.String({ description: "Container ID" }),
  tail: Type.Optional(
    Type.Number({ description: "Number of lines from end (default: 100)" }),
  ),
  since: Type.Optional(
    Type.String({ description: "RFC3339 timestamp to start from" }),
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

export const containerListTool: AgentTool<typeof ContainerListParams> = {
  name: "container_list",
  label: "List Containers",
  description:
    "List all managed containers/sandboxes. Returns container IDs, names, " +
    "status, image, and resource allocation. " +
    "Use this to check what containers are available before performing operations.",
  parameters: ContainerListParams,
  execute: async (_toolCallId, params) => {
    const filters = params.status && params.status !== "all"
      ? { status: [params.status === "running" ? "Running" : "Stopped"] }
      : undefined;

    const containers = await invoke<Array<{
      short_id: string;
      name: string;
      image: string;
      state: string;
      cpu_cores: number | null;
      memory_mb: number | null;
    }>>("container_list", { filters });

    if (!Array.isArray(containers) || containers.length === 0) {
      return textResult("No containers found.");
    }

    const lines = containers.map(
      (c) =>
        `- **${c.name}** (${c.short_id}) — ${c.state} | Image: ${c.image} | CPU: ${c.cpu_cores ?? "?"}c | RAM: ${c.memory_mb ?? "?"}MB`,
    );

    return textResult(`Found ${containers.length} container(s):\n${lines.join("\n")}`);
  },
};

export const containerInspectTool: AgentTool<typeof ContainerInspectParams> = {
  name: "container_inspect",
  label: "Inspect Container",
  description:
    "Get detailed information about a specific container including configuration, " +
    "state, network settings, mounts, resource limits, and port mappings. " +
    "Use this to check container details before performing operations.",
  parameters: ContainerInspectParams,
  execute: async (_toolCallId, params) => {
    const info = await invoke<{
      short_id: string;
      name: string;
      image: string;
      state: string;
      created_at: string;
      started_at: string | null;
      cpu_cores: number | null;
      memory_mb: number | null;
      env: string[];
      ports: Array<{
        host_port: number;
        container_port: number;
        protocol: string;
      }>;
      mounts: Array<{
        source: string;
        destination: string;
        mode: string;
      }>;
      network: {
        ip_address: string | null;
        gateway: string | null;
        network_name: string | null;
      } | null;
    }>("container_inspect", { id: params.containerId });

    const lines = [
      `**Container: ${info.name}** (${info.short_id})`,
      `- Image: ${info.image}`,
      `- State: ${info.state}`,
      `- Created: ${info.created_at}`,
    ];

    if (info.started_at) {
      lines.push(`- Started: ${info.started_at}`);
    }
    if (info.cpu_cores !== null) {
      lines.push(`- CPU: ${info.cpu_cores} cores`);
    }
    if (info.memory_mb !== null) {
      lines.push(`- Memory: ${info.memory_mb} MB`);
    }
    if (info.env.length > 0) {
      lines.push(`- Env: ${info.env.length} variable(s)`);
    }
    if (info.ports.length > 0) {
      const portLines = info.ports.map(
        (p) => `  - ${p.host_port}:${p.container_port}/${p.protocol}`,
      );
      lines.push(`- Ports:\n${portLines.join("\n")}`);
    }
    if (info.mounts.length > 0) {
      const mountLines = info.mounts.map(
        (m) => `  - ${m.source} → ${m.destination} (${m.mode})`,
      );
      lines.push(`- Mounts:\n${mountLines.join("\n")}`);
    }
    if (info.network) {
      lines.push(
        `- Network: ${info.network.network_name ?? "default"} ` +
        `(IP: ${info.network.ip_address ?? "N/A"}, Gateway: ${info.network.gateway ?? "N/A"})`,
      );
    }

    return textResult(lines.join("\n"));
  },
};

export const containerCreateTool: AgentTool<typeof ContainerCreateParams> = {
  name: "container_create",
  label: "Create Container",
  description:
    "Create a new container from a Docker image or template. " +
    "Specify the image, resource limits, ports, and environment variables. " +
    "The container starts automatically after creation.",
  parameters: ContainerCreateParams,
  execute: async (_toolCallId, params) => {
    const request: Record<string, unknown> = {
      name: params.name,
      image: params.image,
    };
    if (params.command !== undefined) request.command = params.command;
    if (params.env !== undefined) request.env = params.env;
    if (params.ports !== undefined) request.ports = params.ports;
    if (params.cpu_cores !== undefined) request.cpu_cores = params.cpu_cores;
    if (params.memory_mb !== undefined) request.memory_mb = params.memory_mb;
    if (params.template_id !== undefined) request.template_id = params.template_id;
    request.auto_start = true;

    const result = await invoke<{
      short_id: string;
      name: string;
      image: string;
      state: string;
    }>("container_create", { request });

    if (!result || !result.short_id) {
      throw new Error("Container creation failed: no result returned from backend");
    }

    return textResult(
      `Container **${result.name}** (${result.short_id}) created successfully. ` +
      `Image: ${result.image}, Status: ${result.state}`,
    );
  },
};

export const containerStartTool: AgentTool<typeof ContainerIdParams> = {
  name: "container_start",
  label: "Start Container",
  description: "Start a stopped container by ID or name.",
  parameters: ContainerIdParams,
  execute: async (_toolCallId, params) => {
    await invoke("container_start", { id: params.id });
    return textResult(`Container ${params.id} started.`);
  },
};

export const containerStopTool: AgentTool<typeof ContainerStopParams> = {
  name: "container_stop",
  label: "Stop Container",
  description:
    "Stop a running container. Optionally specify a timeout in seconds before force killing.",
  parameters: ContainerStopParams,
  execute: async (_toolCallId, params) => {
    await invoke("container_stop", {
      id: params.id,
      timeout: params.timeout,
    });
    return textResult(`Container ${params.id} stopped.`);
  },
};

export const containerDeleteTool: AgentTool<typeof ContainerIdParams> = {
  name: "container_delete",
  label: "Delete Container",
  description:
    "Permanently delete a container. The container must be stopped first. " +
    "All data inside the container will be lost. This action cannot be undone.",
  parameters: ContainerIdParams,
  execute: async (_toolCallId, params) => {
    await invoke("container_delete", { id: params.id, force: false });
    return textResult(`Container ${params.id} deleted.`);
  },
};

export const containerExecTool: AgentTool<typeof ContainerExecParams> = {
  name: "container_exec",
  label: "Execute Command",
  description:
    "Execute a command inside a running container. " +
    "Returns stdout, stderr, and exit code.",
  parameters: ContainerExecParams,
  execute: async (_toolCallId, params) => {
    const result = await invoke<{
      exit_code: number;
      stdout: string;
      stderr: string;
    }>("container_exec", {
      id: params.id,
      cmd: params.cmd,
      working_dir: params.working_dir,
    });

    const parts: string[] = [];
    if (result.stdout) parts.push(`**stdout:**\n\`\`\`\n${result.stdout}\n\`\`\``);
    if (result.stderr) parts.push(`**stderr:**\n\`\`\`\n${result.stderr}\n\`\`\``);
    parts.push(`**exit code:** ${result.exit_code}`);

    return textResult(parts.join("\n\n"));
  },
};

export const containerLogsTool: AgentTool<typeof ContainerLogsParams> = {
  name: "container_logs",
  label: "Container Logs",
  description:
    "Get stdout/stderr logs from a container. " +
    "Useful for debugging and monitoring container output.",
  parameters: ContainerLogsParams,
  execute: async (_toolCallId, params) => {
    const logs = await invoke<Array<{
      stream: string;
      message: string;
      timestamp: string | null;
    }>>("container_logs", {
      id: params.id,
      options: {
        tail: params.tail ?? 100,
        since: params.since,
      },
    });

    if (!Array.isArray(logs) || logs.length === 0) {
      return textResult("No logs found.");
    }

    const lines = logs.map(
      (entry) => `[${entry.stream}] ${entry.message}`,
    );

    return textResult(`\`\`\`\n${lines.join("")}\`\`\``);
  },
};

/**
 * All container tools exported as an array for the tool registry.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const containerTools: AgentTool<any>[] = [
  containerListTool,
  containerInspectTool,
  containerCreateTool,
  containerStartTool,
  containerStopTool,
  containerDeleteTool,
  containerExecTool,
  containerLogsTool,
];
