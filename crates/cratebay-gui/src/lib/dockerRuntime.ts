const dockerRuntimeErrorPatterns = [
  "no docker socket found",
  "failed to connect to docker",
  "cannot connect to the docker daemon",
  "docker daemon",
  "docker named pipe",
]

export function isDockerRuntimeUnavailableError(message: string): boolean {
  const normalized = String(message || "").toLowerCase()
  if (!normalized) return false
  return dockerRuntimeErrorPatterns.some((pattern) => normalized.includes(pattern))
}
