import { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { commandNeedsConfirmation } from "@/lib/aiSkills"
import { formatSandboxError } from "@/lib/sandboxErrors"
import { Assistant } from "./Assistant"
import { AiHubHeader } from "./ai-hub/AiHubHeader"
import { AiHubMcpTab } from "./ai-hub/AiHubMcpTab"
import { AiHubModelsTab } from "./ai-hub/AiHubModelsTab"
import { AiHubSandboxesTab } from "./ai-hub/AiHubSandboxesTab"
import type {
  AiHubActionResultDto,
  GpuStatusDto,
  AiSettings,
  McpServerEntry,
  McpServerStatusDto,
  OllamaModelDto,
  OllamaStatusDto,
  OllamaStorageInfoDto,
  SandboxAuditEventDto,
  SandboxCleanupResultDto,
  SandboxCreateRequest,
  SandboxCreateResultDto,
  SandboxExecResultDto,
  SandboxInfoDto,
  SandboxInspectDto,
  SandboxRuntimeUsageDto,
  SandboxTemplateDto,
} from "../types"

export type AiHubTab = "sandboxes" | "models" | "mcp" | "assistant"

interface AiHubProps {
  t: (key: string) => string
  initialTab?: AiHubTab
}

export function AiHub({ t, initialTab = "sandboxes" }: AiHubProps) {
  const [tab, setTab] = useState<AiHubTab>(initialTab)
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatusDto | null>(null)
  const [ollamaModels, setOllamaModels] = useState<OllamaModelDto[]>([])
  const [ollamaStorage, setOllamaStorage] = useState<OllamaStorageInfoDto | null>(null)
  const [gpuStatus, setGpuStatus] = useState<GpuStatusDto | null>(null)
  const [ollamaLoading, setOllamaLoading] = useState(false)
  const [ollamaError, setOllamaError] = useState("")
  const [ollamaNotice, setOllamaNotice] = useState("")
  const [ollamaPullName, setOllamaPullName] = useState("")
  const [ollamaActingName, setOllamaActingName] = useState("")
  const [sandboxTemplates, setSandboxTemplates] = useState<SandboxTemplateDto[]>([])
  const [sandboxes, setSandboxes] = useState<SandboxInfoDto[]>([])
  const [sandboxAudit, setSandboxAudit] = useState<SandboxAuditEventDto[]>([])
  const [sandboxInspect, setSandboxInspect] = useState<SandboxInspectDto | null>(null)
  const [sandboxRuntimeUsage, setSandboxRuntimeUsage] = useState<SandboxRuntimeUsageDto | null>(null)
  const [sandboxRuntimeLoading, setSandboxRuntimeLoading] = useState(false)
  const [sandboxRuntimeError, setSandboxRuntimeError] = useState("")
  const [sandboxLoading, setSandboxLoading] = useState(false)
  const [sandboxCreating, setSandboxCreating] = useState(false)
  const [sandboxActingId, setSandboxActingId] = useState("")
  const [sandboxError, setSandboxError] = useState("")
  const [sandboxNotice, setSandboxNotice] = useState("")
  const [sandboxInspectError, setSandboxInspectError] = useState("")
  const [sandboxSelectedTemplate, setSandboxSelectedTemplate] = useState("")
  const [sandboxName, setSandboxName] = useState("")
  const [sandboxOwner, setSandboxOwner] = useState("")
  const [sandboxCpu, setSandboxCpu] = useState<number | "">("")
  const [sandboxMemoryMb, setSandboxMemoryMb] = useState<number | "">("")
  const [sandboxTtlHours, setSandboxTtlHours] = useState<number | "">("")
  const [sandboxCommand, setSandboxCommand] = useState("")
  const [sandboxEnvLines, setSandboxEnvLines] = useState("")
  const [sandboxExecCommand, setSandboxExecCommand] = useState("")
  const [sandboxExecResult, setSandboxExecResult] = useState<SandboxExecResultDto | null>(null)
  const [mcpServers, setMcpServers] = useState<McpServerStatusDto[]>([])
  const [mcpDrafts, setMcpDrafts] = useState<McpServerEntry[]>([])
  const [mcpSelectedId, setMcpSelectedId] = useState("")
  const [mcpLogs, setMcpLogs] = useState<string[]>([])
  const [mcpExportClient, setMcpExportClient] = useState("codex")
  const [mcpExportValue, setMcpExportValue] = useState("")
  const [mcpLoading, setMcpLoading] = useState(false)
  const [mcpError, setMcpError] = useState("")
  const [mcpActingId, setMcpActingId] = useState("")

  const sandboxErrorMessage = useCallback((error: unknown) => formatSandboxError(String(error), t), [t])

  const refreshOllama = useCallback(async () => {
    setOllamaLoading(true)
    setOllamaError("")
    setOllamaNotice("")
    try {
      const [status, gpu] = await Promise.all([
        invoke<OllamaStatusDto>("ollama_status"),
        invoke<GpuStatusDto>("gpu_status").catch(() => null),
      ])
      setOllamaStatus(status)
      setGpuStatus(gpu)
      if (status.installed) {
        const storage = await invoke<OllamaStorageInfoDto>("ollama_storage_info")
        setOllamaStorage(storage)
      } else {
        setOllamaStorage(null)
      }
      if (status.running) {
        const models = await invoke<OllamaModelDto[]>("ollama_list_models")
        setOllamaModels(models)
      } else {
        setOllamaModels([])
      }
    } catch (e) {
      setOllamaError(String(e))
    } finally {
      setOllamaLoading(false)
    }
  }, [])

  const refreshSandboxes = useCallback(async () => {
    setSandboxLoading(true)
    setSandboxError("")
    try {
      const [templates, instances, auditEvents] = await Promise.all([
        invoke<SandboxTemplateDto[]>("sandbox_templates"),
        invoke<SandboxInfoDto[]>("sandbox_list"),
        invoke<SandboxAuditEventDto[]>("sandbox_audit_list", { limit: 40 }),
      ])
      setSandboxTemplates(templates)
      setSandboxes(instances)
      setSandboxAudit(auditEvents)
      setSandboxSelectedTemplate((prev) =>
        prev && templates.some((it) => it.id === prev) ? prev : (templates[0]?.id ?? "")
      )
    } catch (e) {
      setSandboxError(sandboxErrorMessage(e))
    } finally {
      setSandboxLoading(false)
    }
  }, [sandboxErrorMessage])

  const refreshMcp = useCallback(async () => {
    setMcpLoading(true)
    setMcpError("")
    try {
      const [settings, servers] = await Promise.all([
        invoke<AiSettings>("load_ai_settings"),
        invoke<McpServerStatusDto[]>("mcp_list_servers"),
      ])
      setMcpDrafts(settings.mcp_servers ?? [])
      setMcpServers(servers)
      setMcpSelectedId((prev) =>
        prev && (settings.mcp_servers ?? []).some((item) => item.id === prev)
          ? prev
          : (settings.mcp_servers?.[0]?.id ?? "")
      )
    } catch (e) {
      setMcpError(String(e))
    } finally {
      setMcpLoading(false)
    }
  }, [])

  useEffect(() => {
    refreshOllama()
  }, [refreshOllama])

  useEffect(() => {
    refreshSandboxes()
  }, [refreshSandboxes])

  useEffect(() => {
    refreshMcp()
  }, [refreshMcp])

  useEffect(() => {
    setTab(initialTab)
  }, [initialTab])

  useEffect(() => {
    if (!mcpSelectedId) {
      setMcpLogs([])
      return
    }
    invoke<string[]>("mcp_server_logs", { id: mcpSelectedId, limit: 120 })
      .then((logs) => setMcpLogs(logs))
      .catch(() => setMcpLogs([]))
  }, [mcpSelectedId, mcpServers])

  const formatGpuMetric = useCallback((value: number | null | undefined, suffix: string) => {
    if (value == null || Number.isNaN(value)) return "-"
    const rounded = suffix === "%" ? Math.round(value) : Math.round(value * 10) / 10
    return `${rounded}${suffix}`
  }, [])

  const formatGpuMemory = useCallback((used?: string | null, total?: string | null) => {
    if (used && total) return `${used} / ${total}`
    return total || used || "-"
  }, [])

  const formatUsagePercent = useCallback((value: number | null | undefined) => {
    if (value == null || Number.isNaN(value)) return "-"
    return `${Math.round(value * 10) / 10}%`
  }, [])

  const formatUsageMegabytes = useCallback((value: number | null | undefined) => {
    if (value == null || Number.isNaN(value)) return "-"
    return `${Math.round(value * 10) / 10} MB`
  }, [])

  const totalModelBytes = useMemo(
    () => ollamaModels.reduce((sum, m) => sum + (m.size_bytes || 0), 0),
    [ollamaModels]
  )

  const selectedSandboxTemplate = useMemo(
    () => sandboxTemplates.find((it) => it.id === sandboxSelectedTemplate) ?? null,
    [sandboxSelectedTemplate, sandboxTemplates]
  )

  const selectedMcpDraft = useMemo(
    () => mcpDrafts.find((item) => item.id === mcpSelectedId) ?? null,
    [mcpDrafts, mcpSelectedId]
  )

  const selectedMcpStatus = useMemo(
    () => mcpServers.find((item) => item.id === mcpSelectedId) ?? null,
    [mcpServers, mcpSelectedId]
  )

  const runningSandboxCount = useMemo(
    () => sandboxes.filter((s) => s.lifecycle_state === "running").length,
    [sandboxes]
  )

  const expiredSandboxCount = useMemo(
    () => sandboxes.filter((s) => s.is_expired).length,
    [sandboxes]
  )

  const loadSandboxInspect = useCallback(async (id: string) => {
    setSandboxRuntimeLoading(true)
    setSandboxRuntimeError("")
    setSandboxRuntimeUsage(null)
    try {
      const inspect = await invoke<SandboxInspectDto>("sandbox_inspect", { id })
      setSandboxInspect(inspect)
      try {
        const runtimeUsage = await invoke<SandboxRuntimeUsageDto>("sandbox_runtime_usage", { id })
        setSandboxRuntimeUsage(runtimeUsage)
      } catch (e) {
        setSandboxRuntimeError(sandboxErrorMessage(e))
      }
      return inspect
    } finally {
      setSandboxRuntimeLoading(false)
    }
  }, [sandboxErrorMessage])

  useEffect(() => {
    if (!selectedSandboxTemplate) return
    if (sandboxCpu === "" && sandboxMemoryMb === "" && sandboxTtlHours === "" && !sandboxCommand.trim()) {
      setSandboxCpu(selectedSandboxTemplate.cpu_default)
      setSandboxMemoryMb(selectedSandboxTemplate.memory_mb_default)
      setSandboxTtlHours(selectedSandboxTemplate.ttl_hours_default)
      setSandboxCommand(selectedSandboxTemplate.default_command)
    }
  }, [
    selectedSandboxTemplate,
    sandboxCommand,
    sandboxCpu,
    sandboxMemoryMb,
    sandboxTtlHours,
  ])

  const tabLabel = useMemo(() => {
    switch (tab) {
      case "models":
        return t("models")
      case "sandboxes":
        return t("sandboxes")
      case "mcp":
        return t("mcp")
      case "assistant":
        return t("assistant")
      default:
        return t("ai")
    }
  }, [tab, t])

  const applyTemplateDefaults = (template: SandboxTemplateDto) => {
    setSandboxCpu(template.cpu_default)
    setSandboxMemoryMb(template.memory_mb_default)
    setSandboxTtlHours(template.ttl_hours_default)
    setSandboxCommand(template.default_command)
  }

  const handleSelectSandboxTemplate = (templateId: string) => {
    setSandboxSelectedTemplate(templateId)
    const template = sandboxTemplates.find((it) => it.id === templateId)
    if (template) {
      applyTemplateDefaults(template)
    }
  }

  const handleCreateSandbox = async () => {
    if (!sandboxSelectedTemplate) {
      setSandboxError(t("sandboxTemplateRequired"))
      return
    }

    setSandboxCreating(true)
    setSandboxError("")
    try {
      const env = sandboxEnvLines
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean)

      const request: SandboxCreateRequest = {
        template_id: sandboxSelectedTemplate,
        name: sandboxName.trim() ? sandboxName.trim() : null,
        owner: sandboxOwner.trim() ? sandboxOwner.trim() : null,
        cpu_cores: sandboxCpu === "" ? null : sandboxCpu,
        memory_mb: sandboxMemoryMb === "" ? null : sandboxMemoryMb,
        ttl_hours: sandboxTtlHours === "" ? null : sandboxTtlHours,
        command: sandboxCommand.trim() ? sandboxCommand.trim() : null,
        env: env.length > 0 ? env : null,
      }

      const created = await invoke<SandboxCreateResultDto>("sandbox_create", { request })
      await loadSandboxInspect(created.id)
      setSandboxInspectError("")
      setSandboxName("")
      setSandboxEnvLines("")
      await refreshSandboxes()
    } catch (e) {
      setSandboxError(sandboxErrorMessage(e))
    } finally {
      setSandboxCreating(false)
    }
  }

  const handleSandboxAction = async (
    action: "start" | "stop" | "delete" | "inspect",
    item: SandboxInfoDto
  ) => {
    setSandboxError("")
    setSandboxInspectError("")

    if (action === "delete" && !window.confirm(t("confirmDeleteSandbox"))) {
      return
    }

    const actionKey = `${action}:${item.id}`
    setSandboxActingId(actionKey)
    try {
      if (action === "inspect") {
        await loadSandboxInspect(item.id)
      } else if (action === "start") {
        await invoke("sandbox_start", { id: item.id })
        await refreshSandboxes()
        if (sandboxInspect?.id === item.id) {
          await loadSandboxInspect(item.id)
        }
      } else if (action === "stop") {
        await invoke("sandbox_stop", { id: item.id })
        await refreshSandboxes()
        if (sandboxInspect?.id === item.id) {
          await loadSandboxInspect(item.id)
        }
      } else if (action === "delete") {
        await invoke("sandbox_delete", { id: item.id })
        if (sandboxInspect?.id === item.id) {
          setSandboxInspect(null)
          setSandboxRuntimeUsage(null)
          setSandboxRuntimeError("")
          setSandboxExecResult(null)
        }
        await refreshSandboxes()
      }
    } catch (e) {
      if (action === "inspect") {
        setSandboxInspectError(sandboxErrorMessage(e))
      } else {
        setSandboxError(sandboxErrorMessage(e))
      }
    } finally {
      setSandboxActingId("")
    }
  }

  const handleCleanupExpiredSandboxes = async () => {
    if (commandNeedsConfirmation("sandbox_cleanup_expired")) {
      const confirmed = window.confirm(
        `${t("assistantConfirmAction")}\n${t("sandboxCleanupExpired")}\nsandbox_cleanup_expired`
      )
      if (!confirmed) return
    }

    setSandboxError("")
    setSandboxNotice("")
    setSandboxActingId("cleanup")
    try {
      const result = await invoke<SandboxCleanupResultDto>("sandbox_cleanup_expired")
      if (result.message) {
        setSandboxNotice(result.message)
      }
      await refreshSandboxes()
    } catch (e) {
      setSandboxError(sandboxErrorMessage(e))
    } finally {
      setSandboxActingId("")
    }
  }

  const handleSandboxExec = async () => {
    const targetId = sandboxInspect?.id ?? sandboxes.find((item) => item.lifecycle_state === "running")?.id
    if (!targetId) {
      setSandboxInspectError(t("sandboxExecTitle"))
      return
    }
    const execCommand = sandboxExecCommand.trim()
    if (!execCommand) {
      return
    }
    if (commandNeedsConfirmation("sandbox_exec")) {
      const confirmed = window.confirm(
        `${t("assistantConfirmAction")}\n${t("sandboxExecTitle")}\n${execCommand}`
      )
      if (!confirmed) return
    }
    setSandboxActingId(`exec:${targetId}`)
    setSandboxInspectError("")
    setSandboxExecResult(null)
    try {
      const result = await invoke<SandboxExecResultDto>("sandbox_exec", {
        id: targetId,
        command: execCommand,
      })
      setSandboxExecResult(result)
    } catch (e) {
      setSandboxInspectError(sandboxErrorMessage(e))
    } finally {
      setSandboxActingId("")
    }
  }

  const handlePullModel = async () => {
    if (!ollamaPullName.trim()) {
      setOllamaError(t("ollamaModelRequired"))
      return
    }
    setOllamaActingName(`pull:${ollamaPullName.trim()}`)
    setOllamaError("")
    setOllamaNotice("")
    try {
      const result = await invoke<AiHubActionResultDto>("ollama_pull_model", { name: ollamaPullName.trim() })
      setOllamaPullName("")
      if (!result.ok && result.message) {
        setOllamaError(result.message)
      } else if (result.message) {
        setOllamaNotice(result.message)
      }
      await refreshOllama()
    } catch (e) {
      setOllamaError(String(e))
    } finally {
      setOllamaActingName("")
    }
  }

  const handleDeleteModel = async (name: string) => {
    if (!window.confirm(t("confirmDeleteModel"))) {
      return
    }
    setOllamaActingName(`delete:${name}`)
    setOllamaError("")
    setOllamaNotice("")
    try {
      const result = await invoke<AiHubActionResultDto>("ollama_delete_model", { name })
      if (!result.ok && result.message) {
        setOllamaError(result.message)
      } else if (result.message) {
        setOllamaNotice(result.message)
      }
      await refreshOllama()
    } catch (e) {
      setOllamaError(String(e))
    } finally {
      setOllamaActingName("")
    }
  }

  const updateSelectedMcpDraft = (updater: (draft: McpServerEntry) => McpServerEntry) => {
    let nextSelectedId = mcpSelectedId
    setMcpDrafts((prev) =>
      prev.map((item) => {
        if (item.id !== mcpSelectedId) return item
        const next = updater(item)
        nextSelectedId = next.id
        return next
      })
    )
    if (nextSelectedId !== mcpSelectedId) {
      setMcpSelectedId(nextSelectedId)
    }
  }

  const handleAddMcpServer = () => {
    let nextIndex = mcpDrafts.length + 1
    let id = `local-mcp-${nextIndex}`
    while (mcpDrafts.some((item) => item.id === id)) {
      nextIndex += 1
      id = `local-mcp-${nextIndex}`
    }
    const next: McpServerEntry = {
      id,
      name: `Local MCP ${nextIndex}`,
      command: "",
      args: [],
      env: [],
      working_dir: "",
      enabled: true,
      notes: "",
    }
    setMcpDrafts((prev) => [...prev, next])
    setMcpSelectedId(id)
    setMcpExportValue("")
  }

  const handleDeleteMcpServer = (id: string) => {
    if (!window.confirm(`${t("mcpDeleteServer")} ${id}?`)) {
      return
    }
    const nextDrafts = mcpDrafts.filter((item) => item.id !== id)
    setMcpDrafts(nextDrafts)
    if (mcpSelectedId === id) {
      setMcpSelectedId(nextDrafts[0]?.id ?? "")
    }
    setMcpExportValue("")
  }

  const handleSaveMcpRegistry = async () => {
    if (mcpDrafts.some((item) => !item.id.trim())) {
      setMcpError(t("mcpServerIdRequired"))
      return
    }
    if (mcpDrafts.some((item) => !item.command.trim())) {
      setMcpError(t("mcpServerCommandRequired"))
      return
    }
    setMcpActingId("save")
    setMcpError("")
    try {
      await invoke<McpServerEntry[]>("mcp_save_servers", {
        servers: mcpDrafts.map((item) => ({
          ...item,
          id: item.id.trim(),
          name: item.name.trim(),
          command: item.command.trim(),
          working_dir: item.working_dir.trim(),
          notes: item.notes.trim(),
          args: item.args.map((arg) => arg.trim()).filter(Boolean),
          env: item.env.map((entry) => entry.trim()).filter(Boolean),
        })),
      })
      await refreshMcp()
    } catch (e) {
      setMcpError(String(e))
    } finally {
      setMcpActingId("")
    }
  }

  const handleMcpAction = async (action: "start" | "stop" | "export", id?: string) => {
    const targetId = id ?? mcpSelectedId
    if (!targetId) return
    setMcpActingId(`${action}:${targetId}`)
    setMcpError("")
    try {
      if (action === "start") {
        await invoke<AiHubActionResultDto>("mcp_start_server", { id: targetId })
        await refreshMcp()
      } else if (action === "stop") {
        await invoke<AiHubActionResultDto>("mcp_stop_server", { id: targetId })
        await refreshMcp()
      } else {
        const content = await invoke<string>("mcp_export_client_config", { client: mcpExportClient })
        setMcpExportValue(content)
      }
    } catch (e) {
      setMcpError(String(e))
    } finally {
      setMcpActingId("")
    }
  }

  return (
    <div className="space-y-4">
      <AiHubHeader t={t} tabLabel={tabLabel} />
      <Tabs value={tab} onValueChange={(v) => setTab(v as AiHubTab)}>
        <TabsList variant="line" className="w-full justify-start">
          <TabsTrigger value="sandboxes">{t("sandboxes")}</TabsTrigger>
          <TabsTrigger value="models">{t("models")}</TabsTrigger>
          <TabsTrigger value="mcp" data-testid="aihub-tab-mcp">{t("mcp")}</TabsTrigger>
          <TabsTrigger value="assistant">{t("assistant")}</TabsTrigger>
        </TabsList>

        <AiHubModelsTab
          t={t}
          ollamaStatus={ollamaStatus}
          ollamaModels={ollamaModels}
          ollamaStorage={ollamaStorage}
          gpuStatus={gpuStatus}
          ollamaLoading={ollamaLoading}
          ollamaError={ollamaError}
          ollamaNotice={ollamaNotice}
          ollamaPullName={ollamaPullName}
          ollamaActingName={ollamaActingName}
          totalModelBytes={totalModelBytes}
          onRefreshOllama={refreshOllama}
          onDismissOllamaError={() => setOllamaError("")}
          onPullNameChange={(value) => setOllamaPullName(value)}
          onPullModel={handlePullModel}
          onDeleteModel={handleDeleteModel}
          formatGpuMetric={formatGpuMetric}
          formatGpuMemory={formatGpuMemory}
        />

        <AiHubSandboxesTab
          t={t}
          sandboxTemplates={sandboxTemplates}
          sandboxes={sandboxes}
          sandboxAudit={sandboxAudit}
          sandboxInspect={sandboxInspect}
          sandboxRuntimeUsage={sandboxRuntimeUsage}
          sandboxRuntimeLoading={sandboxRuntimeLoading}
          sandboxRuntimeError={sandboxRuntimeError}
          sandboxLoading={sandboxLoading}
          sandboxCreating={sandboxCreating}
          sandboxActingId={sandboxActingId}
          sandboxError={sandboxError}
          sandboxNotice={sandboxNotice}
          sandboxInspectError={sandboxInspectError}
          sandboxSelectedTemplate={sandboxSelectedTemplate}
          sandboxName={sandboxName}
          sandboxOwner={sandboxOwner}
          sandboxCpu={sandboxCpu}
          sandboxMemoryMb={sandboxMemoryMb}
          sandboxTtlHours={sandboxTtlHours}
          sandboxCommand={sandboxCommand}
          sandboxEnvLines={sandboxEnvLines}
          sandboxExecCommand={sandboxExecCommand}
          sandboxExecResult={sandboxExecResult}
          selectedSandboxTemplate={selectedSandboxTemplate}
          runningSandboxCount={runningSandboxCount}
          expiredSandboxCount={expiredSandboxCount}
          onCleanupExpiredSandboxes={handleCleanupExpiredSandboxes}
          onRefreshSandboxes={refreshSandboxes}
          onDismissSandboxError={() => setSandboxError("")}
          onSelectSandboxTemplate={handleSelectSandboxTemplate}
          onSandboxNameChange={(value) => setSandboxName(value)}
          onSandboxOwnerChange={(value) => setSandboxOwner(value)}
          onSandboxCpuChange={(value) => setSandboxCpu(value)}
          onSandboxMemoryMbChange={(value) => setSandboxMemoryMb(value)}
          onSandboxTtlHoursChange={(value) => setSandboxTtlHours(value)}
          onSandboxCommandChange={(value) => setSandboxCommand(value)}
          onSandboxEnvLinesChange={(value) => setSandboxEnvLines(value)}
          onCreateSandbox={handleCreateSandbox}
          onSandboxAction={handleSandboxAction}
          onDismissSandboxInspectError={() => setSandboxInspectError("")}
          onDismissSandboxRuntimeError={() => setSandboxRuntimeError("")}
          onSandboxExecCommandChange={(value) => setSandboxExecCommand(value)}
          onSandboxExec={handleSandboxExec}
          formatUsagePercent={formatUsagePercent}
          formatUsageMegabytes={formatUsageMegabytes}
        />

        <AiHubMcpTab
          t={t}
          mcpServers={mcpServers}
          mcpDrafts={mcpDrafts}
          mcpSelectedId={mcpSelectedId}
          mcpLogs={mcpLogs}
          mcpExportClient={mcpExportClient}
          mcpExportValue={mcpExportValue}
          mcpLoading={mcpLoading}
          mcpError={mcpError}
          mcpActingId={mcpActingId}
          selectedMcpDraft={selectedMcpDraft}
          selectedMcpStatus={selectedMcpStatus}
          onRefreshMcp={refreshMcp}
          onDismissMcpError={() => setMcpError("")}
          onAddMcpServer={handleAddMcpServer}
          onSaveMcpRegistry={handleSaveMcpRegistry}
          onSelectMcpServer={(id) => setMcpSelectedId(id)}
          onUpdateSelectedMcpDraft={updateSelectedMcpDraft}
          onDeleteMcpServer={handleDeleteMcpServer}
          onMcpAction={handleMcpAction}
          onMcpExportClientChange={(value) => setMcpExportClient(value)}
        />

        <TabsContent value="assistant" className="space-y-3">
          <Assistant t={t} />
        </TabsContent>
      </Tabs>
    </div>
  )
}
