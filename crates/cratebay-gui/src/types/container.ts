/**
 * Container/sandbox type definitions for CrateBay.
 *
 * Matches frontend-spec.md §4.3 — containerStore types.
 */

/**
 * Container information returned from the backend.
 */
export interface ContainerInfo {
  id: string;
  shortId: string;
  name: string;
  templateId: string;
  image: string;
  status: "running" | "stopped" | "creating" | "error";
  createdAt: string;
  cpuCores: number;
  memoryMb: number;
  ports: PortMapping[];
}

/**
 * Request payload for creating a new container.
 */
export interface ContainerCreateRequest {
  templateId: string;
  name?: string;
  image?: string;
  command?: string;
  env?: Record<string, string>;
  cpuCores?: number;
  memoryMb?: number;
  ttlHours?: number;
}

/**
 * Container template definition.
 */
export interface ContainerTemplate {
  id: string;
  name: string;
  description: string;
  image: string;
  defaultCommand: string;
  defaultCpuCores: number;
  defaultMemoryMb: number;
  tags: string[];
}

/**
 * Filter criteria for container list.
 */
export interface ContainerFilter {
  status: "all" | "running" | "stopped";
  search: string;
  templateId: string | null;
}

/**
 * Port mapping between host and container.
 */
export interface PortMapping {
  hostPort: number;
  containerPort: number;
  protocol: "tcp" | "udp";
}

/**
 * Container status change event from the backend.
 */
export interface ContainerStatusEvent {
  containerId: string;
  status: "running" | "stopped" | "error";
  message?: string;
}

/**
 * Container log line event from the backend.
 */
export interface ContainerLogEvent {
  containerId: string;
  line: string;
  stream: "stdout" | "stderr";
  timestamp: string;
}
