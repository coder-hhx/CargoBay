(() => {
  "use strict"

  const translations = {
    en: {
      lang: "en",
      title: "CrateBay · Local AI Sandbox",
      description: "CrateBay — open-source local AI sandbox. Run code safely on your machine. No cloud, no cost, no Docker required.",
      keywords: "cratebay, ai sandbox, code execution, mcp server, container runtime, desktop app, tauri, local ai",
      brand: "CrateBay",
      comingSoon: "v0.9 Alpha",
      heroTitle: "CrateBay",
      heroLead: "Local AI sandbox. Run code safely on your machine.",
      heroSub:
        "Give any AI agent a secure sandbox to execute code \u2014 locally, privately, for free. Works with Claude, Cursor, Windsurf, and any MCP-compatible client. No Docker required.",
      githubCta: "GitHub",
      summary1Label: "AI Sandbox",
      summary1Title: "One-shot code execution",
      summary1Body:
        "AI agents call sandbox_run_code via MCP \u2014 CrateBay handles container creation, code execution, and result delivery automatically.",
      summary2Label: "Zero Config",
      summary2Title: "Built-in runtime, no Docker needed",
      summary2Body:
        "Embedded VM runtime on every platform. Install CrateBay and run code immediately \u2014 macOS, Windows, and Linux.",
      summary3Label: "MCP Native",
      summary3Title: "Works with any AI client",
      summary3Body:
        "Connect Claude Desktop, Cursor, Windsurf, or any MCP client. Your AI gets 13 sandbox tools out of the box.",
      sectionKicker: "Why CrateBay",
      sectionTitle: "The local alternative to cloud sandboxes.",
      sectionBody:
        "E2B and Modal charge per minute and send your code to the cloud. CrateBay runs everything locally \u2014 zero cost, full privacy, no limits.",
      card1Title: "Code Execution",
      card1Body:
        "Python, JavaScript, Bash, Rust \u2014 run any code in isolated sandboxes. Install packages, transfer files, stream output.",
      card2Title: "Privacy First",
      card2Body:
        "Code never leaves your machine. Runs inside a lightweight VM with hardware isolation. API keys encrypted at rest.",
      card3Title: "Cross-Platform",
      card3Body:
        "macOS (Virtualization.framework), Linux (KVM), Windows (WSL2). Built with Tauri v2 for native performance.",
      card4Title: "Open Source",
      card4Body:
        "MIT licensed. Free forever. Contribute features, add MCP tools, or self-host for your team.",
      statusKicker: "Status",
      statusTitle: "v0.9 \u2014 MCP Sandbox Ready",
      statusBody:
        "Core sandbox infrastructure complete. MCP Server with 13 tools, built-in runtime, offline image bundling. Working toward v1.0 with full GUI polish.",
      footer: "CrateBay \u00b7 <span data-year></span>",
    },
    zh: {
      lang: "zh-CN",
      title: "CrateBay \u00b7 \u672c\u5730 AI \u6c99\u7bb1",
      description: "CrateBay \u2014\u2014 \u5f00\u6e90\u672c\u5730 AI \u6c99\u7bb1\u3002\u5728\u4f60\u7684\u673a\u5668\u4e0a\u5b89\u5168\u8fd0\u884c\u4ee3\u7801\u3002\u96f6\u6210\u672c\uff0c\u96f6\u4e91\u7aef\uff0c\u65e0\u9700 Docker\u3002",
      keywords: "cratebay, ai \u6c99\u7bb1, \u4ee3\u7801\u6267\u884c, mcp server, \u5bb9\u5668\u8fd0\u884c\u65f6, \u684c\u9762\u5e94\u7528, tauri",
      brand: "CrateBay",
      comingSoon: "v0.9 Alpha",
      heroTitle: "CrateBay",
      heroLead: "\u672c\u5730 AI \u6c99\u7bb1\u3002\u5728\u4f60\u7684\u673a\u5668\u4e0a\u5b89\u5168\u8fd0\u884c\u4ee3\u7801\u3002",
      heroSub:
        "\u4e3a\u4efb\u4f55 AI Agent \u63d0\u4f9b\u5b89\u5168\u7684\u4ee3\u7801\u6267\u884c\u6c99\u7bb1 \u2014 \u672c\u5730\u8fd0\u884c\u3001\u9690\u79c1\u5b89\u5168\u3001\u5b8c\u5168\u514d\u8d39\u3002\u652f\u6301 Claude\u3001Cursor\u3001Windsurf \u53ca\u6240\u6709 MCP \u5ba2\u6237\u7aef\u3002\u65e0\u9700 Docker\u3002",
      githubCta: "GitHub",
      summary1Label: "AI \u6c99\u7bb1",
      summary1Title: "\u4e00\u952e\u4ee3\u7801\u6267\u884c",
      summary1Body:
        "AI Agent \u901a\u8fc7 MCP \u8c03\u7528 sandbox_run_code \u2014 CrateBay \u81ea\u52a8\u5b8c\u6210\u5bb9\u5668\u521b\u5efa\u3001\u4ee3\u7801\u6267\u884c\u548c\u7ed3\u679c\u8fd4\u56de\u3002",
      summary2Label: "\u96f6\u914d\u7f6e",
      summary2Title: "\u5185\u7f6e\u8fd0\u884c\u65f6\uff0c\u65e0\u9700 Docker",
      summary2Body:
        "\u6bcf\u4e2a\u5e73\u53f0\u5185\u5d4c VM \u8fd0\u884c\u65f6\u3002\u5b89\u88c5 CrateBay \u5373\u53ef\u7acb\u5373\u8fd0\u884c\u4ee3\u7801 \u2014 \u652f\u6301 macOS\u3001Windows\u3001Linux\u3002",
      summary3Label: "MCP \u539f\u751f",
      summary3Title: "\u517c\u5bb9\u4efb\u4f55 AI \u5ba2\u6237\u7aef",
      summary3Body:
        "\u8fde\u63a5 Claude Desktop\u3001Cursor\u3001Windsurf \u6216\u4efb\u4f55 MCP \u5ba2\u6237\u7aef\u3002AI \u5f00\u7bb1\u5373\u7528 13 \u4e2a\u6c99\u7bb1\u5de5\u5177\u3002",
      sectionKicker: "\u4e3a\u4ec0\u4e48\u9009 CrateBay",
      sectionTitle: "\u4e91\u7aef\u6c99\u7bb1\u7684\u672c\u5730\u66ff\u4ee3\u65b9\u6848\u3002",
      sectionBody:
        "E2B \u548c Modal \u6309\u5206\u949f\u8ba1\u8d39\uff0c\u4ee3\u7801\u4f1a\u79bb\u5f00\u4f60\u7684\u673a\u5668\u3002CrateBay \u4e00\u5207\u672c\u5730\u8fd0\u884c \u2014 \u96f6\u6210\u672c\u3001\u5b8c\u5168\u9690\u79c1\u3001\u65e0\u9650\u5236\u3002",
      card1Title: "\u4ee3\u7801\u6267\u884c",
      card1Body:
        "Python\u3001JavaScript\u3001Bash\u3001Rust \u2014 \u5728\u9694\u79bb\u6c99\u7bb1\u4e2d\u8fd0\u884c\u4efb\u4f55\u4ee3\u7801\u3002\u5b89\u88c5\u4f9d\u8d56\u3001\u4f20\u8f93\u6587\u4ef6\u3001\u6d41\u5f0f\u8f93\u51fa\u3002",
      card2Title: "\u9690\u79c1\u4f18\u5148",
      card2Body:
        "\u4ee3\u7801\u4e0d\u79bb\u5f00\u4f60\u7684\u673a\u5668\u3002\u5728\u8f7b\u91cf\u7ea7 VM \u4e2d\u8fd0\u884c\uff0c\u786c\u4ef6\u7ea7\u9694\u79bb\u3002API \u5bc6\u94a5\u52a0\u5bc6\u5b58\u50a8\u3002",
      card3Title: "\u8de8\u5e73\u53f0",
      card3Body:
        "macOS (Virtualization.framework)\u3001Linux (KVM)\u3001Windows (WSL2)\u3002\u57fa\u4e8e Tauri v2 \u6784\u5efa\uff0c\u539f\u751f\u6027\u80fd\u3002",
      card4Title: "\u5f00\u6e90",
      card4Body:
        "MIT \u534f\u8bae\u3002\u6c38\u4e45\u514d\u8d39\u3002\u8d21\u732e\u529f\u80fd\u3001\u6dfb\u52a0 MCP \u5de5\u5177\u6216\u4e3a\u56e2\u961f\u81ea\u90e8\u7f72\u3002",
      statusKicker: "\u72b6\u6001",
      statusTitle: "v0.9 \u2014 MCP \u6c99\u7bb1\u5c31\u7eea",
      statusBody:
        "\u6838\u5fc3\u6c99\u7bb1\u57fa\u7840\u8bbe\u65bd\u5b8c\u6210\u3002MCP Server \u542b 13 \u4e2a\u5de5\u5177\u3001\u5185\u7f6e\u8fd0\u884c\u65f6\u3001\u79bb\u7ebf\u955c\u50cf\u6253\u5305\u3002\u6b63\u5728\u8fc8\u5411 v1.0 \u5b8c\u6574 GUI \u6253\u78e8\u3002",
      footer: "CrateBay \u00b7 <span data-year></span>",
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
