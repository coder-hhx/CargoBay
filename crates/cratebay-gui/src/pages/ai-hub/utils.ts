import type { McpServerStatusDto } from "../../types"

export type TranslateFn = (key: string) => string

export function formatAiHubTime(value: string) {
  if (!value) return "-"
  const parsed = new Date(value)
  if (Number.isNaN(parsed.getTime())) {
    return value
  }
  return parsed.toLocaleString()
}

export function getMcpStatusLabel(status: McpServerStatusDto | null | undefined, t: TranslateFn) {
  if (status?.running) return t("mcpRunning")
  if (status?.status === "exited") return t("mcpExited")
  return t("mcpStopped")
}
