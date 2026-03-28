# CrateBay

开源本地 AI 沙盒。在你的机器上安全地运行代码 — 无需云端，零成本。

CrateBay 为任何 AI Agent（Claude、Cursor、Windsurf 等）提供安全的代码执行沙盒 — 全部在本地轻量级虚拟机中运行。无需安装 Docker。

## 为什么选 CrateBay？

AI Agent 需要一个安全的地方来运行代码。云端沙盒（E2B、Modal）按分钟计费，且代码会离开你的机器。CrateBay 一切本地运行：

- **零成本** — 无云端账单，无用量限制
- **隐私安全** — 代码不离开你的电脑
- **低延迟** — 本地虚拟机，无网络往返
- **兼容任何 AI** — MCP 协议，支持 Claude Desktop、Cursor、Windsurf 等所有 MCP 客户端
- **无需 Docker** — 内置虚拟机运行时（macOS: Virtualization.framework, Linux: KVM, Windows: WSL2）

## 工作原理

```
你的 AI Agent                    CrateBay                         本地 VM
(Claude, Cursor 等)              (MCP Server)                     (VM 内的 Docker)
       │                              │                                │
       │  "运行这段 Python 代码"       │                                │
       ├─────────────────────────────►│                                │
       │                              │  创建沙盒 + 执行代码            │
       │                              ├───────────────────────────────►│
       │                              │                                │
       │                              │  stdout/stderr + exit code     │
       │       返回结果                │◄───────────────────────────────┤
       │◄─────────────────────────────┤                                │
```

## 快速开始

### 1. 安装

```bash
# macOS (Apple Silicon & Intel)
brew install --cask cratebay

# 或从 Releases 页面下载
```

### 2. 连接你的 AI

将以下配置添加到 Claude Desktop 配置文件（`claude_desktop_config.json`）：

```json
{
  "mcpServers": {
    "cratebay": {
      "command": "cratebay-mcp"
    }
  }
}
```

### 3. 开始使用

在 Claude 中输入：

> "创建一个 Python 沙盒并运行：print('Hello from CrateBay')"

CrateBay 会自动完成 — 虚拟机启动、容器创建、代码执行、结果返回。

## 功能

### MCP Server — 让任何 AI 运行代码

`cratebay-mcp` 通过 MCP 协议暴露沙盒工具：

| 工具 | 功能 |
|------|------|
| `sandbox_run_code` | 创建沙盒 + 执行代码 + 返回结果（一键完成） |
| `sandbox_install` | 安装依赖包（pip、npm、apt） |
| `sandbox_create` | 创建持久化沙盒 |
| `sandbox_exec` | 在已有沙盒中执行命令 |
| `sandbox_list` | 列出运行中的沙盒 |

### 桌面应用 — 可视化沙盒管理

CrateBay 桌面应用提供：Chat 界面、沙盒仪表盘、镜像管理、MCP 服务器管理、设置页面。

### CLI — 无头操作

```bash
cratebay sandbox create --template python-dev
cratebay sandbox exec <id> -- python -c "print('hello')"
cratebay sandbox list
```

## 技术栈

Tauri v2 | React 19 | Rust | bollard | SQLite | pi-agent-core

## 竞品对比

| | CrateBay | E2B | Docker Desktop |
|---|---|---|---|
| 本地运行 | 是 | 否（云端） | 是 |
| AI 原生（MCP） | 是 | 仅 API | 否 |
| 成本 | 免费 | $0.01/分钟 | 免费/$5+/月 |
| 隐私 | 代码在本地 | 代码在云端 | 代码在本地 |
| 无需 Docker | 是（内置 VM） | 不适用 | 需要 Docker |
| 开源 | MIT | 部分 | 否 |

## 状态

v2.0 alpha — 核心沙盒功能完成，正在准备首次公开发布。

详见 [docs/progress.md](docs/progress.md) 和 [docs/ROADMAP.md](docs/ROADMAP.md)。

## 贡献

本项目使用 [AGENTS.md](AGENTS.md) 进行 AI 辅助开发。技术规范和工作流指南见 [docs/](docs/)。

## 许可证

[MIT](LICENSE)
