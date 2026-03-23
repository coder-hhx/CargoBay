/**
 * Container management tools for the CrateBay Agent.
 *
 * Wraps Tauri container commands as AgentTools for pi-agent-core.
 */

import { Type } from "@sinclair/typebox";
import type { AgentTool, AgentToolResult } from "@mariozechner/pi-agent-core";
import { invoke } from "@/lib/tauri";

const ContainerListParams = Type.Object({
  status: Type.Optional(
    Type.Union([Type.Literal("all"), Type.Literal("running"), Type.Literal("stopped")], {
      description: "Filter containers by status",
    }),
  ),
});

const ContainerCreateParams = Type.Object({
  templateId: Type.Optional(
    Type.String({
      description: "Template ID (preferred, aligns with runtime templates)",
    }),
  ),
  template_id: Type.Optional(
    Type.String({
      description: "Deprecated alias for templateId",
    }),
  ),
  image: Type.Optional(
    Type.String({
      description: "Docker image reference (required when templateId is not provided)",
    }),
  ),
  name: Type.Optional(
    Type.String({
      description: "Container name (auto-generated when omitted)",
      minLength: 1,
      maxLength: 64,
    }),
  ),
  cpuCores: Type.Optional(
    Type.Number({
      description: "CPU cores",
      minimum: 1,
      maximum: 16,
    }),
  ),
  cpu_cores: Type.Optional(
    Type.Number({
      description: "Deprecated alias for cpuCores",
      minimum: 1,
      maximum: 16,
    }),
  ),
  memoryMb: Type.Optional(
    Type.Number({
      description: "Memory in MB",
      minimum: 256,
      maximum: 65536,
    }),
  ),
  memory_mb: Type.Optional(
    Type.Number({
      description: "Deprecated alias for memoryMb",
      minimum: 256,
      maximum: 65536,
    }),
  ),
  env: Type.Optional(
    Type.Union([
      Type.Array(Type.String(), {
        description: "Environment variables as KEY=VALUE list",
      }),
      Type.Record(Type.String(), Type.String(), {
        description: "Environment variables as key-value object",
      }),
    ]),
  ),
  ttlHours: Type.Optional(
    Type.Number({
      description: "Optional TTL hours metadata label",
      minimum: 1,
      maximum: 720,
    }),
  ),
});

const ContainerInspectParams = Type.Object({
  containerId: Type.Optional(Type.String({ description: "Container ID or name (preferred)" })),
  id: Type.Optional(Type.String({ description: "Deprecated alias for containerId" })),
});

const ContainerStartParams = Type.Object({
  containerId: Type.Optional(Type.String({ description: "Container ID or name (preferred)" })),
  id: Type.Optional(Type.String({ description: "Deprecated alias for containerId" })),
});

const ContainerStopParams = Type.Object({
  containerId: Type.Optional(Type.String({ description: "Container ID or name (preferred)" })),
  id: Type.Optional(Type.String({ description: "Deprecated alias for containerId" })),
  timeout: Type.Optional(
    Type.Number({
      description: "Graceful stop timeout in seconds",
      minimum: 1,
      maximum: 120,
    }),
  ),
});

const ContainerDeleteParams = Type.Object({
  containerId: Type.Optional(Type.String({ description: "Container ID or name (preferred)" })),
  id: Type.Optional(Type.String({ description: "Deprecated alias for containerId" })),
});

const ContainerExecParams = Type.Object({
  containerId: Type.Optional(Type.String({ description: "Container ID or name (preferred)" })),
  id: Type.Optional(Type.String({ description: "Deprecated alias for containerId" })),
  command: Type.Optional(Type.String({
    description: "Shell command to execute inside the container",
    minLength: 1,
  })),
  cmd: Type.Optional(
    Type.Array(Type.String(), {
      description: "Deprecated alias for command as argv array",
      minItems: 1,
    }),
  ),
  workingDir: Type.Optional(
    Type.String({
      description: "Optional working directory inside container",
    }),
  ),
  working_dir: Type.Optional(
    Type.String({
      description: "Deprecated alias for workingDir",
    }),
  ),
});

const ContainerLogsParams = Type.Object({
  containerId: Type.Optional(Type.String({ description: "Container ID or name (preferred)" })),
  id: Type.Optional(Type.String({ description: "Deprecated alias for containerId" })),
  tail: Type.Optional(
    Type.Number({
      description: "Number of trailing log lines",
      minimum: 1,
      maximum: 5000,
    }),
  ),
  since: Type.Optional(
    Type.String({
      description: "RFC3339 timestamp to filter logs after a point in time",
    }),
  ),
});

type ContainerSummary = {
  id: string;
  shortId?: string;
  short_id?: string;
  name: string;
  image: string;
  status?: string;
  state?: string;
  cpuCores?: number | null;
  cpu_cores?: number | null;
  memoryMb?: number | null;
  memory_mb?: number | null;
};

type ContainerInspectResponse = {
  info?: Record<string, unknown>;
  state?: Record<string, unknown>;
  mounts?: unknown;
  network?: unknown;
  networkSettings?: unknown;
  network_settings?: unknown;
};

type ContainerTemplate = {
  id: string;
  image: string;
};

type ExecResult = {
  exitCode?: number;
  exit_code?: number;
  stdout: string;
  stderr: string;
};

type LogEntry = {
  stream: string;
  message: string;
  timestamp?: string | null;
};

function textResult(text: string): AgentToolResult<undefined> {
  return {
    content: [{ type: "text", text }],
    details: undefined,
  };
}

function getShortId(container: { shortId?: string; short_id?: string; id: string }): string {
  return container.shortId ?? container.short_id ?? container.id.slice(0, 12);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function getStringField(record: Record<string, unknown> | undefined, ...keys: string[]): string | undefined {
  if (!record) return undefined;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.length > 0) {
      return value;
    }
  }
  return undefined;
}

function getNumberField(record: Record<string, unknown> | undefined, ...keys: string[]): number | undefined {
  if (!record) return undefined;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "number" && Number.isFinite(value)) {
      return value;
    }
  }
  return undefined;
}

function getShortIdFromRecord(record: Record<string, unknown>): string {
  const shortId = getStringField(record, "shortId", "short_id");
  if (shortId) return shortId;
  const id = getStringField(record, "id");
  return id ? id.slice(0, 12) : "unknown";
}

function getContainerId(params: { containerId?: string; id?: string }): string {
  const containerId = params.containerId ?? params.id;
  if (containerId === undefined || containerId.trim().length === 0) {
    throw new Error("Missing required parameter: containerId");
  }
  return containerId;
}

function getCommandArgs(params: {
  command?: string;
  cmd?: string[];
}): string[] {
  if (Array.isArray(params.cmd) && params.cmd.length > 0) {
    return params.cmd;
  }
  if (typeof params.command === "string" && params.command.trim().length > 0) {
    return ["/bin/sh", "-c", params.command];
  }
  throw new Error("Missing required parameter: command");
}

function toEnvList(env: string[] | Record<string, string> | undefined): string[] | undefined {
  if (env === undefined) return undefined;
  if (Array.isArray(env)) return env;
  return Object.entries(env).map(([key, value]) => `${key}=${value}`);
}

async function resolveImage(templateId: string): Promise<string | null> {
  const templates = await invoke<ContainerTemplate[]>("container_templates");
  const found = templates.find((template) => template.id === templateId);
  return found?.image ?? null;
}

export const containerListTool: AgentTool<typeof ContainerListParams> = {
  name: "container_list",
  label: "List Containers",
  description:
    "List all managed containers and their runtime status, image, and resources.",
  parameters: ContainerListParams,
  execute: async (_toolCallId, params) => {
    const filters = params.status && params.status !== "all"
      ? { status: [params.status] }
      : undefined;

    const containers = await invoke<ContainerSummary[]>("container_list", { filters });

    if (!Array.isArray(containers) || containers.length === 0) {
      return textResult("No containers found.");
    }

    const lines = containers.map((container) => {
      const cpuCores = container.cpuCores ?? container.cpu_cores ?? "?";
      const memoryMb = container.memoryMb ?? container.memory_mb ?? "?";
      const state = container.state ?? container.status ?? "unknown";
      return `- **${container.name}** (${getShortId(container)}) — ${state} | Image: ${container.image} | CPU: ${cpuCores}c | RAM: ${memoryMb}MB`;
    });

    return textResult(`Found ${containers.length} container(s):\n${lines.join("\n")}`);
  },
};

export const containerInspectTool: AgentTool<typeof ContainerInspectParams> = {
  name: "container_inspect",
  label: "Inspect Container",
  description: "Get detailed runtime and configuration information for a container.",
  parameters: ContainerInspectParams,
  execute: async (_toolCallId, params) => {
    const containerId = getContainerId(params);
    const detail = await invoke<ContainerInspectResponse>("container_inspect", {
      id: containerId,
    });

    const infoRecord = isRecord(detail.info)
      ? detail.info
      : (detail as Record<string, unknown>);
    if (!infoRecord) {
      return textResult("Container details were returned in an unsupported format.");
    }
    const stateRecord = isRecord(detail.state) ? detail.state : undefined;

    const containerName = getStringField(infoRecord, "name") ?? "unknown";
    const image = getStringField(infoRecord, "image") ?? "unknown";
    const lines: string[] = [
      `**Container: ${containerName}** (${getShortIdFromRecord(infoRecord)})`,
      `- Image: ${image}`,
      `- Status: ${
        getStringField(stateRecord, "status") ??
        getStringField(infoRecord, "status", "state") ??
        "unknown"
      }`,
    ];

    const createdAt = getStringField(infoRecord, "createdAt", "created_at");
    if (createdAt !== undefined) {
      lines.push(`- Created: ${createdAt}`);
    }

    const startedAt = getStringField(stateRecord, "startedAt", "started_at");
    if (startedAt !== undefined && startedAt !== null) {
      lines.push(`- Started: ${startedAt}`);
    }

    const cpuCores = getNumberField(infoRecord, "cpuCores", "cpu_cores");
    if (cpuCores !== undefined && cpuCores !== null) {
      lines.push(`- CPU: ${cpuCores} cores`);
    }

    const memoryMb = getNumberField(infoRecord, "memoryMb", "memory_mb");
    if (memoryMb !== undefined && memoryMb !== null) {
      lines.push(`- Memory: ${memoryMb} MB`);
    }

    const portsRaw = infoRecord.ports;
    if (Array.isArray(portsRaw) && portsRaw.length > 0) {
      const portLines = portsRaw
        .filter(isRecord)
        .map((port) => {
          const hostPort = getNumberField(port, "hostPort", "host_port") ?? "?";
          const containerPort = getNumberField(port, "containerPort", "container_port") ?? "?";
          const protocol = getStringField(port, "protocol") ?? "tcp";
          return `  - ${hostPort}:${containerPort}/${protocol}`;
        });
      if (portLines.length > 0) {
        lines.push(`- Ports:\n${portLines.join("\n")}`);
      }
    }

    const mounts = Array.isArray(detail.mounts)
      ? detail.mounts
      : Array.isArray(infoRecord.mounts)
        ? infoRecord.mounts
        : undefined;
    if (Array.isArray(mounts) && mounts.length > 0) {
      lines.push(`- Mounts: ${mounts.length}`);
    }

    const exitCode = getNumberField(stateRecord, "exitCode", "exit_code");
    if (exitCode !== undefined && exitCode !== null) {
      lines.push(`- Exit Code: ${exitCode}`);
    }

    const error = getStringField(stateRecord, "error") ?? getStringField(infoRecord, "error");
    if (error) {
      lines.push(`- Error: ${error}`);
    }

    const detailRecord = detail as Record<string, unknown>;
    const networkRecord = isRecord(detailRecord.network)
      ? detailRecord.network
      : isRecord(detailRecord.networkSettings)
        ? detailRecord.networkSettings
        : isRecord(detailRecord.network_settings)
          ? detailRecord.network_settings
          : undefined;
    if (networkRecord) {
      let networkName = getStringField(networkRecord, "networkName", "network_name", "name");
      let ipAddress = getStringField(networkRecord, "ipAddress", "ip_address", "IPAddress");
      let gateway = getStringField(networkRecord, "gateway", "Gateway");

      const networkGroup = networkRecord.Networks ?? networkRecord.networks;
      if (isRecord(networkGroup)) {
        const firstNetwork = Object.values(networkGroup).find(isRecord);
        if (firstNetwork) {
          networkName = networkName ?? getStringField(firstNetwork, "networkName", "network_name", "name", "NetworkID");
          ipAddress = ipAddress ?? getStringField(firstNetwork, "ipAddress", "ip_address", "IPAddress");
          gateway = gateway ?? getStringField(firstNetwork, "gateway", "Gateway");
        }
      }

      if (networkName || ipAddress || gateway) {
        lines.push(
          `- Network: ${networkName ?? "default"} (IP: ${ipAddress ?? "N/A"}, Gateway: ${gateway ?? "N/A"})`,
        );
      }
    }

    return textResult(lines.join("\n"));
  },
};

export const containerCreateTool: AgentTool<typeof ContainerCreateParams> = {
  name: "container_create",
  label: "Create Container",
  description:
    "Create a container from a template or image with optional resource and env settings.",
  parameters: ContainerCreateParams,
  execute: async (_toolCallId, params) => {
    const templateId = params.templateId ?? params.template_id;
    let resolvedImage = params.image?.trim();

    if ((resolvedImage === undefined || resolvedImage.length === 0) && templateId) {
      const templateImage = await resolveImage(templateId);
      if (templateImage === null) {
        throw new Error(`Template "${templateId}" not found.`);
      }
      resolvedImage = templateImage;
    }

    if (resolvedImage === undefined || resolvedImage.length === 0) {
      throw new Error("container_create requires either templateId or image.");
    }

    const request: Record<string, unknown> = {
      name: params.name?.trim() || `cratebay-${Date.now().toString(36)}`,
      image: resolvedImage,
      autoStart: true,
    };

    const envList = toEnvList(params.env);
    if (envList !== undefined) request.env = envList;
    if (templateId !== undefined) request.templateId = templateId;

    const cpuCores = params.cpuCores ?? params.cpu_cores;
    if (cpuCores !== undefined) request.cpuCores = cpuCores;

    const memoryMb = params.memoryMb ?? params.memory_mb;
    if (memoryMb !== undefined) request.memoryMb = memoryMb;

    if (params.ttlHours !== undefined) {
      request.labels = {
        "com.cratebay.ttl_hours": String(params.ttlHours),
      };
    }

    const result = await invoke<Record<string, unknown>>("container_create", { request });
    if (!isRecord(result)) {
      throw new Error("Container creation failed: invalid backend response");
    }

    const createdName = getStringField(result, "name");
    if (createdName === undefined) {
      throw new Error("Container creation failed: invalid backend response");
    }
    const createdImage = getStringField(result, "image") ?? resolvedImage;
    const createdShortId = getStringField(result, "shortId", "short_id")
      ?? (getStringField(result, "id")?.slice(0, 12) ?? "unknown");
    const createdState = getStringField(result, "state", "status") ?? "created";

    return textResult(
      `Container **${createdName}** (${createdShortId}) created successfully. ` +
      `Image: ${createdImage}, Status: ${createdState}`,
    );
  },
};

export const containerStartTool: AgentTool<typeof ContainerStartParams> = {
  name: "container_start",
  label: "Start Container",
  description: "Start a stopped container.",
  parameters: ContainerStartParams,
  execute: async (_toolCallId, params) => {
    const containerId = getContainerId(params);
    await invoke("container_start", { id: containerId });
    return textResult(`Container ${containerId} started.`);
  },
};

export const containerStopTool: AgentTool<typeof ContainerStopParams> = {
  name: "container_stop",
  label: "Stop Container",
  description: "Stop a running container.",
  parameters: ContainerStopParams,
  execute: async (_toolCallId, params) => {
    const containerId = getContainerId(params);
    await invoke("container_stop", {
      id: containerId,
      timeout: params.timeout,
    });
    return textResult(`Container ${containerId} stopped.`);
  },
};

export const containerDeleteTool: AgentTool<typeof ContainerDeleteParams> = {
  name: "container_delete",
  label: "Delete Container",
  description: "Delete a container permanently.",
  parameters: ContainerDeleteParams,
  execute: async (_toolCallId, params) => {
    const containerId = getContainerId(params);
    await invoke("container_delete", { id: containerId, force: false });
    return textResult(`Container ${containerId} deleted.`);
  },
};

export const containerExecTool: AgentTool<typeof ContainerExecParams> = {
  name: "container_exec",
  label: "Execute Command",
  description: "Execute a shell command inside a running container and return output.",
  parameters: ContainerExecParams,
  execute: async (_toolCallId, params) => {
    const containerId = getContainerId(params);
    const cmd = getCommandArgs(params);
    const result = await invoke<ExecResult>("container_exec", {
      id: containerId,
      cmd,
      working_dir: params.workingDir ?? params.working_dir,
    });

    const exitCode = result.exitCode ?? result.exit_code ?? 0;
    const parts: string[] = [];
    if (result.stdout) parts.push(`**stdout:**\n\`\`\`\n${result.stdout}\n\`\`\``);
    if (result.stderr) parts.push(`**stderr:**\n\`\`\`\n${result.stderr}\n\`\`\``);
    parts.push(`**exit code:** ${exitCode}`);

    return textResult(parts.join("\n\n"));
  },
};

export const containerLogsTool: AgentTool<typeof ContainerLogsParams> = {
  name: "container_logs",
  label: "Container Logs",
  description: "Get stdout/stderr logs from a container.",
  parameters: ContainerLogsParams,
  execute: async (_toolCallId, params) => {
    const containerId = getContainerId(params);
    const logs = await invoke<LogEntry[]>("container_logs", {
      id: containerId,
      options: {
        tail: params.tail ?? 100,
        since: params.since,
      },
    });

    if (!Array.isArray(logs) || logs.length === 0) {
      return textResult("No logs found.");
    }

    const lines = logs.map((entry) => {
      const timePrefix = entry.timestamp ? `${entry.timestamp} ` : "";
      return `${timePrefix}[${entry.stream}] ${entry.message}`;
    });

    return textResult(`\`\`\`\n${lines.join("\n")}\n\`\`\``);
  },
};

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
