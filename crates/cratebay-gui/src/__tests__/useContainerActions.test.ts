import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";

// Mock Tauri before importing hooks that use stores
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import { useContainerActions } from "@/hooks/useContainerActions";
import { useContainerStore } from "@/stores/containerStore";
import type { ContainerInfo } from "@/types/container";

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
describe("useContainerActions", () => {
  beforeEach(() => {
    resetStore();
    resetTauriMocks();
  });

  it("exposes refresh, create, start, stop, remove, loading, error", () => {
    const { result } = renderHook(() => useContainerActions());

    expect(typeof result.current.refresh).toBe("function");
    expect(typeof result.current.create).toBe("function");
    expect(typeof result.current.start).toBe("function");
    expect(typeof result.current.stop).toBe("function");
    expect(typeof result.current.remove).toBe("function");
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("refresh calls fetchContainers and fetchTemplates", async () => {
    const containers = [makeContainer()];
    const templates = [
      {
        id: "node-dev",
        name: "Node.js Dev",
        description: "Node.js",
        image: "node:20-slim",
        defaultCommand: "bash",
        defaultCpuCores: 2,
        defaultMemoryMb: 2048,
        tags: ["node"],
      },
    ];
    // fetchContainers → container_list, fetchTemplates → container_templates
    mockInvoke
      .mockResolvedValueOnce(containers)
      .mockResolvedValueOnce(templates);

    const { result } = renderHook(() => useContainerActions());

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockInvoke).toHaveBeenCalledWith("container_list");
    expect(mockInvoke).toHaveBeenCalledWith("container_templates");
    expect(useContainerStore.getState().containers).toEqual(containers);
    expect(useContainerStore.getState().templates).toEqual(templates);
  });

  it("create creates a container and returns it", async () => {
    const newContainer = makeContainer({ id: "c-new", name: "new-one" });
    mockInvoke.mockResolvedValueOnce(newContainer);

    const { result } = renderHook(() => useContainerActions());

    let created: ContainerInfo | undefined;
    await act(async () => {
      created = await result.current.create({ templateId: "node-dev", name: "new-one" });
    });

    expect(created?.id).toBe("c-new");
    expect(useContainerStore.getState().containers).toHaveLength(1);
  });

  it("start updates container status to running", async () => {
    useContainerStore.setState({
      containers: [makeContainer({ id: "c-1", status: "stopped" })],
    });
    mockInvoke.mockResolvedValueOnce(undefined);

    const { result } = renderHook(() => useContainerActions());

    await act(async () => {
      await result.current.start("c-1");
    });

    expect(useContainerStore.getState().containers[0].status).toBe("running");
  });

  it("stop updates container status to stopped", async () => {
    useContainerStore.setState({
      containers: [makeContainer({ id: "c-1", status: "running" })],
    });
    mockInvoke.mockResolvedValueOnce(undefined);

    const { result } = renderHook(() => useContainerActions());

    await act(async () => {
      await result.current.stop("c-1");
    });

    expect(useContainerStore.getState().containers[0].status).toBe("stopped");
  });

  it("remove deletes a container from the store", async () => {
    useContainerStore.setState({
      containers: [makeContainer({ id: "c-1" }), makeContainer({ id: "c-2", name: "py-dev" })],
    });
    mockInvoke.mockResolvedValueOnce(undefined);

    const { result } = renderHook(() => useContainerActions());

    await act(async () => {
      await result.current.remove("c-1");
    });

    expect(useContainerStore.getState().containers).toHaveLength(1);
    expect(useContainerStore.getState().containers[0].id).toBe("c-2");
  });

  it("reflects loading state from store", () => {
    useContainerStore.setState({ loading: true });

    const { result } = renderHook(() => useContainerActions());

    expect(result.current.loading).toBe(true);
  });

  it("reflects error state from store", () => {
    useContainerStore.setState({ error: "Something went wrong" });

    const { result } = renderHook(() => useContainerActions());

    expect(result.current.error).toBe("Something went wrong");
  });
});
