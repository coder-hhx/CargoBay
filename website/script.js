(() => {
  "use strict"

  const translations = {
    en: {
      lang: "en",
      title: "CrateBay · Coming Soon",
      description: "CrateBay — desktop GUI for local AI workflows.",
      keywords: "cratebay, local ai, mcp, desktop gui",
      brand: "CrateBay",
      comingSoon: "Coming Soon",
      heroTitle: "CrateBay",
      heroLead: "Local AI workflows, in one GUI.",
      heroSub: "Coming soon.",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "Local workflows",
      summary1Body: "Build local AI workflows around managed sandboxes.",
      summary2Label: "Local Models",
      summary2Title: "Model control",
      summary2Body: "Keep local model setup and management in one place.",
      summary3Label: "MCP + Tools",
      summary3Title: "MCP ready",
      summary3Body:
        "Connect tools and local AI surfaces through a single desktop entry point.",
      sectionKicker: "Why It Hits",
      sectionTitle: "Local AI is moving fast.",
      sectionBody:
        "CrateBay is focused on making that workflow easier to operate from one desktop surface.",
      card1Title: "Sandbox-first GUI",
      card1Body: "Keep local AI sandboxes visible and manageable.",
      card2Title: "Local model workflows",
      card2Body: "Bring model setup and day-to-day use into the same GUI.",
      card3Title: "MCP built in",
      card3Body: "Keep MCP servers close to the rest of the workflow.",
      card4Title: "Coming soon",
      card4Body: "Follow CrateBay for public updates.",
      statusKicker: "Status",
      statusTitle: "Coming soon",
      statusBody: "Follow CrateBay for updates.",
      footer: "CrateBay · <span data-year></span>",
    },
    zh: {
      lang: "zh-CN",
      title: "CrateBay · 即将推出",
      description: "CrateBay —— 面向本地 AI 工作流的一体化桌面 GUI。",
      keywords: "cratebay, local ai, mcp, desktop gui",
      brand: "CrateBay",
      comingSoon: "即将推出",
      heroTitle: "CrateBay",
      heroLead: "本地 AI 工作流，一个 GUI 管起来。",
      heroSub: "即将推出。",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "本地工作流",
      summary1Body: "围绕托管沙箱组织你的本地 AI 工作流。",
      summary2Label: "Local Models",
      summary2Title: "模型管理",
      summary2Body: "把本地模型的配置与日常使用放到同一个界面里。",
      summary3Label: "MCP + Tools",
      summary3Title: "MCP 就绪",
      summary3Body:
        "把工具连接和本地 AI 能力收敛到同一个桌面入口。",
      sectionKicker: "为什么它有吸引力",
      sectionTitle: "本地 AI 变化很快。",
      sectionBody:
        "CrateBay 正在把这套工作流收敛到一个更易操作的桌面界面里。",
      card1Title: "Sandbox-first GUI",
      card1Body: "让本地 AI 沙箱保持可见、可控、可管理。",
      card2Title: "本地模型工作流",
      card2Body: "把模型配置和日常使用放到同一个 GUI。",
      card3Title: "桌面内建 MCP",
      card3Body: "让 MCP Server 更贴近整套本地 AI 工作流。",
      card4Title: "即将推出",
      card4Body: "关注 CrateBay，等待公开更新。",
      statusKicker: "状态",
      statusTitle: "即将推出",
      statusBody: "关注 CrateBay，等待更新。",
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
