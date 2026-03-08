import type { AiSkillDefinition, AiSkillExecutionResult } from "../types"

const assistantCommandsNeedingConfirmation = new Set([
  "remove_container",
  "vm_delete",
  "ollama_delete_model",
  "sandbox_delete",
  "sandbox_cleanup_expired",
  "sandbox_exec",
])

const mcpDestructiveKeywords = [
  "delete",
  "remove",
  "destroy",
  "drop",
  "wipe",
  "prune",
  "terminate",
  "kill",
  "uninstall",
  "purge",
]

const mcpActionHasKeyword = (action: string, keyword: string) =>
  action === keyword ||
  action.split(/[^a-z0-9]+/i).some((segment) => segment === keyword) ||
  action.includes(keyword)

export const skillUsesPromptInput = (skill: AiSkillDefinition) =>
  skill.executor === "agent_cli_preset"

export const commandNeedsConfirmation = (command: string) =>
  assistantCommandsNeedingConfirmation.has(command.trim())

export const mcpActionNeedsConfirmation = (action: string) => {
  const actionLower = action.trim().toLowerCase()
  return mcpDestructiveKeywords.some((keyword) => mcpActionHasKeyword(actionLower, keyword))
}

export const defaultSkillInputValue = (skill: AiSkillDefinition) => {
  if (skillUsesPromptInput(skill)) return ""

  switch (skill.target) {
    case "k8s_list_pods":
      return JSON.stringify({ namespace: "default" }, null, 2)
    case "sandbox_exec":
      return JSON.stringify(
        { id: "<sandbox-id>", command: "echo hello from sandbox" },
        null,
        2
      )
    default:
      return "{}"
  }
}

export const skillNeedsConfirmation = (skill: AiSkillDefinition) => {
  if (skillUsesPromptInput(skill)) return false
  if (skill.executor === "mcp_action") return mcpActionNeedsConfirmation(skill.target)
  if (skill.executor === "assistant_step" || skill.executor === "sandbox_action") {
    return commandNeedsConfirmation(skill.target)
  }
  return false
}

export const formatSkillExecutionOutput = (
  result: Pick<AiSkillExecutionResult, "output" | "request_id">,
  doneLabel: string
) => {
  const output =
    typeof result.output === "string"
      ? result.output
      : JSON.stringify(result.output, null, 2) || doneLabel

  return result.request_id ? `${output}
request_id=${result.request_id}` : output
}
