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
  image: string;
  status: "running" | "stopped" | "creating" | "exited" | "paused" | "restarting" | "removing" | "dead" | "created";
  state: string;
  createdAt: string;
  cpuCores?: number;
  memoryMb?: number;
  ports: PortMapping[];
  labels: Record<string, string>;
}

/**
 * Request payload for creating a new container.
 */
export interface ContainerCreateRequest {
  name: string;
  image: string;
  templateId?: string;
  command?: string;
  env?: string[];
  cpuCores?: number;
  memoryMb?: number;
  autoStart?: boolean;
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
  status: "all" | "running" | "stopped" | "creating";
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
 * Docker image information from local Docker daemon.
 */
export interface DockerImageInfo {
  id: string;
  repoTags: string[];
  size: number;
  created: number;
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
