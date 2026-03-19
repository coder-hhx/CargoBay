import { beforeEach, describe, expect, it, vi } from "vitest"
import { render, screen, within } from "@testing-library/react"
import App from "./App"

class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

vi.stubGlobal("ResizeObserver", ResizeObserverMock)

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    maximize: vi.fn().mockResolvedValue(undefined),
    setSize: vi.fn().mockResolvedValue(undefined),
    setPosition: vi.fn().mockResolvedValue(undefined),
    onResized: vi.fn().mockResolvedValue(() => {}),
    isMaximized: vi.fn().mockResolvedValue(false),
    innerSize: vi.fn().mockResolvedValue({ width: 1280, height: 800 }),
    outerPosition: vi.fn().mockResolvedValue({ x: 0, y: 0 }),
    scaleFactor: vi.fn().mockResolvedValue(1),
    minimize: vi.fn().mockResolvedValue(undefined),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
  }),
}))

vi.mock("./hooks/useContainers", () => ({
  useContainers: () => ({
    containers: [],
    running: [],
    groups: [],
    loading: false,
    error: "",
    runtimeMissing: false,
    acting: null,
    expandedGroups: new Set(),
    containerAction: vi.fn(),
    toggleGroup: vi.fn(),
    fetchContainers: vi.fn(),
    connected: true,
    setError: vi.fn(),
  }),
}))

vi.mock("./hooks/useImageSearch", () => ({
  useImageSearch: () => ({
    imgResults: [],
    doRunDirect: vi.fn(),
    doSearch: vi.fn(),
    doTags: vi.fn(),
    doRun: vi.fn(),
    doLoad: vi.fn(),
    doPush: vi.fn(),
  }),
}))

vi.mock("./hooks/useVolumes", () => ({
  useVolumes: () => ({
    volumes: [],
    loading: false,
    error: "",
    runtimeMissing: false,
    fetchVolumes: vi.fn(),
    createVolume: vi.fn(),
    inspectVolume: vi.fn(),
    removeVolume: vi.fn(),
  }),
}))

vi.mock("./hooks/useModal", () => ({
  useModal: () => ({
    kind: "",
    open: false,
    title: "",
    body: "",
    copyText: "",
    packageName: "",
    openTextModal: vi.fn(),
    openPackageModal: vi.fn(),
    closeModal: vi.fn(),
  }),
}))

vi.mock("./pages/Dashboard", () => ({ Dashboard: () => <div>Dashboard Page</div> }))
vi.mock("./pages/Containers", () => ({ Containers: () => <div>Containers Page</div> }))
vi.mock("./pages/Images", () => ({ Images: () => <div>Images Page</div> }))
vi.mock("./pages/Volumes", () => ({ Volumes: () => <div>Volumes Page</div> }))
vi.mock("./pages/Settings", () => ({ Settings: () => <div>Settings Page</div> }))
vi.mock("./pages/AiHub", () => ({ AiHub: () => <div>AI Page</div> }))
vi.mock("./components/AppModal", () => ({ AppModal: () => null }))
vi.mock("./components/UpdateChecker", () => ({ UpdateChecker: () => null }))

beforeEach(() => {
  localStorage.clear()
  vi.clearAllMocks()
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn().mockImplementation(() => ({
      matches: false,
      media: "(prefers-color-scheme: dark)",
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  })
})

describe("App navigation", () => {
  it("hides experimental routes from the sidebar", () => {
    render(<App />)

    const primary = screen.getByTestId("nav-primary-section")

    expect(within(primary).getByTestId("nav-dashboard")).toBeInTheDocument()
    expect(within(primary).getByTestId("nav-ai")).toBeInTheDocument()
    expect(within(primary).getByTestId("nav-containers")).toBeInTheDocument()
    expect(within(primary).queryByTestId("nav-vms")).not.toBeInTheDocument()
    expect(within(primary).queryByTestId("nav-kubernetes")).not.toBeInTheDocument()
    expect(screen.queryByTestId("nav-experimental-section")).not.toBeInTheDocument()
    expect(screen.queryByText("Experimental")).not.toBeInTheDocument()
  })
})
