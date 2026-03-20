import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";

// Mock Tauri before importing stores
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import { useContainerStore } from "@/stores/containerStore";
import type { ContainerInfo, ContainerTemplate } from "@/types/container";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const makeContainer = (overrides: Partial<ContainerInfo> = {}): ContainerInfo => ({
  id: "c-1",
  shortId: "c-1",
  name: "node-01",
  templateId: "node-dev",
  image: "node:20-slim",
  status: "running",
  createdAt: new Date().toISOString(),
  cpuCores: 2,
  memoryMb: 2048,
  ports: [],
  ...overrides,
});

const makeTemplate = (overrides: Partial<ContainerTemplate> = {}): ContainerTemplate => ({
  id: "node-dev",
  name: "Node.js Dev",
  description: "Node.js development environment",
  image: "node:20-slim",
  defaultCommand: "bash",
  defaultCpuCores: 2,
  defaultMemoryMb: 2048,
  tags: ["node", "javascript"],
  ...overrides,
});

function resetStore() {
  useContainerStore.setState({
    containers: [],
    loading: false,
    error: null,
    selectedContainerId: null,
    templates: [],
    filter: { status: "all", search: "", templateId: null },
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
describe("containerStore", () => {
  beforeEach(() => {
    resetStore();
    resetTauriMocks();
  });

  // -------------------------------------------------------------------------
  // fetchContainers
  // -------------------------------------------------------------------------
  describe("fetchContainers", () => {
    it("sets loading=true then populates containers from invoke", async () => {
      const containers = [makeContainer(), makeContainer({ id: "c-2", name: "py-dev" })];
      mockInvoke.mockResolvedValueOnce(containers);

      await useContainerStore.getState().fetchContainers();

      expect(mockInvoke).toHaveBeenCalledWith("container_list");
      expect(useContainerStore.getState().containers).toEqual(containers);
      expect(useContainerStore.getState().loading).toBe(false);
    });

    it("falls back gracefully when invoke fails (non-Tauri mode)", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("no Tauri"));

      await useContainerStore.getState().fetchContainers();

      // Should not throw, loading should be false
      expect(useContainerStore.getState().loading).toBe(false);
    });
  });

  // -------------------------------------------------------------------------
  // createContainer
  // -------------------------------------------------------------------------
  describe("createContainer", () => {
    it("invokes container_create and appends result to containers", async () => {
      const newContainer = makeContainer({ id: "c-new", name: "new-box" });
      mockInvoke.mockResolvedValueOnce(newContainer);

      const result = await useContainerStore.getState().createContainer({
        templateId: "node-dev",
        name: "new-box",
      });

      expect(mockInvoke).toHaveBeenCalledWith("container_create", {
        req: { templateId: "node-dev", name: "new-box" },
      });
      expect(result.id).toBe("c-new");
      expect(useContainerStore.getState().containers).toHaveLength(1);
    });

    it("creates mock container when invoke fails", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("no Tauri"));

      const result = await useContainerStore.getState().createContainer({
        templateId: "node-dev",
        name: "fallback-box",
      });

      expect(result.name).toBe("fallback-box");
      expect(result.templateId).toBe("node-dev");
      expect(useContainerStore.getState().containers).toHaveLength(1);
    });
  });

  // -------------------------------------------------------------------------
  // startContainer / stopContainer
  // -------------------------------------------------------------------------
  describe("startContainer", () => {
    it("updates container status to running", async () => {
      const c = makeContainer({ id: "c-1", status: "stopped" });
      useContainerStore.setState({ containers: [c] });
      mockInvoke.mockResolvedValueOnce(undefined);

      await useContainerStore.getState().startContainer("c-1");

      expect(useContainerStore.getState().containers[0].status).toBe("running");
    });
  });

  describe("stopContainer", () => {
    it("updates container status to stopped", async () => {
      const c = makeContainer({ id: "c-1", status: "running" });
      useContainerStore.setState({ containers: [c] });
      mockInvoke.mockResolvedValueOnce(undefined);

      await useContainerStore.getState().stopContainer("c-1");

      expect(useContainerStore.getState().containers[0].status).toBe("stopped");
    });
  });

  // -------------------------------------------------------------------------
  // deleteContainer
  // -------------------------------------------------------------------------
  describe("deleteContainer", () => {
    it("removes the container from the list", async () => {
      const c1 = makeContainer({ id: "c-1" });
      const c2 = makeContainer({ id: "c-2", name: "py-dev" });
      useContainerStore.setState({ containers: [c1, c2] });
      mockInvoke.mockResolvedValueOnce(undefined);

      await useContainerStore.getState().deleteContainer("c-1");

      expect(useContainerStore.getState().containers).toHaveLength(1);
      expect(useContainerStore.getState().containers[0].id).toBe("c-2");
    });

    it("clears selectedContainerId if deleted container was selected", async () => {
      const c = makeContainer({ id: "c-1" });
      useContainerStore.setState({ containers: [c], selectedContainerId: "c-1" });
      mockInvoke.mockResolvedValueOnce(undefined);

      await useContainerStore.getState().deleteContainer("c-1");

      expect(useContainerStore.getState().selectedContainerId).toBeNull();
    });

    it("preserves selectedContainerId if different container deleted", async () => {
      const c1 = makeContainer({ id: "c-1" });
      const c2 = makeContainer({ id: "c-2", name: "py-dev" });
      useContainerStore.setState({
        containers: [c1, c2],
        selectedContainerId: "c-2",
      });
      mockInvoke.mockResolvedValueOnce(undefined);

      await useContainerStore.getState().deleteContainer("c-1");

      expect(useContainerStore.getState().selectedContainerId).toBe("c-2");
    });
  });

  // -------------------------------------------------------------------------
  // fetchTemplates
  // -------------------------------------------------------------------------
  describe("fetchTemplates", () => {
    it("populates templates from invoke", async () => {
      const templates = [makeTemplate(), makeTemplate({ id: "python-dev", name: "Python Dev" })];
      mockInvoke.mockResolvedValueOnce(templates);

      await useContainerStore.getState().fetchTemplates();

      expect(useContainerStore.getState().templates).toEqual(templates);
    });

    it("uses mock templates when invoke fails", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("no Tauri"));

      await useContainerStore.getState().fetchTemplates();

      const templates = useContainerStore.getState().templates;
      expect(templates.length).toBeGreaterThanOrEqual(3);
      expect(templates.map((t) => t.id)).toEqual(
        expect.arrayContaining(["node-dev", "python-dev", "rust-dev"]),
      );
    });
  });

  // -------------------------------------------------------------------------
  // selectContainer
  // -------------------------------------------------------------------------
  describe("selectContainer", () => {
    it("sets selectedContainerId", () => {
      useContainerStore.getState().selectContainer("c-42");
      expect(useContainerStore.getState().selectedContainerId).toBe("c-42");
    });

    it("can clear selection with null", () => {
      useContainerStore.setState({ selectedContainerId: "c-42" });
      useContainerStore.getState().selectContainer(null);
      expect(useContainerStore.getState().selectedContainerId).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // filter + filteredContainers
  // -------------------------------------------------------------------------
  describe("filteredContainers", () => {
    const containers = [
      makeContainer({ id: "c-1", name: "node-01", image: "node:20-slim", status: "running", templateId: "node-dev" }),
      makeContainer({ id: "c-2", name: "py-dev", image: "python:3.12-slim", status: "stopped", templateId: "python-dev" }),
      makeContainer({ id: "c-3", name: "rust-box", image: "rust:1.75-slim", status: "running", templateId: "rust-dev" }),
    ];

    beforeEach(() => {
      useContainerStore.setState({ containers });
    });

    it("returns all containers with default filter", () => {
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(3);
    });

    it("filters by status", () => {
      useContainerStore.getState().setFilter({ status: "running" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(2);
      expect(filtered.every((c) => c.status === "running")).toBe(true);
    });

    it("filters by templateId", () => {
      useContainerStore.getState().setFilter({ templateId: "python-dev" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe("py-dev");
    });

    it("filters by search term (name)", () => {
      useContainerStore.getState().setFilter({ search: "rust" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe("rust-box");
    });

    it("filters by search term (case insensitive)", () => {
      useContainerStore.getState().setFilter({ search: "NODE" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe("node-01");
    });

    it("combines multiple filters", () => {
      useContainerStore.getState().setFilter({ status: "running", search: "rust" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe("rust-box");
    });

    it("returns empty when no matches", () => {
      useContainerStore.getState().setFilter({ search: "nonexistent" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(0);
    });
  });
});
