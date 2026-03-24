/**
 * System information tools for the CrateBay Agent.
 *
 * Wraps Tauri system commands as AgentTools for pi-agent-core.
 */

import { Type } from "@sinclair/typebox";
import type { AgentTool, AgentToolResult } from "@mariozechner/pi-agent-core";
import { invoke } from "@/lib/tauri";

// --- Parameter Schemas ---

const EmptyParams = Type.Object({});

// --- Helper ---

function textResult(text: string): AgentToolResult<undefined> {
  return {
    content: [{ type: "text", text }],
    details: undefined,
  };
}

// --- Tool Definitions ---

export const dockerStatusTool: AgentTool<typeof EmptyParams> = {
  name: "docker_status",
  label: "Docker Status",
  description:
    "Check the current Docker connection status. " +
    "Returns whether Docker is connected, version info, and source (external, built-in, or podman).",
  parameters: EmptyParams,
  execute: async () => {
    const status = await invoke<{
      connected: boolean;
      version: string | null;
      api_version: string | null;
      os: string | null;
      arch: string | null;
      source: string;
      socket_path: string | null;
    }>("docker_status");

    if (!status.connected) {
      return textResult(
        "Docker is **not connected**. Source: " + status.source +
        (status.socket_path ? ` (socket: ${status.socket_path})` : ""),
      );
    }

    return textResult(
      `Docker is **connected**.\n` +
      `- Version: ${status.version ?? "unknown"}\n` +
      `- API: ${status.api_version ?? "unknown"}\n` +
      `- OS/Arch: ${status.os ?? "?"}/${status.arch ?? "?"}\n` +
      `- Source: ${status.source}`,
    );
  },
};

export const systemInfoTool: AgentTool<typeof EmptyParams> = {
  name: "system_info",
  label: "System Info",
  description:
    "Get system-level information about the host machine and CrateBay installation. " +
    "Returns OS, architecture, app version, and data paths.",
  parameters: EmptyParams,
  execute: async () => {
    const info = await invoke<{
      os: string;
      os_version: string;
      arch: string;
      app_version: string;
      data_dir: string;
      db_path: string;
      db_size_bytes: number;
      log_path: string;
    }>("system_info");

    const dbSizeMb = (info.db_size_bytes / 1024 / 1024).toFixed(2);

    return textResult(
      `**System Information**\n` +
      `- OS: ${info.os} ${info.os_version}\n` +
      `- Architecture: ${info.arch}\n` +
      `- CrateBay Version: ${info.app_version}\n` +
      `- Data Directory: ${info.data_dir}\n` +
      `- Database: ${info.db_path} (${dbSizeMb} MB)\n` +
      `- Log File: ${info.log_path}`,
    );
  },
};

export const runtimeStatusTool: AgentTool<typeof EmptyParams> = {
  name: "runtime_status",
  label: "Runtime Status",
  description:
    "Get the built-in container runtime status. " +
    "Returns VM state, resource allocation, and Docker responsiveness.",
  parameters: EmptyParams,
  execute: async () => {
    const status = await invoke<{
      state: string;
      platform: string;
      cpu_cores: number;
      memory_mb: number;
      disk_gb: number;
      docker_responsive: boolean;
      uptime_seconds: number | null;
      resource_usage: {
        cpu_percent: number;
        memory_used_mb: number;
        memory_total_mb: number;
        disk_used_gb: number;
        disk_total_gb: number;
        container_count: number;
      } | null;
    }>("runtime_status");

    const lines = [
      `**Runtime Status**`,
      `- State: ${status.state}`,
      `- Platform: ${status.platform}`,
      `- Resources: ${status.cpu_cores} CPU, ${status.memory_mb}MB RAM, ${status.disk_gb}GB Disk`,
      `- Docker Responsive: ${status.docker_responsive ? "Yes" : "No"}`,
    ];

    if (status.uptime_seconds !== null && status.uptime_seconds !== undefined) {
      const hours = Math.floor(status.uptime_seconds / 3600);
      const minutes = Math.floor((status.uptime_seconds % 3600) / 60);
      lines.push(`- Uptime: ${hours}h ${minutes}m`);
    }

    if (status.resource_usage) {
      const usage = status.resource_usage;
      lines.push(
        `- CPU Usage: ${usage.cpu_percent.toFixed(1)}%`,
        `- Memory: ${usage.memory_used_mb}/${usage.memory_total_mb} MB`,
        `- Disk: ${usage.disk_used_gb.toFixed(1)}/${usage.disk_total_gb.toFixed(1)} GB`,
        `- Containers: ${usage.container_count}`,
      );
    }

    return textResult(lines.join("\n"));
  },
};

/**
 * All system tools exported as an array for the tool registry.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const systemTools: AgentTool<any>[] = [
  dockerStatusTool,
  systemInfoTool,
  runtimeStatusTool,
];
