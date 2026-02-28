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

type NavPage = "dashboard" | "containers" | "vms" | "images" | "settings"
type Theme = "dark" | "light"

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
}

function App() {
  const [containers, setContainers] = useState<ContainerInfo[]>([])
  const [error, setError] = useState("")
  const [loading, setLoading] = useState(true)
  const [connected, setConnected] = useState(false)
  const [activePage, setActivePage] = useState<NavPage>("dashboard")
  const [acting, setActing] = useState("")
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem("theme") as Theme) || "dark")
  const [lang, setLang] = useState(() => localStorage.getItem("lang") || "en")

  const t = (key: string) => messages[lang]?.[key] || messages.en[key] || key

  useEffect(() => { localStorage.setItem("theme", theme) }, [theme])
  useEffect(() => { localStorage.setItem("lang", lang) }, [lang])

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

  useEffect(() => {
    fetchContainers()
    const iv = setInterval(fetchContainers, 3000)
    return () => clearInterval(iv)
  }, [])

  const running = containers.filter(c => c.state === "running")
  const stopped = containers.filter(c => c.state !== "running")

  /* CargoBay 自己的导航结构 — 按功能维度划分 */
  const navItems: { page: NavPage; icon: React.ReactNode; count?: number; soon?: boolean }[] = [
    { page: "dashboard", icon: I.dashboard },
    { page: "containers", icon: I.box, count: containers.length },
    { page: "vms", icon: I.server, soon: true },
    { page: "images", icon: I.layers, soon: true },
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
        <div className="dash-card">
          <div className="dash-card-icon">{I.server}</div>
          <div className="dash-card-info">
            <div className="dash-card-value">0</div>
            <div className="dash-card-label">{t("vms")}</div>
          </div>
          <div className="dash-card-sub"><span className="dash-badge">{t("soon")}</span></div>
        </div>
        <div className="dash-card">
          <div className="dash-card-icon">{I.layers}</div>
          <div className="dash-card-info">
            <div className="dash-card-value">0</div>
            <div className="dash-card-label">{t("images")}</div>
          </div>
          <div className="dash-card-sub"><span className="dash-badge">{t("soon")}</span></div>
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
    if (loading) return <div className="loading"><div className="spinner" />{t("loading")}</div>
    if (error) return <div className="error-msg">{error}</div>
    if (containers.length === 0) return (
      <div className="empty-state">
        <div className="empty-icon">{I.box}</div>
        <h3>{t("noContainers")}</h3>
        <p>Run a container to get started</p>
        <code>docker run -it -p 80:80 docker/getting-started</code>
      </div>
    )

    return <>
      {running.length > 0 && <>
        <div className="section-title">{t("running")} ({running.length})</div>
        {running.map(c => (
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
            <div className="card-actions">
              <button className="action-btn" disabled={acting === c.id} onClick={() => containerAction("stop_container", c.id)} title={t("stop")}>{I.stop}</button>
              <button className="action-btn danger" disabled={acting === c.id} onClick={() => containerAction("remove_container", c.id)} title={t("delete")}>{I.trash}</button>
            </div>
          </div>
        ))}
      </>}

      {stopped.length > 0 && <>
        <div className="section-title">{t("stopped")} ({stopped.length})</div>
        {stopped.map(c => (
          <div className="container-card" key={c.id}>
            <div className="card-icon stopped">{I.box}</div>
            <div className="card-body">
              <div className="card-name">{c.name}</div>
              <div className="card-meta">{c.image} · {c.id}</div>
            </div>
            <div className="card-status">
              <span className="dot stopped" />
              <span>{c.status}</span>
            </div>
            <div className="card-actions">
              <button className="action-btn" disabled={acting === c.id} onClick={() => containerAction("start_container", c.id)} title={t("start")}>{I.play}</button>
              <button className="action-btn danger" disabled={acting === c.id} onClick={() => containerAction("remove_container", c.id)} title={t("delete")}>{I.trash}</button>
            </div>
          </div>
        ))}
      </>}
    </>
  }

  const renderSettings = () => (
    <div className="settings">
      <div className="setting-row">
        <div>
          <div className="setting-label">{t("theme")}</div>
          <div className="setting-desc">Switch between dark and light mode</div>
        </div>
        <select value={theme} onChange={e => setTheme(e.target.value as Theme)}>
          <option value="dark">{t("dark")}</option>
          <option value="light">{t("light")}</option>
        </select>
      </div>
      <div className="setting-row">
        <div>
          <div className="setting-label">{t("language")}</div>
          <div className="setting-desc">English, 中文, 日本語, 한국어</div>
        </div>
        <select value={lang} onChange={e => setLang(e.target.value)}>
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
