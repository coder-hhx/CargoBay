import { useState, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { messages, langNames } from "./i18n/messages"
import "./App.css"

interface ContainerInfo {
  id: string
  name: string
  image: string
  state: string
  status: string
  ports: string
}

interface ImageSearchResult {
  source: string
  reference: string
  description: string
  stars?: number
  pulls?: number
  official: boolean
}

interface RunContainerResult {
  id: string
  name: string
  image: string
  login_cmd: string
}

interface SharedDirectoryDto {
  tag: string
  host_path: string
  guest_path: string
  read_only: boolean
}

interface VmInfoDto {
  id: string
  name: string
  state: string
  cpus: number
  memory_mb: number
  disk_gb: number
  rosetta_enabled: boolean
  mounts: SharedDirectoryDto[]
}

type NavPage = "dashboard" | "containers" | "vms" | "images" | "settings"
type Theme = "dark" | "light"
type ModalKind = "" | "text" | "package"

interface ContainerGroup {
  key: string
  containers: ContainerInfo[]
  runningCount: number
  stoppedCount: number
}

function containerGroupCandidates(name: string): string[] {
  const trimmed = name.trim()
  if (!trimmed) return []

  const out = new Set<string>()
  out.add(trimmed)

  const base = trimmed.replace(/[-_]\d+$/, "")
  if (base) out.add(base)

  for (let i = 0; i < trimmed.length; i++) {
    const ch = trimmed[i]
    if (ch === "-" || ch === "_") {
      const prefix = trimmed.slice(0, i)
      if (prefix) out.add(prefix)
    }
  }

  return Array.from(out)
}

function groupContainersByNamePrefix(containers: ContainerInfo[]): ContainerGroup[] {
  const candidateCounts = new Map<string, number>()
  const candidatesById = new Map<string, string[]>()

  for (const c of containers) {
    const name = (c.name || c.id).trim()
    const uniqueCandidates = Array.from(new Set(containerGroupCandidates(name)))
    candidatesById.set(c.id, uniqueCandidates)
    for (const cand of uniqueCandidates) {
      candidateCounts.set(cand, (candidateCounts.get(cand) || 0) + 1)
    }
  }

  const groups = new Map<string, ContainerInfo[]>()
  for (const c of containers) {
    const name = (c.name || c.id).trim()
    const candidates = candidatesById.get(c.id) || containerGroupCandidates(name)

    let bestKey = name
    let bestCount = 1
    let bestLen = 0

    for (const cand of candidates) {
      const count = candidateCounts.get(cand) || 0
      if (count < 2) continue
      if (count > bestCount || (count === bestCount && cand.length > bestLen)) {
        bestKey = cand
        bestCount = count
        bestLen = cand.length
      }
    }

    const existing = groups.get(bestKey)
    if (existing) {
      existing.push(c)
    } else {
      groups.set(bestKey, [c])
    }
  }

  const out: ContainerGroup[] = []
  for (const [key, items] of groups) {
    const runningCount = items.filter((c) => c.state === "running").length
    const stoppedCount = items.length - runningCount
    items.sort((a, b) => {
      const ar = a.state === "running"
      const br = b.state === "running"
      if (ar !== br) return ar ? -1 : 1
      const an = (a.name || a.id).localeCompare(b.name || b.id)
      if (an !== 0) return an
      return a.id.localeCompare(b.id)
    })
    out.push({ key, containers: items, runningCount, stoppedCount })
  }

  out.sort((a, b) => {
    const ar = a.runningCount > 0
    const br = b.runningCount > 0
    if (ar !== br) return ar ? -1 : 1
    if (a.containers.length !== b.containers.length) return b.containers.length - a.containers.length
    return a.key.localeCompare(b.key)
  })

  return out
}

/* SVG icons — Lucide-style stroke icons */
const I = {
  dashboard: <svg viewBox="0 0 24 24"><rect x="3" y="3" width="7" height="9" rx="1"/><rect x="14" y="3" width="7" height="5" rx="1"/><rect x="14" y="12" width="7" height="9" rx="1"/><rect x="3" y="16" width="7" height="5" rx="1"/></svg>,
  box: <svg viewBox="0 0 24 24"><path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg>,
  server: <svg viewBox="0 0 24 24"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/></svg>,
  layers: <svg viewBox="0 0 24 24"><polygon points="12 2 2 7 12 12 22 7 12 2"/><polyline points="2 17 12 22 22 17"/><polyline points="2 12 12 17 22 12"/></svg>,
  settings: <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg>,
  stop: <svg viewBox="0 0 24 24"><rect x="6" y="6" width="12" height="12" rx="1"/></svg>,
  play: <svg viewBox="0 0 24 24"><polygon points="5 3 19 12 5 21 5 3"/></svg>,
  trash: <svg viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>,
  plus: <svg viewBox="0 0 24 24"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>,
  cpu: <svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M15 2v2M15 20v2M2 15h2M2 9h2M20 15h2M20 9h2M9 2v2M9 20v2"/></svg>,
  memory: <svg viewBox="0 0 24 24"><path d="M6 19v2M10 19v2M14 19v2M18 19v2M4 15V5a2 2 0 012-2h12a2 2 0 012 2v10a2 2 0 01-2 2H6a2 2 0 01-2-2z"/><path d="M8 7h8M8 11h8"/></svg>,
  terminal: <svg viewBox="0 0 24 24"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/><rect x="2" y="3" width="20" height="18" rx="2"/></svg>,
  copy: <svg viewBox="0 0 24 24"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>,
  refresh: <svg viewBox="0 0 24 24"><path d="M21 12a9 9 0 11-3-6.7"/><polyline points="21 3 21 9 15 9"/></svg>,
  chevronRight: <svg viewBox="0 0 24 24"><polyline points="9 18 15 12 9 6"/></svg>,
  chevronDown: <svg viewBox="0 0 24 24"><polyline points="6 9 12 15 18 9"/></svg>,
}

function App() {
  const [containers, setContainers] = useState<ContainerInfo[]>([])
  const [error, setError] = useState("")
  const [loading, setLoading] = useState(true)
  const [connected, setConnected] = useState(false)
  const [activePage, setActivePage] = useState<NavPage>("dashboard")
  const [acting, setActing] = useState("")
  const [modalKind, setModalKind] = useState<ModalKind>("")
  const [modalTitle, setModalTitle] = useState("")
  const [modalBody, setModalBody] = useState("")
  const [modalCopyText, setModalCopyText] = useState("")
  const [toast, setToast] = useState("")
  const [packageContainer, setPackageContainer] = useState("")
  const [packageTag, setPackageTag] = useState("")
  const [packageLoading, setPackageLoading] = useState(false)
  const [expandedContainerGroups, setExpandedContainerGroups] = useState<Record<string, boolean>>({})

  // Images state
  const [imgQuery, setImgQuery] = useState("")
  const [imgSource, setImgSource] = useState("all")
  const [imgLimit, setImgLimit] = useState(20)
  const [imgResults, setImgResults] = useState<ImageSearchResult[]>([])
  const [imgSearching, setImgSearching] = useState(false)
  const [imgError, setImgError] = useState("")
  const [imgTags, setImgTags] = useState<string[]>([])
  const [imgTagsRef, setImgTagsRef] = useState("")
  const [imgTagsLoading, setImgTagsLoading] = useState(false)
  const [runImage, setRunImage] = useState("")
  const [runName, setRunName] = useState("")
  const [runCpus, setRunCpus] = useState<number | "">("")
  const [runMem, setRunMem] = useState<number | "">("")
  const [runPull, setRunPull] = useState(true)
  const [runLoading, setRunLoading] = useState(false)
  const [runResult, setRunResult] = useState<RunContainerResult | null>(null)
  const [loadPath, setLoadPath] = useState("")
  const [loadLoading, setLoadLoading] = useState(false)
  const [pushRef, setPushRef] = useState("")
  const [pushLoading, setPushLoading] = useState(false)

  // VMs state
  const [vms, setVms] = useState<VmInfoDto[]>([])
  const [vmLoading, setVmLoading] = useState(false)
  const [vmError, setVmError] = useState("")
  const [vmName, setVmName] = useState("")
  const [vmCpus, setVmCpus] = useState(2)
  const [vmMem, setVmMem] = useState(2048)
  const [vmDisk, setVmDisk] = useState(20)
  const [vmRosetta, setVmRosetta] = useState(false)
  const [vmActing, setVmActing] = useState("")
  const [vmLoginUser, setVmLoginUser] = useState("root")
  const [vmLoginHost, setVmLoginHost] = useState("127.0.0.1")
  const [vmLoginPort, setVmLoginPort] = useState<number | "">(2222)

  // VirtioFS mount state (per action)
  const [mountVmId, setMountVmId] = useState("")
  const [mountTag, setMountTag] = useState("")
  const [mountHostPath, setMountHostPath] = useState("")
  const [mountGuestPath, setMountGuestPath] = useState("/mnt/host")
  const [mountReadonly, setMountReadonly] = useState(false)

  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem("theme") as Theme) || "dark")
  const normalizeLang = (value: string | null) => (value === "zh" ? "zh" : "en")
  const [lang, setLang] = useState(() => normalizeLang(localStorage.getItem("lang")))

  const t = (key: string) => messages[lang]?.[key] || messages.en[key] || key

  useEffect(() => { localStorage.setItem("theme", theme) }, [theme])
  useEffect(() => { localStorage.setItem("lang", lang) }, [lang])
  useEffect(() => {
    if (!toast) return
    const tmr = setTimeout(() => setToast(""), 2200)
    return () => clearTimeout(tmr)
  }, [toast])

  const openTextModal = (title: string, body: string, copyText?: string) => {
    setModalKind("text")
    setModalTitle(title)
    setModalBody(body)
    setModalCopyText(copyText ?? body)
  }
  const openPackageModal = (container: string, defaultTag: string) => {
    setModalKind("package")
    setModalTitle(t("packageImage"))
    setPackageContainer(container)
    setPackageTag(defaultTag)
    setModalBody(`${t("packageFromContainer")}\n${t("container")}: ${container}`)
    setModalCopyText("")
  }
  const closeModal = () => {
    setModalKind("")
    setModalTitle("")
    setModalBody("")
    setModalCopyText("")
    setPackageContainer("")
    setPackageTag("")
    setPackageLoading(false)
  }
  const copyText = async (text: string) => {
    try { await navigator.clipboard.writeText(text); setToast(t("copied")) }
    catch { setToast(t("copyFailed")) }
  }

  const containerAction = async (cmd: string, id: string) => {
    setActing(id)
    try { await invoke(cmd, { id }); await fetchContainers() }
    catch (e) { setError(String(e)) }
    finally { setActing("") }
  }

  const fetchContainers = async () => {
    try {
      const result = await invoke<ContainerInfo[]>("list_containers")
      setContainers(result); setError(""); setConnected(true)
    } catch (e) { setError(String(e)); setConnected(false) }
    finally { setLoading(false) }
  }

  const fetchVms = async () => {
    setVmLoading(true)
    try {
      const result = await invoke<VmInfoDto[]>("vm_list")
      setVms(result)
      setVmError("")
    } catch (e) {
      setVmError(String(e))
    } finally {
      setVmLoading(false)
    }
  }

  useEffect(() => {
    fetchContainers()
    const iv = setInterval(fetchContainers, 3000)
    return () => clearInterval(iv)
  }, [])
  useEffect(() => {
    fetchVms()
  }, [])

  const running = containers.filter(c => c.state === "running")

  const toggleContainerGroup = (key: string) => {
    setExpandedContainerGroups((prev) => ({ ...prev, [key]: !prev[key] }))
  }

  /* CargoBay 自己的导航结构 — 按功能维度划分 */
  const navItems: { page: NavPage; icon: React.ReactNode; count?: number; soon?: boolean }[] = [
    { page: "dashboard", icon: I.dashboard },
    { page: "containers", icon: I.box, count: containers.length },
    { page: "vms", icon: I.server },
    { page: "images", icon: I.layers },
  ]

  const pageNames: Record<NavPage, string> = {
    dashboard: t("dashboard"), containers: t("containers"),
    vms: t("vms"), images: t("images"), settings: t("settings"),
  }

  const renderDashboard = () => (
    <div className="dashboard">
      <div className="dash-cards">
        <div className="dash-card" onClick={() => setActivePage("containers")}>
          <div className="dash-card-icon">{I.box}</div>
          <div className="dash-card-info">
            <div className="dash-card-value">{containers.length}</div>
            <div className="dash-card-label">{t("containers")}</div>
          </div>
          <div className="dash-card-sub">
            {running.length > 0 && <span className="dash-running">{running.length} {t("runningCount")}</span>}
          </div>
        </div>
        <div className="dash-card" onClick={() => setActivePage("vms")}>
          <div className="dash-card-icon">{I.server}</div>
          <div className="dash-card-info">
            <div className="dash-card-value">{vms.length}</div>
            <div className="dash-card-label">{t("vms")}</div>
          </div>
          <div className="dash-card-sub">{vms.length > 0 && <span className="dash-running">{vms.filter(v => v.state === "running").length} {t("runningCount")}</span>}</div>
        </div>
        <div className="dash-card" onClick={() => setActivePage("images")}>
          <div className="dash-card-icon">{I.layers}</div>
          <div className="dash-card-info">
            <div className="dash-card-value">{imgResults.length}</div>
            <div className="dash-card-label">{t("images")}</div>
          </div>
          <div className="dash-card-sub">{imgResults.length > 0 && <span className="dash-badge">{t("searchResults")}</span>}</div>
        </div>
        <div className="dash-card">
          <div className="dash-card-icon">{I.cpu}</div>
          <div className="dash-card-info">
            <div className="dash-card-value">{connected ? "OK" : "--"}</div>
            <div className="dash-card-label">{t("system")}</div>
          </div>
          <div className="dash-card-sub">
            <span className={`dot ${connected ? "on" : "off"}`} />
            <span>{connected ? "Docker " + t("connected") : t("disconnected")}</span>
          </div>
        </div>
      </div>

      {running.length > 0 && <>
        <div className="section-title">{t("running")} ({running.length})</div>
        {running.slice(0, 5).map(c => (
          <div className="container-card" key={c.id}>
            <div className="card-icon">{I.box}</div>
            <div className="card-body">
              <div className="card-name">{c.name}</div>
              <div className="card-meta">{c.image} · {c.ports || c.id}</div>
            </div>
            <div className="card-status">
              <span className="dot running" />
              <span>{c.status}</span>
            </div>
          </div>
        ))}
        {running.length > 5 && (
          <div className="view-all" onClick={() => setActivePage("containers")}>
            {t("viewAll")} ({running.length})
          </div>
        )}
      </>}
    </div>
  )

  const renderContainers = () => {
    if (loading) return <div className="loading"><div className="spinner" />{t("loadingContainers")}</div>
    if (error) return <div className="error-msg">{error}</div>
    if (containers.length === 0) return (
      <div className="empty-state">
        <div className="empty-icon">{I.box}</div>
        <h3>{t("noContainers")}</h3>
        <p>{t("runContainerTip")}</p>
        <code>docker run -it -p 80:80 docker/getting-started</code>
      </div>
    )

    const renderContainerCard = (c: ContainerInfo, opts?: { child?: boolean }) => {
      const isRunning = c.state === "running"
      const name = c.name || c.id
      const meta = isRunning ? (c.ports || c.id) : c.id
      const childClass = opts?.child ? " container-child" : ""
      return (
        <div className={`container-card${childClass}`} key={c.id}>
          <div className={`card-icon${isRunning ? "" : " stopped"}`}>{I.box}</div>
          <div className="card-body">
            <div className="card-name">{name}</div>
            <div className="card-meta">{c.image} · {meta}</div>
          </div>
          <div className="card-status">
            <span className={`dot ${isRunning ? "running" : "stopped"}`} />
            <span>{c.status}</span>
          </div>
          <div className="card-actions">
            <button
              className="action-btn"
              disabled={acting === c.id}
              onClick={async () => {
                const target = c.name || c.id
                const cmd = await invoke<string>("container_login_cmd", { container: target, shell: "/bin/sh" })
                openTextModal(t("loginCommand"), cmd, cmd)
              }}
              title={t("loginCommand")}
            >
              {I.terminal}
            </button>
            {isRunning ? (
              <button className="action-btn" disabled={acting === c.id} onClick={() => containerAction("stop_container", c.id)} title={t("stop")}>{I.stop}</button>
            ) : (
              <button className="action-btn" disabled={acting === c.id} onClick={() => containerAction("start_container", c.id)} title={t("start")}>{I.play}</button>
            )}
            <button
              className="action-btn"
              disabled={acting === c.id}
              onClick={() => {
                const target = c.name || c.id
                const defaultTag = `${(c.image || "image").split(":")[0]}-snapshot:latest`
                openPackageModal(target, defaultTag)
              }}
              title={t("packageImage")}
            >
              {I.layers}
            </button>
            <button className="action-btn danger" disabled={acting === c.id} onClick={() => containerAction("remove_container", c.id)} title={t("delete")}>{I.trash}</button>
          </div>
        </div>
      )
    }

    const groups = groupContainersByNamePrefix(containers)

    return <>
      {groups.map((g) => {
        if (g.containers.length <= 1) {
          return renderContainerCard(g.containers[0])
        }

        const expanded = !!expandedContainerGroups[g.key]
        const hasRunning = g.runningCount > 0
        return (
          <div className="container-group" key={g.key}>
            <div
              className={`container-card container-group-header${expanded ? " expanded" : ""}`}
              role="button"
              tabIndex={0}
              onClick={() => toggleContainerGroup(g.key)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault()
                  toggleContainerGroup(g.key)
                }
              }}
              title={expanded ? "Collapse" : "Expand"}
            >
              <div className="card-icon">{I.box}</div>
              <div className="card-body">
                <div className="card-name">{g.key}</div>
                <div className="card-meta">
                  {t("running")}: {g.runningCount} · {t("stopped")}: {g.stoppedCount}
                </div>
              </div>
              <div className="card-status">
                <span className={`dot ${hasRunning ? "running" : "stopped"}`} />
                <span>{hasRunning ? t("running") : t("stopped")}</span>
              </div>
              <div className="group-chevron" aria-hidden="true">
                {expanded ? I.chevronDown : I.chevronRight}
              </div>
            </div>
            {expanded && (
              <div className="container-group-children">
                {g.containers.map((c) => renderContainerCard(c, { child: true }))}
              </div>
            )}
          </div>
        )
      })}
    </>
  }

  const renderImages = () => {
    const canTags = (ref: string) => ref.includes(".") || ref.includes(":") || ref.startsWith("localhost/")

    const doSearch = async () => {
      setImgSearching(true)
      setImgError("")
      setRunResult(null)
      try {
        const result = await invoke<ImageSearchResult[]>("image_search", { query: imgQuery, source: imgSource, limit: imgLimit })
        setImgResults(result)
      } catch (e) {
        setImgError(String(e))
      } finally {
        setImgSearching(false)
      }
    }

    const doTags = async (reference: string) => {
      setImgTagsLoading(true)
      setImgTagsRef(reference)
      try {
        const tags = await invoke<string[]>("image_tags", { reference, limit: 50 })
        setImgTags(tags)
      } catch (e) {
        setImgTags([])
        setImgError(String(e))
      } finally {
        setImgTagsLoading(false)
      }
    }

    const doRun = async () => {
      if (!runImage) return
      setRunLoading(true)
      setImgError("")
      try {
        const result = await invoke<RunContainerResult>("docker_run", {
          image: runImage,
          name: runName.trim() ? runName.trim() : null,
          cpus: runCpus === "" ? null : runCpus,
          memory_mb: runMem === "" ? null : runMem,
          pull: runPull,
        })
        setRunResult(result)
        setToast(t("containerCreated"))
        await fetchContainers()
      } catch (e) {
        setImgError(String(e))
      } finally {
        setRunLoading(false)
      }
    }

    const doLoad = async () => {
      if (!loadPath.trim()) return
      setLoadLoading(true)
      setImgError("")
      try {
        const out = await invoke<string>("image_load", { path: loadPath.trim() })
        openTextModal(t("imageLoaded"), out || t("done"), out || t("done"))
        setToast(t("done"))
      } catch (e) {
        setImgError(String(e))
      } finally {
        setLoadLoading(false)
      }
    }

    const doPush = async () => {
      if (!pushRef.trim()) return
      setPushLoading(true)
      setImgError("")
      try {
        const out = await invoke<string>("image_push", { reference: pushRef.trim() })
        openTextModal(t("imagePushed"), out || t("done"), out || t("done"))
        setToast(t("done"))
      } catch (e) {
        setImgError(String(e))
      } finally {
        setPushLoading(false)
      }
    }

    return (
      <div className="page">
        <div className="toolbar">
          <input
            className="input"
            placeholder={t("searchImages")}
            value={imgQuery}
            onChange={e => setImgQuery(e.target.value)}
            onKeyDown={e => e.key === "Enter" && doSearch()}
          />
          <select className="select" value={imgSource} onChange={e => setImgSource(e.target.value)}>
            <option value="all">{t("sourceAll")}</option>
            <option value="dockerhub">{t("sourceDockerHub")}</option>
            <option value="quay">{t("sourceQuay")}</option>
          </select>
          <input
            className="input small"
            type="number"
            min={1}
            max={100}
            value={imgLimit}
            onChange={e => setImgLimit(Number(e.target.value) || 20)}
          />
          <button className="btn primary" disabled={imgSearching || !imgQuery.trim()} onClick={doSearch}>
            {imgSearching ? t("searching") : t("search")}
          </button>
          <button className="btn" onClick={() => { setImgResults([]); setImgTags([]); setImgError(""); setRunResult(null) }}>
            {t("clear")}
          </button>
        </div>

        {imgError && <div className="error-inline">{imgError}</div>}

        <div className="grid2">
          <div className="panel">
            <div className="panel-title">{t("results")}</div>
            {imgResults.length === 0 ? (
              <div className="hint">{t("searchHint")}</div>
            ) : (
              <div className="table">
                <div className="tr head">
                  <div>{t("source")}</div>
                  <div>{t("image")}</div>
                  <div className="right">{t("stars")}</div>
                  <div className="right">{t("pulls")}</div>
                  <div className="grow">{t("description")}</div>
                  <div className="right">{t("actions")}</div>
                </div>
                {imgResults.map((r, idx) => (
                  <div className="tr" key={`${r.source}-${r.reference}-${idx}`}>
                    <div className="badge">{r.source}</div>
                    <div className="mono">{r.reference}{r.official ? ` (${t("official")})` : ""}</div>
                    <div className="right">{r.stars ?? "-"}</div>
                    <div className="right">{r.pulls ?? "-"}</div>
                    <div className="grow">{r.description || "-"}</div>
                    <div className="right">
                      <button className="btn small" onClick={() => { setRunImage(r.reference); setRunName(""); setRunResult(null) }}>
                        {t("run")}
                      </button>
                      <button className="btn small" disabled={!canTags(r.reference)} onClick={() => doTags(r.reference)}>
                        {t("tags")}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="panel">
            <div className="panel-title">{t("runContainer")}</div>
            <div className="form">
              <div className="row">
                <label>{t("image")}</label>
                <input className="input" value={runImage} onChange={e => setRunImage(e.target.value)} placeholder="nginx:latest" />
              </div>
              <div className="row">
                <label>{t("nameOptional")}</label>
                <input className="input" value={runName} onChange={e => setRunName(e.target.value)} placeholder="web" />
              </div>
              <div className="row two">
                <div>
                  <label>{t("cpus")}</label>
                  <input className="input" type="number" min={1} value={runCpus} onChange={e => setRunCpus(e.target.value === "" ? "" : Number(e.target.value))} />
                </div>
                <div>
                  <label>{t("memoryMb")}</label>
                  <input className="input" type="number" min={64} value={runMem} onChange={e => setRunMem(e.target.value === "" ? "" : Number(e.target.value))} />
                </div>
              </div>
              <div className="row inline">
                <input type="checkbox" checked={runPull} onChange={e => setRunPull(e.target.checked)} />
                <span>{t("pullBeforeRun")}</span>
              </div>
              <div className="row">
                <button className="btn primary" disabled={runLoading || !runImage.trim()} onClick={doRun}>
                  {runLoading ? t("creating") : t("create")}
                </button>
              </div>
              {runResult && (
                <div className="result">
                  <div className="result-title">{t("loginCommand")}</div>
                  <div className="result-code">
                    <code>{runResult.login_cmd}</code>
                    <button className="icon-btn" onClick={() => copyText(runResult.login_cmd)} title={t("copy")}>{I.copy}</button>
                  </div>
                </div>
              )}
            </div>

            <div className="panel-title" style={{ marginTop: 14 }}>{t("tags")}</div>
            {imgTagsLoading ? (
              <div className="hint">{t("loading")}</div>
            ) : imgTags.length === 0 ? (
              <div className="hint">{imgTagsRef ? t("noTags") : t("tagsHint")}</div>
            ) : (
              <div className="tags">
                {imgTags.map(tag => (
                  <div className="tag" key={tag} onClick={() => { setRunImage(`${imgTagsRef}:${tag}`); setRunResult(null) }}>
                    {tag}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        <div className="grid2" style={{ marginTop: 14 }}>
          <div className="panel">
            <div className="panel-title">{t("importImage")}</div>
            <div className="form">
              <div className="row">
                <label>{t("imageArchivePath")}</label>
                <input className="input" value={loadPath} onChange={e => setLoadPath(e.target.value)} placeholder="/path/to/image.tar" />
              </div>
              <div className="row">
                <button className="btn" disabled={loadLoading || !loadPath.trim()} onClick={doLoad}>
                  {loadLoading ? t("working") : t("load")}
                </button>
              </div>
              <div className="hint">{t("importHint")}</div>
            </div>
          </div>
          <div className="panel">
            <div className="panel-title">{t("pushImage")}</div>
            <div className="form">
              <div className="row">
                <label>{t("imageRef")}</label>
                <input className="input" value={pushRef} onChange={e => setPushRef(e.target.value)} placeholder="ghcr.io/org/image:tag" />
              </div>
              <div className="row">
                <button className="btn" disabled={pushLoading || !pushRef.trim()} onClick={doPush}>
                  {pushLoading ? t("working") : t("push")}
                </button>
              </div>
              <div className="hint">{t("pushHint")}</div>
            </div>
          </div>
        </div>
      </div>
    )
  }

  const renderVms = () => {
    const doCreate = async () => {
      if (!vmName.trim()) return
      setVmActing("create")
      setVmError("")
      try {
        await invoke<string>("vm_create", { name: vmName.trim(), cpus: vmCpus, memory_mb: vmMem, disk_gb: vmDisk, rosetta: vmRosetta })
        setVmName("")
        await fetchVms()
        setToast(t("done"))
      } catch (e) {
        setVmError(String(e))
      } finally {
        setVmActing("")
      }
    }

    const action = async (cmd: string, id: string) => {
      setVmActing(id)
      setVmError("")
      try {
        await invoke(cmd, { id })
        await fetchVms()
      } catch (e) {
        setVmError(String(e))
      } finally {
        setVmActing("")
      }
    }

    const doLoginCmd = async (vm: VmInfoDto) => {
      setVmError("")
      try {
        const cmd = await invoke<string>("vm_login_cmd", {
          name: vm.name || vm.id,
          user: vmLoginUser,
          host: vmLoginHost,
          port: vmLoginPort === "" ? null : vmLoginPort,
        })
        openTextModal(t("loginCommand"), cmd, cmd)
      } catch (e) {
        setVmError(String(e))
      }
    }

    const doMountAdd = async () => {
      if (!mountVmId || !mountTag.trim() || !mountHostPath.trim()) return
      setVmError("")
      try {
        await invoke("vm_mount_add", {
          vm: mountVmId,
          tag: mountTag.trim(),
          host_path: mountHostPath.trim(),
          guest_path: mountGuestPath.trim() || "/mnt/host",
          readonly: mountReadonly,
        })
        setMountTag("")
        setMountHostPath("")
        await fetchVms()
        setToast(t("done"))
      } catch (e) {
        setVmError(String(e))
      }
    }

    const doMountRemove = async (vmId: string, tag: string) => {
      setVmError("")
      try {
        await invoke("vm_mount_remove", { vm: vmId, tag })
        await fetchVms()
        setToast(t("done"))
      } catch (e) {
        setVmError(String(e))
      }
    }

    return (
      <div className="page">
        <div className="toolbar">
          <button className="btn" onClick={fetchVms} disabled={vmLoading}>
            <span className="icon">{I.refresh}</span>{vmLoading ? t("loading") : t("refresh")}
          </button>
          <div className="hint" style={{ marginLeft: 8 }}>{t("vmHint")}</div>
        </div>

        {vmError && <div className="error-inline">{vmError}</div>}

        <div className="grid2">
          <div className="panel">
            <div className="panel-title">{t("createVm")}</div>
            <div className="form">
              <div className="row">
                <label>{t("name")}</label>
                <input className="input" value={vmName} onChange={e => setVmName(e.target.value)} placeholder="myvm" />
              </div>
              <div className="row two">
                <div>
                  <label>{t("cpus")}</label>
                  <input className="input" type="number" min={1} value={vmCpus} onChange={e => setVmCpus(Number(e.target.value) || 2)} />
                </div>
                <div>
                  <label>{t("memoryMb")}</label>
                  <input className="input" type="number" min={256} value={vmMem} onChange={e => setVmMem(Number(e.target.value) || 2048)} />
                </div>
              </div>
              <div className="row">
                <label>{t("diskGb")}</label>
                <input className="input" type="number" min={10} value={vmDisk} onChange={e => setVmDisk(Number(e.target.value) || 20)} />
              </div>
              <div className="row inline">
                <input type="checkbox" checked={vmRosetta} onChange={e => setVmRosetta(e.target.checked)} />
                <span>{t("enableRosetta")}</span>
              </div>
              <div className="row">
                <button className="btn primary" disabled={vmActing === "create" || !vmName.trim()} onClick={doCreate}>
                  {vmActing === "create" ? t("creating") : t("create")}
                </button>
              </div>
            </div>
          </div>

          <div className="panel">
            <div className="panel-title">{t("loginCommand")}</div>
            <div className="form">
              <div className="row two">
                <div>
                  <label>{t("user")}</label>
                  <input className="input" value={vmLoginUser} onChange={e => setVmLoginUser(e.target.value)} />
                </div>
                <div>
                  <label>{t("host")}</label>
                  <input className="input" value={vmLoginHost} onChange={e => setVmLoginHost(e.target.value)} />
                </div>
              </div>
              <div className="row">
                <label>{t("port")}</label>
                <input className="input" type="number" min={1} value={vmLoginPort} onChange={e => setVmLoginPort(e.target.value === "" ? "" : Number(e.target.value))} />
              </div>
              <div className="hint">{t("vmLoginHint")}</div>
            </div>
          </div>
        </div>

        <div className="panel" style={{ marginTop: 14 }}>
          <div className="panel-title">{t("vms")}</div>
          {vms.length === 0 ? (
            <div className="hint">{t("noVms")}</div>
          ) : (
            vms.map(vm => (
              <div className="container-card" key={vm.id}>
                <div className={`card-icon ${vm.state === "running" ? "" : "stopped"}`}>{I.server}</div>
                <div className="card-body">
                  <div className="card-name">{vm.name} <span className="sub">({vm.id})</span></div>
                  <div className="card-meta">
                    {t("state")}: {vm.state} · {vm.cpus} {t("cpus")} · {vm.memory_mb} {t("memoryMb")} · {vm.rosetta_enabled ? t("rosettaOn") : t("rosettaOff")}
                  </div>
                  {vm.mounts?.length > 0 && (
                    <div className="mounts">
                      {vm.mounts.map(m => (
                        <div className="mount" key={`${vm.id}-${m.tag}`}>
                          <span className="mono">{m.tag}</span>
                          <span className="muted">{m.host_path} → {m.guest_path} ({m.read_only ? "ro" : "rw"})</span>
                          <button className="btn tiny" onClick={() => doMountRemove(vm.id, m.tag)}>{t("remove")}</button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
                <div className="card-status">
                  <span className={`dot ${vm.state === "running" ? "running" : "stopped"}`} />
                  <span>{vm.state}</span>
                </div>
                <div className="card-actions">
                  {vm.state === "running" ? (
                    <button className="action-btn" disabled={vmActing === vm.id} onClick={() => action("vm_stop", vm.id)} title={t("stop")}>{I.stop}</button>
                  ) : (
                    <button className="action-btn" disabled={vmActing === vm.id} onClick={() => action("vm_start", vm.id)} title={t("start")}>{I.play}</button>
                  )}
                  <button className="action-btn" disabled={vmActing === vm.id} onClick={() => doLoginCmd(vm)} title={t("loginCommand")}>{I.terminal}</button>
                  <button className="action-btn danger" disabled={vmActing === vm.id} onClick={() => action("vm_delete", vm.id)} title={t("delete")}>{I.trash}</button>
                </div>
              </div>
            ))
          )}
        </div>

        <div className="panel" style={{ marginTop: 14 }}>
          <div className="panel-title">{t("virtiofs")}</div>
          <div className="form">
            <div className="row">
              <label>{t("vm")}</label>
              <select className="select" value={mountVmId} onChange={e => setMountVmId(e.target.value)}>
                <option value="">{t("selectVm")}</option>
                {vms.map(vm => <option key={vm.id} value={vm.id}>{vm.name} ({vm.id})</option>)}
              </select>
            </div>
            <div className="row two">
              <div>
                <label>{t("tag")}</label>
                <input className="input" value={mountTag} onChange={e => setMountTag(e.target.value)} placeholder="code" />
              </div>
              <div>
                <label>{t("guestPath")}</label>
                <input className="input" value={mountGuestPath} onChange={e => setMountGuestPath(e.target.value)} placeholder="/mnt/code" />
              </div>
            </div>
            <div className="row">
              <label>{t("hostPath")}</label>
              <input className="input" value={mountHostPath} onChange={e => setMountHostPath(e.target.value)} placeholder="~/code" />
            </div>
            <div className="row inline">
              <input type="checkbox" checked={mountReadonly} onChange={e => setMountReadonly(e.target.checked)} />
              <span>{t("readOnly")}</span>
            </div>
            <div className="row">
              <button className="btn" onClick={doMountAdd} disabled={!mountVmId || !mountTag.trim() || !mountHostPath.trim()}>
                {t("addMount")}
              </button>
            </div>
            <div className="hint">{t("virtiofsHint")}</div>
          </div>
        </div>
      </div>
    )
  }

  const renderSettings = () => (
    <div className="settings">
      <div className="setting-row">
        <div>
          <div className="setting-label">{t("theme")}</div>
          <div className="setting-desc">{t("themeDesc")}</div>
        </div>
        <select value={theme} onChange={e => setTheme(e.target.value as Theme)}>
          <option value="dark">{t("dark")}</option>
          <option value="light">{t("light")}</option>
        </select>
      </div>
      <div className="setting-row">
        <div>
          <div className="setting-label">{t("language")}</div>
          <div className="setting-desc">{t("languageDesc")}</div>
        </div>
        <select value={lang} onChange={e => setLang(normalizeLang(e.target.value))}>
          {Object.entries(langNames).map(([code, name]) => (
            <option key={code} value={code}>{name}</option>
          ))}
        </select>
      </div>
    </div>
  )

  const renderPage = () => {
    switch (activePage) {
      case "dashboard": return renderDashboard()
      case "containers": return renderContainers()
      case "images": return renderImages()
      case "vms": return renderVms()
      case "settings": return renderSettings()
      default: return (
        <div className="empty-state">
          <div className="empty-icon">{I.plus}</div>
          <h3>{t("comingSoon")}</h3>
          <p>{pageNames[activePage]} {t("underDev")}</p>
        </div>
      )
    }
  }

  return (
        <div className={`app ${theme === "light" ? "light" : ""}`}>
      {(modalTitle || modalBody) && (
        <div className="modal-backdrop" onClick={closeModal}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <div className="modal-head">
              <div className="modal-title">{modalTitle}</div>
              <div className="modal-actions">
                <button className="icon-btn" onClick={() => copyText(modalKind === "package" ? packageTag : modalCopyText)} title={t("copy")}>{I.copy}</button>
                <button className="icon-btn" onClick={closeModal} title={t("close")}>×</button>
              </div>
            </div>
            {modalKind === "package" ? (
              <div className="modal-body">
                <div className="hint">{modalBody}</div>
                <div className="form" style={{ marginTop: 10 }}>
                  <div className="row">
                    <label>{t("newImageTag")}</label>
                    <input className="input" value={packageTag} onChange={e => setPackageTag(e.target.value)} placeholder="myimage:latest" />
                  </div>
                </div>
              </div>
            ) : (
              <pre className="modal-pre">{modalBody}</pre>
            )}
            {modalKind === "package" && (
              <div className="modal-footer">
                <button
                  className="btn primary"
                  disabled={packageLoading || !packageContainer || !packageTag.trim()}
                  onClick={async () => {
                    if (!packageContainer || !packageTag.trim()) return
                    setPackageLoading(true)
                    try {
                      const out = await invoke<string>("image_pack_container", { container: packageContainer, tag: packageTag.trim() })
                      closeModal()
                      openTextModal(t("imagePacked"), out || t("done"), out || t("done"))
                      setToast(t("done"))
                    } catch (e) {
                      setError(String(e))
                    } finally {
                      setPackageLoading(false)
                    }
                  }}
                >
                  {packageLoading ? t("working") : t("package")}
                </button>
              </div>
            )}
          </div>
        </div>
      )}
      {toast && <div className="toast">{toast}</div>}
      <div className="sidebar">
        <div className="sidebar-header">
          <img src="/logo.png" alt={t("appName")} />
          <span className="brand-name">{t("appName")}</span>
          <span className="brand-version">v0.1</span>
        </div>
        <div className="sidebar-nav">
          {navItems.map(item => (
            <div
              key={item.page}
              className={`nav-item ${activePage === item.page ? "active" : ""}`}
              onClick={() => setActivePage(item.page)}
            >
              <span className="nav-icon">{item.icon}</span>
              <span className="nav-label">{pageNames[item.page]}</span>
              {item.count != null && item.count > 0 && <span className="nav-count">{item.count}</span>}
              {item.soon && <span className="nav-badge">{t("soon")}</span>}
            </div>
          ))}
          <div style={{ flex: 1 }} />
          <div
            className={`nav-item ${activePage === "settings" ? "active" : ""}`}
            onClick={() => setActivePage("settings")}
          >
            <span className="nav-icon">{I.settings}</span>
            <span className="nav-label">{t("settings")}</span>
          </div>
        </div>
      </div>

      <div className="main">
        <div className="topbar">
          <div className="topbar-left">
            <h1>{pageNames[activePage]}</h1>
            {activePage === "containers" && running.length > 0 && (
              <span className="count-chip">{running.length} {t("runningCount")}</span>
            )}
          </div>
          <div className="topbar-right">
            <div className="status-pill">
              <span className={`dot ${connected ? "on" : "off"}`} />
              {connected ? t("connected") : t("disconnected")}
            </div>
          </div>
        </div>
        <div className="content">
          {renderPage()}
        </div>
      </div>
    </div>
  )
}

export default App
