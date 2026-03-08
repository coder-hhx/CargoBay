export type SandboxErrorKind =
  | "runtime"
  | "permission"
  | "template"
  | "resource"
  | "validation"
  | "not_found"
  | "internal"

const PREFIX = "sandbox_error::"

export const parseSandboxError = (raw: string) => {
  if (!raw.startsWith(PREFIX)) {
    return { kind: null as SandboxErrorKind | null, message: raw }
  }

  const rest = raw.slice(PREFIX.length)
  const separator = rest.indexOf("::")
  if (separator === -1) {
    return { kind: null as SandboxErrorKind | null, message: raw }
  }

  const kind = rest.slice(0, separator) as SandboxErrorKind
  const message = rest.slice(separator + 2)
  return { kind, message }
}

export const formatSandboxError = (raw: string, t: (key: string) => string) => {
  const parsed = parseSandboxError(raw)
  if (!parsed.kind) return parsed.message

  const labels: Record<SandboxErrorKind, string> = {
    runtime: t("sandboxErrorRuntime"),
    permission: t("sandboxErrorPermission"),
    template: t("sandboxErrorTemplate"),
    resource: t("sandboxErrorResource"),
    validation: t("sandboxErrorValidation"),
    not_found: t("sandboxErrorNotFound"),
    internal: t("sandboxErrorInternal"),
  }

  const hints: Partial<Record<SandboxErrorKind, string>> = {
    runtime: t("sandboxErrorRuntimeHint"),
    permission: t("sandboxErrorPermissionHint"),
    resource: t("sandboxErrorResourceHint"),
  }

  const hint = hints[parsed.kind]
  return hint ? `${labels[parsed.kind]}: ${parsed.message}
${hint}` : `${labels[parsed.kind]}: ${parsed.message}`
}
