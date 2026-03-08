import { describe, expect, it } from "vitest"
import { formatSandboxError, parseSandboxError } from "../sandboxErrors"

const t = (key: string) => {
  const dict: Record<string, string> = {
    sandboxErrorRuntime: "Runtime",
    sandboxErrorPermission: "Permission",
    sandboxErrorTemplate: "Template",
    sandboxErrorResource: "Resources",
    sandboxErrorValidation: "Validation",
    sandboxErrorNotFound: "Not found",
    sandboxErrorInternal: "Internal",
    sandboxErrorRuntimeHint: "Start a Docker-compatible runtime or set DOCKER_HOST, then try again.",
    sandboxErrorPermissionHint: "Check runtime permissions, socket access, or current user privileges.",
    sandboxErrorResourceHint: "Check disk space, memory limits, or runtime quotas, then retry.",
  }

  return dict[key] ?? key
}

describe("parseSandboxError", () => {
  it("passes through non-prefixed errors", () => {
    expect(parseSandboxError("plain failure message")).toEqual({
      kind: null,
      message: "plain failure message",
    })
  })

  it("extracts kind and message from sandbox errors", () => {
    expect(parseSandboxError("sandbox_error::runtime::docker socket unavailable")).toEqual({
      kind: "runtime",
      message: "docker socket unavailable",
    })
  })
})

describe("formatSandboxError", () => {
  it("adds category labels and hints for runtime errors", () => {
    expect(formatSandboxError("sandbox_error::runtime::docker socket unavailable", t)).toBe(
      "Runtime: docker socket unavailable\nStart a Docker-compatible runtime or set DOCKER_HOST, then try again."
    )
  })

  it("formats categories without hints when none are defined", () => {
    expect(formatSandboxError("sandbox_error::template::missing image tag", t)).toBe(
      "Template: missing image tag"
    )
  })

  it("returns raw messages when no sandbox category exists", () => {
    expect(formatSandboxError("network timeout", t)).toBe("network timeout")
  })
})
