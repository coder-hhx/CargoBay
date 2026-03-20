import { create } from "zustand";
import { invoke } from "@/lib/tauri";
import type {
  ContainerInfo,
  ContainerCreateRequest,
  ContainerTemplate,
  ContainerFilter,
} from "@/types/container";

// Re-export types for backward compatibility
export type { ContainerInfo, ContainerCreateRequest, ContainerTemplate };

interface ContainerState {
  // Container list
  containers: ContainerInfo[];
  loading: boolean;
  error: string | null;
  fetchContainers: () => Promise<void>;

  // Selection
  selectedContainerId: string | null;
  selectContainer: (id: string | null) => void;

  // Operations
  createContainer: (req: ContainerCreateRequest) => Promise<ContainerInfo>;
  startContainer: (id: string) => Promise<void>;
  stopContainer: (id: string) => Promise<void>;
  deleteContainer: (id: string) => Promise<void>;

  // Templates
  templates: ContainerTemplate[];
  fetchTemplates: () => Promise<void>;

  // Filters
  filter: ContainerFilter;
  setFilter: (filter: Partial<ContainerFilter>) => void;

  // Computed
  filteredContainers: () => ContainerInfo[];
}

let mockIdCounter = 0;

export const useContainerStore = create<ContainerState>()((set, get) => ({
  containers: [],
  loading: false,
  error: null,

  fetchContainers: async () => {
    set({ loading: true, error: null });
    try {
      const containers = await invoke<ContainerInfo[]>("container_list");
      set({ containers, loading: false });
    } catch {
      // Mock data for non-Tauri development
      set({ loading: false });
    }
  },

  selectedContainerId: null,
  selectContainer: (id) => set({ selectedContainerId: id }),

  createContainer: async (req) => {
    try {
      const container = await invoke<ContainerInfo>("container_create", { req });
      set((state) => ({ containers: [...state.containers, container] }));
      return container;
    } catch {
      // Mock for non-Tauri development
      const container: ContainerInfo = {
        id: `mock-${++mockIdCounter}-${Date.now()}`,
        shortId: `mock-${mockIdCounter}`.slice(0, 12),
        name: req.name ?? `container-${mockIdCounter}`,
        templateId: req.templateId,
        image: req.image ?? "ubuntu:latest",
        status: "running",
        createdAt: new Date().toISOString(),
        cpuCores: req.cpuCores ?? 2,
        memoryMb: req.memoryMb ?? 2048,
        ports: [],
      };
      set((state) => ({ containers: [...state.containers, container] }));
      return container;
    }
  },

  startContainer: async (id) => {
    try {
      await invoke("container_start", { id });
    } catch {
      // Mock
    }
    set((state) => ({
      containers: state.containers.map((c) =>
        c.id === id ? { ...c, status: "running" as const } : c,
      ),
    }));
  },

  stopContainer: async (id) => {
    try {
      await invoke("container_stop", { id });
    } catch {
      // Mock
    }
    set((state) => ({
      containers: state.containers.map((c) =>
        c.id === id ? { ...c, status: "stopped" as const } : c,
      ),
    }));
  },

  deleteContainer: async (id) => {
    try {
      await invoke("container_delete", { id });
    } catch {
      // Mock
    }
    set((state) => ({
      containers: state.containers.filter((c) => c.id !== id),
      selectedContainerId: state.selectedContainerId === id ? null : state.selectedContainerId,
    }));
  },

  templates: [],
  fetchTemplates: async () => {
    try {
      const templates = await invoke<ContainerTemplate[]>("container_templates");
      set({ templates });
    } catch {
      // Mock templates
      set({
        templates: [
          {
            id: "node-dev",
            name: "Node.js Dev",
            description: "Node.js development environment",
            image: "node:20-slim",
            defaultCommand: "bash",
            defaultCpuCores: 2,
            defaultMemoryMb: 2048,
            tags: ["node", "javascript"],
          },
          {
            id: "python-dev",
            name: "Python Dev",
            description: "Python development environment",
            image: "python:3.12-slim",
            defaultCommand: "bash",
            defaultCpuCores: 2,
            defaultMemoryMb: 2048,
            tags: ["python"],
          },
          {
            id: "rust-dev",
            name: "Rust Dev",
            description: "Rust development environment",
            image: "rust:1.75-slim",
            defaultCommand: "bash",
            defaultCpuCores: 4,
            defaultMemoryMb: 4096,
            tags: ["rust"],
          },
        ],
      });
    }
  },

  filter: { status: "all", search: "", templateId: null },
  setFilter: (patch) =>
    set((state) => ({ filter: { ...state.filter, ...patch } })),

  filteredContainers: () => {
    const { containers, filter } = get();
    return containers.filter((c) => {
      if (filter.status !== "all" && c.status !== filter.status) return false;
      if (filter.templateId !== null && c.templateId !== filter.templateId) return false;
      if (
        filter.search.length > 0 &&
        !c.name.toLowerCase().includes(filter.search.toLowerCase()) &&
        !c.image.toLowerCase().includes(filter.search.toLowerCase())
      ) {
        return false;
      }
      return true;
    });
  },
}));
