# CrateBay

Open-source local AI sandbox. Run code safely on your machine — no cloud, no cost.

CrateBay gives any AI agent (Claude, Cursor, Windsurf, your own) a secure sandbox to execute code, install packages, and manage files — all running locally inside a lightweight VM. No Docker installation required.

## Why CrateBay?

AI agents need a safe place to run code. Cloud sandboxes (E2B, Modal) charge per minute and send your code off-machine. CrateBay runs everything locally:

- **Zero cost** — no cloud bills, no usage limits
- **Private** — code never leaves your machine
- **Fast** — local VM, no network round-trip
- **Works with any AI** — MCP protocol, works with Claude Desktop, Cursor, Windsurf, and any MCP-compatible client
- **No Docker required** — built-in VM runtime (macOS: Virtualization.framework, Linux: KVM, Windows: WSL2)

## How It Works

```
Your AI Agent                    CrateBay                         Local VM
(Claude, Cursor, etc.)           (MCP Server)                     (Docker inside VM)
       │                              │                                │
       │  "run this Python script"    │                                │
       ├─────────────────────────────►│                                │
       │                              │  create sandbox + exec code    │
       │                              ├───────────────────────────────►│
       │                              │                                │
       │                              │  stdout/stderr + exit code     │
       │       result                 │◄───────────────────────────────┤
       │◄─────────────────────────────┤                                │
```

## Quick Start

### 1. Install

```bash
# macOS (Apple Silicon & Intel)
brew install --cask cratebay

# Or download from Releases
```

### 2. Connect to Your AI

Add to your MCP client config (e.g. Claude Desktop `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "cratebay": {
      "command": "cratebay-mcp"
    }
  }
}
```

### 3. Use It

Tell your AI:

> "Create a Python sandbox and run: print('Hello from CrateBay')"

CrateBay handles the rest — VM startup, container creation, code execution, result delivery.

## Features

### MCP Server — Let Any AI Run Code

The `cratebay-mcp` binary exposes sandbox tools via the Model Context Protocol:

| Tool | What It Does |
|------|-------------|
| `sandbox_run_code` | Create sandbox + execute code + return result (one-shot) |
| `sandbox_create` | Create a persistent sandbox from template |
| `sandbox_exec` | Run a command in an existing sandbox |
| `sandbox_install` | Install packages (pip, npm, apt) |
| `sandbox_upload` / `sandbox_download` | Transfer files in/out of sandbox |
| `sandbox_list` | List running sandboxes |
| `sandbox_stop` / `sandbox_delete` | Lifecycle management |

### Desktop App — Visual Sandbox Management

The CrateBay desktop app provides:

- **Chat interface** — talk to an AI assistant that manages sandboxes through natural language
- **Sandbox dashboard** — see running sandboxes, resource usage, logs
- **Image management** — search, pull, and manage container images
- **MCP server management** — connect external MCP tool servers
- **Settings** — LLM provider config, runtime settings, registry mirrors

### CLI — Headless Operations

```bash
cratebay sandbox create --template python-dev
cratebay sandbox exec <id> -- python -c "print('hello')"
cratebay sandbox list
cratebay sandbox stop <id>
```

### Pre-built Sandbox Templates

| Template | Image | Use Case |
|----------|-------|----------|
| `python-dev` | Python 3.12 + pip | Data analysis, scripting, ML |
| `node-dev` | Node.js 20 + npm | Web development, scripting |
| `rust-dev` | Rust stable + cargo | Systems programming |
| `ubuntu-base` | Ubuntu 24.04 | General purpose |

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  CrateBay                                            │
│                                                      │
│  ┌──────────────┐  ┌──────────┐  ┌───────────────┐  │
│  │ cratebay-mcp │  │ GUI App  │  │ cratebay-cli  │  │
│  │ (MCP Server) │  │ (Tauri)  │  │ (CLI)         │  │
│  └──────┬───────┘  └────┬─────┘  └──────┬────────┘  │
│         └───────────────┼───────────────┘            │
│                         │                            │
│              ┌──────────▼──────────┐                 │
│              │   cratebay-core     │                 │
│              │   (Rust library)    │                 │
│              └──────────┬──────────┘                 │
│                         │                            │
│              ┌──────────▼──────────┐                 │
│              │  Built-in Runtime   │                 │
│              │  macOS: VZ.framework│                 │
│              │  Linux: KVM/QEMU   │                 │
│              │  Windows: WSL2     │                 │
│              │       ↓            │                 │
│              │  Docker in VM      │                 │
│              └─────────────────────┘                 │
└─────────────────────────────────────────────────────┘
```

**Tech stack**: Tauri v2 | React 19 | Rust | bollard | SQLite | pi-agent-core

## Compared To

| | CrateBay | E2B | Docker Desktop |
|---|---|---|---|
| Runs locally | Yes | No (cloud) | Yes |
| AI-native (MCP) | Yes | API only | No |
| Cost | Free | $0.01/min | Free / $5+/mo |
| Privacy | Code stays local | Code on cloud | Code stays local |
| No Docker required | Yes (built-in VM) | N/A | Requires Docker |
| Open source | MIT | Partial | No |

## Status

v2.0 alpha — Core sandbox infrastructure complete, working toward first public release.

See [docs/progress.md](docs/progress.md) for detailed development status and [docs/ROADMAP.md](docs/ROADMAP.md) for the release plan.

## Contributing

This project uses [AGENTS.md](AGENTS.md) for AI-assisted development. See [docs/](docs/) for technical specs and workflow guides.

## License

[MIT](LICENSE)
