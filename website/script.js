(() => {
  "use strict"

  const translations = {
    en: {
      lang: "en",
      title: "CrateBay · Coming Soon",
      description: "CrateBay — open-source desktop AI development control plane with chat-first interface and built-in container runtime.",
      keywords: "cratebay, ai development, chat-first, container runtime, mcp tools, desktop app, tauri",
      brand: "CrateBay",
      comingSoon: "Coming Soon",
      heroTitle: "CrateBay",
      heroLead: "Open-source desktop AI development control plane.",
      heroSub:
        "Chat-first interface for managing containers, AI models, and MCP tools. Built-in container runtime \u2014 no external Docker installation required.",
      githubCta: "GitHub",
      summary1Label: "Chat-First",
      summary1Title: "Conversational AI development",
      summary1Body:
        "Interact with an AI agent to manage containers, models, and tools through natural language chat.",
      summary2Label: "Container Runtime",
      summary2Title: "Built-in, zero-install Docker",
      summary2Body:
        "Embedded container runtime on every platform \u2014 no Docker Desktop required. macOS, Windows, and Linux supported.",
      summary3Label: "MCP Tools",
      summary3Title: "Extensible tool ecosystem",
      summary3Body:
        "Connect MCP servers, manage tool lifecycles, and extend agent capabilities without leaving the app.",
      sectionKicker: "Core Features",
      sectionTitle: "Everything you need for AI development, in one app.",
      sectionBody:
        "CrateBay combines a chat-first AI agent, built-in container runtime, and MCP tool management into a single desktop experience.",
      card1Title: "Agent Engine",
      card1Body:
        "Powered by pi-agent-core with streaming markdown rendering via Streamdown. Multi-provider LLM support built in.",
      card2Title: "Container Management",
      card2Body:
        "Full container lifecycle \u2014 create, start, stop, inspect, and manage images and volumes from the chat or GUI.",
      card3Title: "Cross-Platform",
      card3Body:
        "Built with Tauri v2 for macOS, Windows, and Linux. Native performance with a small binary footprint.",
      card4Title: "Open Source",
      card4Body:
        "MIT licensed. Follow the repo for progress, contribute features, or extend with your own MCP tools.",
      statusKicker: "Status",
      statusTitle: "v2 rewrite in progress",
      statusBody:
        "CrateBay v2 is being rebuilt from scratch with a chat-first architecture, built-in container runtime, and deep MCP integration.",
      footer: "CrateBay · <span data-year></span>",
    },
    zh: {
      lang: "zh-CN",
      title: "CrateBay · 即将推出",
      description: "CrateBay —— 开源桌面 AI 开发控制面，对话优先界面，内置容器运行时。",
      keywords: "cratebay, ai development, chat-first, container runtime, mcp tools, desktop app, tauri",
      brand: "CrateBay",
      comingSoon: "即将推出",
      heroTitle: "CrateBay",
      heroLead: "开源桌面 AI 开发控制面。",
      heroSub:
        "对话优先界面，统一管理容器、AI 模型与 MCP 工具。内置容器运行时，无需额外安装 Docker。",
      githubCta: "GitHub",
      summary1Label: "Chat-First",
      summary1Title: "对话式 AI 开发",
      summary1Body:
        "通过自然语言对话与 AI Agent 交互，管理容器、模型与工具。",
      summary2Label: "Container Runtime",
      summary2Title: "内置零安装 Docker",
      summary2Body:
        "每个平台内嵌容器运行时，无需安装 Docker Desktop。支持 macOS、Windows 和 Linux。",
      summary3Label: "MCP Tools",
      summary3Title: "可扩展的工具生态",
      summary3Body:
        "连接 MCP server，管理工具生命周期，无需离开应用即可扩展 Agent 能力。",
      sectionKicker: "核心功能",
      sectionTitle: "AI 开发所需的一切，尽在一个应用。",
      sectionBody:
        "CrateBay 将对话优先的 AI Agent、内置容器运行时与 MCP 工具管理整合为统一的桌面体验。",
      card1Title: "Agent 引擎",
      card1Body:
        "基于 pi-agent-core 驱动，通过 Streamdown 实现流式 Markdown 渲染，内置多供应商 LLM 支持。",
      card2Title: "容器管理",
      card2Body:
        "完整的容器生命周期管理 —— 通过对话或图形界面创建、启动、停止、检查容器，管理镜像与卷。",
      card3Title: "跨平台",
      card3Body:
        "基于 Tauri v2 构建，支持 macOS、Windows 和 Linux。原生性能，小巧的二进制体积。",
      card4Title: "开源",
      card4Body:
        "MIT 开源协议。关注仓库了解进展，贡献功能，或用自定义 MCP 工具扩展。",
      statusKicker: "状态",
      statusTitle: "v2 重写进行中",
      statusBody:
        "CrateBay v2 正在从零重建，采用对话优先架构、内置容器运行时与深度 MCP 集成。",
      footer: "CrateBay · <span data-year></span>",
    },
  }

  const storageKey = "cratebay-site-lang"
  const titleNode = document.querySelector("title")
  const descriptionMeta = document.querySelector('meta[name="description"]')
  const keywordsMeta = document.querySelector('meta[name="keywords"]')
  const year = String(new Date().getFullYear())

  function renderFooter() {
    document.querySelectorAll("[data-year]").forEach((node) => {
      node.textContent = year
    })
  }

  function setLanguage(lang) {
    const next = translations[lang] ? lang : "en"
    const dict = translations[next]
    document.documentElement.lang = dict.lang
    if (titleNode) titleNode.textContent = dict.title
    if (descriptionMeta) descriptionMeta.setAttribute("content", dict.description)
    if (keywordsMeta) keywordsMeta.setAttribute("content", dict.keywords)

    document.querySelectorAll("[data-i18n]").forEach((node) => {
      const key = node.getAttribute("data-i18n")
      if (!key || !(key in dict)) return
      if (key === "footer") {
        node.innerHTML = dict[key]
      } else {
        node.textContent = dict[key]
      }
    })

    document.querySelectorAll(".lang-btn").forEach((button) => {
      const active = button.getAttribute("data-lang") === next
      button.setAttribute("aria-pressed", active ? "true" : "false")
    })

    renderFooter()
    localStorage.setItem(storageKey, next)
  }

  const saved = localStorage.getItem(storageKey)
  const initial = saved || (navigator.language && navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en")
  setLanguage(initial)

  document.querySelectorAll(".lang-btn").forEach((button) => {
    button.addEventListener("click", () => {
      const lang = button.getAttribute("data-lang") || "en"
      setLanguage(lang)
    })
  })
})()
