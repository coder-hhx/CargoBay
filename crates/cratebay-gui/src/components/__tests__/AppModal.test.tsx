import { render, screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"
import { AppModal } from "../AppModal"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

const t = (key: string) => {
  const messages: Record<string, string> = {
    copy: "复制",
    close: "关闭",
    newImageTag: "新镜像标签",
    package: "打包",
    done: "完成",
    imagePacked: "镜像已打包",
  }
  return messages[key] ?? key
}

const baseProps = {
  modalKind: "package" as const,
  modalTitle: "镜像打包",
  modalBody: "从容器打包镜像（docker commit）\n容器: web-server",
  modalCopyText: "",
  packageContainer: "web-server",
  packageTag: "nginx-snapshot:latest",
  setPackageTag: vi.fn(),
  packageLoading: false,
  setPackageLoading: vi.fn(),
  onClose: vi.fn(),
  onCopy: vi.fn(),
  onOpenTextModal: vi.fn(),
  onError: vi.fn(),
  onToast: vi.fn(),
  t,
}

describe("AppModal", () => {
  it("shows the package title and keeps a single copy button beside the tag input", () => {
    render(<AppModal {...baseProps} />)

    const dialog = screen.getByTestId("app-modal-package")
    expect(within(dialog).getByRole("heading", { name: "镜像打包" })).toBeInTheDocument()

    const tagInput = within(dialog).getByDisplayValue("nginx-snapshot:latest")
    const copyButton = within(tagInput.parentElement as HTMLElement).getByRole("button", { name: "复制" })

    expect(copyButton.parentElement).toBe(tagInput.parentElement)
    expect(copyButton).not.toHaveTextContent("复制")
    expect(within(dialog).getAllByRole("button", { name: "复制" })).toHaveLength(1)
  })

  it("copies the current package tag from the inline copy button", async () => {
    const user = userEvent.setup()
    const onCopy = vi.fn()

    render(<AppModal {...baseProps} onCopy={onCopy} />)

    const dialog = screen.getByTestId("app-modal-package")
    const tagInput = within(dialog).getByDisplayValue("nginx-snapshot:latest")
    const copyButton = within(tagInput.parentElement as HTMLElement).getByRole("button", { name: "复制" })

    await user.click(copyButton)

    expect(onCopy).toHaveBeenCalledWith("nginx-snapshot:latest")
  })
})
