# CrateBay

Open-source desktop AI development control plane.

- **Chat-First interface** for managing containers, AI models, and MCP tools
- **Built-in container runtime** — no external Docker installation required
- **Cross-platform**: macOS, Windows, Linux

## Status

v2.0 Rewrite in Progress — See [docs/progress.md](docs/progress.md) for current status.

## For AI-Assisted Development

This project uses [AGENTS.md](https://agents.md/) for AI-first development. Your IDE's AI assistant will automatically load project rules:

| IDE / Tool | Auto-loaded File |
|-----------|-----------------|
| Codex CLI | `AGENTS.md` (direct) |
| Gemini CLI | `GEMINI.md` → AGENTS.md |
| Claude Code | `CLAUDE.md` → AGENTS.md |
| CodeBuddy Code | `AGENTS.md` + `.codebuddy/` |
| Cursor | `.cursorrules` → AGENTS.md |
| Windsurf | `.windsurfrules` → AGENTS.md |
| GitHub Copilot | `.github/copilot-instructions.md` → AGENTS.md |
| VS Code / JetBrains / Zed | `AGENTS.md` (direct) |

## Documentation

| Document | Purpose |
|----------|---------|
| [AGENTS.md](AGENTS.md) | AI agent entry point — project overview, tech stack, spec loading protocol |
| [docs/README.md](docs/README.md) | Documentation index |
| [docs/progress.md](docs/progress.md) | Development progress & cross-machine recovery |
| [docs/specs/](docs/specs/) | Technical specifications (9 documents) |
| [docs/workflow/](docs/workflow/) | Development workflow & team collaboration |
| [docs/references/](docs/references/) | Technical decisions (ADR) & glossary |

## Tech Stack

Tauri v2 | React 19 | shadcn/ui | Tailwind CSS v4 | Zustand | Rust | bollard | SQLite | pi-agent-core | Streamdown

## License

[MIT](LICENSE)
