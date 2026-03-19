(() => {
  "use strict"

  const translations = {
    en: {
      lang: "en",
      title: "CrateBay · Coming Soon",
      description: "CrateBay — open-source desktop control plane for local AI workflows.",
      keywords: "cratebay, local ai, ai sandboxes, mcp, desktop app",
      brand: "CrateBay",
      comingSoon: "Coming Soon",
      heroTitle: "CrateBay",
      heroLead: "Open-source desktop control plane for local AI workflows.",
      heroSub:
        "Public focus today: AI sandboxes, local models, MCP servers, and provider / CLI bridges. Containers stay in scope as the runtime underneath.",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "Managed local sandboxes",
      summary1Body:
        "Create, inspect, execute, and clean up local AI sandboxes from one desktop surface.",
      summary2Label: "Local Models",
      summary2Title: "Model runtime visibility",
      summary2Body:
        "Check Ollama status, storage, GPU telemetry, and model lifecycle in one place.",
      summary3Label: "MCP + Bridges",
      summary3Title: "Tooling handoff",
      summary3Body:
        "Manage MCP servers and provider / CLI bridge presets without leaving the app.",
      sectionKicker: "Public Scope",
      sectionTitle: "What the desktop app is proving now.",
      sectionBody:
        "CrateBay is validating the AI-first workflow first, with containers as the supporting runtime layer.",
      card1Title: "AI-first GUI",
      card1Body:
        "Dashboard, AI Hub, Containers, Images, Volumes, and Settings are the current default desktop surfaces.",
      card2Title: "Runtime-backed flows",
      card2Body:
        "Sandboxes, local models, MCP lifecycle, and container operations are all under active validation.",
      card3Title: "Future tracks stay hidden",
      card3Body:
        "VM and Kubernetes backends remain in the repo, but the default GUI hides them until dedicated runtime runners are ready.",
      card4Title: "Open development",
      card4Body:
        "Follow the repo for public progress while release sequencing stays private.",
      statusKicker: "Status",
      statusTitle: "Coming soon, with an AI-first scope",
      statusBody:
        "VM and Kubernetes stay hidden in the default GUI until dedicated runtime validation exists.",
      footer: "CrateBay · <span data-year></span>",
    },
    zh: {
      lang: "zh-CN",
      title: "CrateBay · 即将推出",
      description: "CrateBay —— 面向本地 AI 工作流的开源桌面控制面。",
      keywords: "cratebay, local ai, ai sandboxes, mcp, desktop app",
      brand: "CrateBay",
      comingSoon: "即将推出",
      heroTitle: "CrateBay",
      heroLead: "面向本地 AI 工作流的开源桌面控制面。",
      heroSub:
        "当前公开聚焦：AI sandboxes、本地模型、MCP server，以及 provider / CLI bridge；containers 继续作为底层运行时能力保留在范围内。",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "受管本地沙箱",
      summary1Body:
        "在一个桌面界面里完成本地 AI sandbox 的创建、查看、执行与清理。",
      summary2Label: "Local Models",
      summary2Title: "模型运行时可见性",
      summary2Body:
        "把 Ollama 状态、存储、GPU 遥测与模型生命周期放到同一个界面里。",
      summary3Label: "MCP + Bridges",
      summary3Title: "工具交接",
      summary3Body:
        "不离开应用即可管理 MCP server 与 provider / CLI bridge 预设。",
      sectionKicker: "公开范围",
      sectionTitle: "桌面应用当前在验证什么。",
      sectionBody:
        "CrateBay 先验证 AI-first 工作流，containers 作为底层运行时配套能力一起推进。",
      card1Title: "AI-first GUI",
      card1Body:
        "Dashboard、AI Hub、Containers、Images、Volumes 与 Settings 是当前默认对外的桌面界面。",
      card2Title: "运行时驱动的能力链",
      card2Body:
        "Sandboxes、本地模型、MCP 生命周期与容器操作都在持续验证中。",
      card3Title: "未来轨道继续隐藏",
      card3Body:
        "VM 与 Kubernetes 后端代码仍保留在仓库里，但默认 GUI 会继续隐藏它们，直到专用 runtime runner 就绪。",
      card4Title: "开放开发中",
      card4Body:
        "可以关注仓库了解公开进展，具体发布节奏与路线仍放在私有规划里。",
      statusKicker: "状态",
      statusTitle: "即将推出，但公开范围已经明确",
      statusBody:
        "在专用 runtime 验证补齐前，VM 与 Kubernetes 会继续从默认 GUI 中隐藏。",
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
