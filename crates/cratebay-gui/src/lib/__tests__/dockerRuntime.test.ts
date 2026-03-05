import { describe, expect, it } from "vitest"
import { isDockerRuntimeUnavailableError } from "../dockerRuntime"

describe("isDockerRuntimeUnavailableError", () => {
  it("detects missing docker socket errors", () => {
    expect(
      isDockerRuntimeUnavailableError(
        "No Docker socket found. Set DOCKER_HOST or start a Docker-compatible runtime."
      )
    ).toBe(true)
  })

  it("detects docker connection failures", () => {
    expect(
      isDockerRuntimeUnavailableError(
        "Failed to connect to Docker at /var/run/docker.sock: No such file or directory"
      )
    ).toBe(true)
  })

  it("ignores unrelated errors", () => {
    expect(isDockerRuntimeUnavailableError("permission denied reading volume metadata")).toBe(false)
  })
})
