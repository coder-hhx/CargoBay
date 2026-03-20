# AGENTS.md — CrateBay v2 Project Guide

> **Version**: 2.0.0-rewrite | **Branch**: `rewrite/v2` | **Last Updated**: 2026-03-20
>
> This file is the **entry point** for all AI Agents working on this project.
> Detailed specs are in `docs/specs/` — load them on-demand based on your task (see Spec Loading Protocol below).
>
> Linked from: `.cursorrules`, `CLAUDE.md`, `GEMINI.md`, `.windsurfrules`, `.github/copilot-instructions.md`

---

## Project Identity

**CrateBay** is an open-source desktop AI development control plane.

- **Chat-First interface** for managing containers, AI models, and MCP tools
- **Built-in container runtime** — no external Docker installation required
- **Platforms**: macOS, Windows, Linux
- **License**: MIT

---

## Tech Stack Summary

| Layer | Technology | Version |
|-------|-----------|---------|
| Desktop Framework | Tauri | v2.x |
| Frontend | React + shadcn/ui + Radix | 19.x |
| CSS | Tailwind CSS | v4 |
| State Management | Zustand | latest |
| Streaming Markdown | Streamdown (Vercel) | latest |
| Agent Engine | @mariozechner/pi-agent-core | latest |
| LLM API | @mariozechner/pi-ai | latest |
| Type Contract | tauri-specta | latest |
| Backend | Rust | stable |
| Docker SDK | bollard | 0.18 |
| Storage | SQLite (rusqlite) | latest |
| MCP | modelcontextprotocol SDK | latest |
| Frontend Testing | Vitest + Playwright | latest |
| Backend Testing | cargo test + Criterion | latest |
| Build Tool | Vite | 6.x |

### Deferred to v2.1+

| Technology | Reason |
|-----------|--------|
| gRPC Daemon (tonic) | Tauri commands suffice for local desktop; remote management is v2.1+ |
| Ollama Integration | External LLM APIs are mainstream; local models are niche |

---

## Repository Structure

```
CrateBay/
├── AGENTS.md                        # THIS FILE — AI Agent entry point
├── README.md                        # Project homepage (for humans / GitHub)
├── .cursorrules                     # Symlink → AGENTS.md (Cursor IDE)
├── CLAUDE.md                        # Symlink → AGENTS.md (Claude Code)
├── GEMINI.md                        # Symlink → AGENTS.md (Gemini CLI)
├── .windsurfrules                   # Symlink → AGENTS.md (Windsurf IDE)
├── .github/
│   ├── copilot-instructions.md      # Symlink → ../AGENTS.md (GitHub Copilot)
│   └── workflows/                   # CI/CD pipelines
├── .mcp.json                        # MCP Server configuration
├── .codebuddy/
│   ├── project.yaml                 # Project metadata & stage config
│   ├── rules/                       # Project-level coding rules
│   └── tasks/                       # Task list snapshots
│
├── crates/
│   ├── cratebay-core/               # Core library: Docker, storage, LLM proxy, MCP client
│   ├── cratebay-gui/                # Tauri v2 desktop app (src-tauri/ + React frontend)
│   │   ├── src-tauri/               # Rust backend (Tauri commands)
│   │   └── src/                     # React frontend
│   ├── cratebay-cli/                # CLI binary (clap)
│   └── cratebay-mcp/               # MCP Server (standalone binary)
│
├── docs/
│   ├── README.md                    # Documentation index
│   ├── progress.md                  # Development progress tracker (cross-machine)
│   ├── specs/                       # Technical specifications (English, versioned)
│   │   ├── architecture.md          # System architecture overview
│   │   ├── frontend-spec.md         # Frontend specification
│   │   ├── backend-spec.md          # Backend specification (incl. CLI)
│   │   ├── agent-spec.md            # Agent integration specification
│   │   ├── database-spec.md         # Database design
│   │   ├── runtime-spec.md          # Built-in container runtime design
│   │   ├── api-spec.md              # Tauri Commands API specification
│   │   ├── mcp-spec.md              # MCP Server + Client specification
│   │   └── testing-spec.md          # Testing strategy & specification
│   ├── workflow/                    # Process docs (Chinese)
│   │   ├── dev-workflow.md          # Spec-Driven development workflow
│   │   ├── agent-team-workflow.md   # Agent Team collaboration rules
│   │   └── knowledge-base.md       # Knowledge base management
│   └── references/                  # Reference materials
│       ├── tech-decisions.md        # ADR-format technical decisions
│       └── glossary.md              # Terminology glossary
│
├── assets/                          # Brand resources
│   └── logo.png                     # CrateBay logo (1024×1024)
├── website/                         # Official website (GitHub Pages → cratebay.io)
├── scripts/                         # Dev scripts
├── proto/                           # gRPC proto definitions (v2.1+ reserved)
└── LICENSE                          # MIT License
```

---

## Architecture: Hybrid Agent Model

```
┌──────────────────────────────────────────────────────────────────┐
│                        CrateBay Desktop App                      │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────── Frontend (React 19) ───────────────────┐  │
│  │                                                            │  │
│  │  ChatPage ──→ pi-agent-core ──→ streamFn ──→ Tauri invoke │  │
│  │     │              │                            │          │  │
│  │     │         Tool calls              Tauri Event ←──┐    │  │
│  │     │              │                            │     │    │  │
│  │  Streamdown    AgentTool[]                      │     │    │  │
│  │  (rendering)   (TS wrappers)                    │     │    │  │
│  │                    │                            │     │    │  │
│  │  Zustand Stores    │                            │     │    │  │
│  │  (app/chat/container/mcp/settings/workflow)     │     │    │  │
│  └────────────────────┼────────────────────────────┼─────┼────┘  │
│                       │                            │     │       │
│  ─ ─ ─ ─ ─ ─ ─ ─ tauri-specta (auto-gen types) ─ ─ ─ ─ ─ ─ ─  │
│                       │                            │     │       │
│  ┌─────────────── Backend (Rust) ──────────────────┼─────┼────┐  │
│  │                                                 │     │    │  │
│  │  Tauri Commands ←───────────────────────────────┘     │    │  │
│  │     │                                                 │    │  │
│  │     ├── container.rs  → bollard (Docker SDK)          │    │  │
│  │     ├── llm.rs        → LLM Provider API ─────────────┘    │  │
│  │     ├── storage.rs    → rusqlite (SQLite)                  │  │
│  │     ├── mcp.rs        → MCP Client (stdio/SSE)             │  │
│  │     └── system.rs     → Runtime status, Docker status      │  │
│  │                                                            │  │
│  │  API Keys encrypted in SQLite (never sent to frontend)     │  │
│  └────────────────────────────────────────────────────────────┘  │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│  Built-in Container Runtime                                      │
│  macOS: VZ.framework VM │ Linux: KVM/QEMU VM │ Windows: WSL2    │
│  └── Docker engine inside VM, exposed via socket                 │
└──────────────────────────────────────────────────────────────────┘
```

**Key Flows**:

1. **Chat → Agent → Tool**: User message → pi-agent-core processes → calls AgentTool → tool wraps Tauri invoke → Rust executes → result returned
2. **LLM Proxy**: Frontend → Tauri invoke `llm_proxy_stream` → Rust streams to LLM → Tauri Event emits tokens → Streamdown renders
3. **MCP Bridge**: External MCP tools registered via `.mcp.json` → Rust MCP Client connects → tools appear as AgentTools in pi-agent-core

---

## Build & Test Commands

```bash
# Rust
cargo check --workspace                    # Quick compile check
cargo test --workspace                     # Run all tests
cargo build --release                      # Release build
cargo bench -p cratebay-core               # Criterion benchmarks

# Frontend
pnpm install                               # Install dependencies
pnpm dev                                   # Dev server with HMR
pnpm build                                 # Production build
pnpm test                                  # Vitest unit tests
pnpm test:e2e                              # Playwright E2E tests

# Full project
pnpm tauri dev                             # Run Tauri app in dev mode
pnpm tauri build                           # Build desktop app
./scripts/ci-local.sh                      # Full CI gate (Rust + frontend)
./scripts/bench-perf.sh                    # Performance validation
```

---

## Documentation Index

| Document | Language | Purpose |
|----------|----------|---------|
| [architecture.md](docs/specs/architecture.md) | EN | System architecture, crate dependencies, design principles |
| [frontend-spec.md](docs/specs/frontend-spec.md) | EN | React/shadcn/Zustand standards, page architecture |
| [backend-spec.md](docs/specs/backend-spec.md) | EN | Rust coding conventions, Tauri commands, CLI design |
| [agent-spec.md](docs/specs/agent-spec.md) | EN | pi-agent-core integration, tool definitions, LLM proxy |
| [database-spec.md](docs/specs/database-spec.md) | EN | SQLite schema, migrations, encryption |
| [runtime-spec.md](docs/specs/runtime-spec.md) | EN | Built-in container runtime (VZ/KVM/WSL2) |
| [api-spec.md](docs/specs/api-spec.md) | EN | Tauri Commands API catalog |
| [mcp-spec.md](docs/specs/mcp-spec.md) | EN | MCP Server + Client design |
| [testing-spec.md](docs/specs/testing-spec.md) | EN | Testing pyramid, CI/CD pipeline |
| [dev-workflow.md](docs/workflow/dev-workflow.md) | CN | Spec-Driven development workflow |
| [agent-team-workflow.md](docs/workflow/agent-team-workflow.md) | CN | Agent Team collaboration rules |
| [knowledge-base.md](docs/workflow/knowledge-base.md) | CN | Knowledge base auto-update rules |
| [tech-decisions.md](docs/references/tech-decisions.md) | EN | ADR-format technical decision records |
| [glossary.md](docs/references/glossary.md) | EN | Terminology definitions |
| [progress.md](docs/progress.md) | CN | Development progress (cross-machine recovery) |

---

## Conventions

### Commit Messages

**Conventional Commits** format required:

```
feat: add container list API
fix: resolve Docker socket timeout on macOS
docs: update api-spec with new commands
test: add unit tests for LLM proxy
refactor: extract Docker connection logic to core
```

- Max subject line: 72 characters
- Body: explain "why", not "what"

### Rust Conventions

- Error handling: `thiserror` + `Result<T, AppError>` (never `unwrap()` in production code)
- Mutex: Use `lock_or_recover()` (never `.lock().unwrap()`)
- Platform code: gated with `#[cfg(target_os = "...")]`
- Async: tokio runtime, no sync blocking in async context

### Frontend Conventions

- Components: shadcn/ui patterns, Radix primitives
- State: Zustand stores (6 stores), no prop drilling
- Styling: Tailwind CSS v4, CSS variables for theming
- Types: strict TypeScript, no `any`

### Branch Strategy

- `master` — stable releases
- `rewrite/v2` — current v2 rewrite branch
- Feature branches: `feat/description`, `fix/description`

---

## Spec-Driven Workflow (CRITICAL)

**Every feature must follow this flow:**

```
1. Update relevant spec document(s) FIRST
2. Get spec reviewed (human approval for breaking changes)
3. Implement code according to spec
4. Write tests
5. Update knowledge base:
   ├── AGENTS.md (if repo structure changed)
   ├── api-spec.md (if Tauri commands changed)
   ├── database-spec.md (if schema changed)
   ├── frontend-spec.md (if pages/stores changed)
   ├── agent-spec.md (if tools changed)
   ├── progress.md (always — mark completion)
   └── website/ (if user-facing change)
6. Increment spec version number
```

---

## Knowledge Base Auto-Update Protocol

When a feature is completed, the responsible agent MUST check:

| Code Change | Update Required |
|------------|-----------------|
| `crates/cratebay-gui/src-tauri/src/commands/*.rs` | api-spec.md |
| New/modified Zustand store | frontend-spec.md |
| New/modified Agent tool | agent-spec.md |
| SQLite schema change | database-spec.md |
| MCP tool change | mcp-spec.md |
| Cargo.toml dependency change | architecture.md |
| package.json dependency change | frontend-spec.md, architecture.md |
| Crate directory structure change | AGENTS.md |
| New platform support code | runtime-spec.md |
| Testing strategy change | testing-spec.md |

---

## Current Development Stage

**Phase 1: Documentation** (started 2026-03-20) — **Completed**

All 16 specification documents created. Moving to Phase 2: Project Skeleton Initialization.

See [docs/progress.md](docs/progress.md) for detailed progress and cross-machine recovery instructions.

---

## IDE / AI Tool Compatibility

This file is automatically loaded by 20+ AI coding tools:

| IDE / Tool | Auto-loaded File | Status |
|-----------|-----------------|--------|
| Codex CLI (OpenAI) | `AGENTS.md` | Direct |
| Gemini CLI (Google) | `GEMINI.md` → AGENTS.md | Symlink |
| Claude Code (Anthropic) | `CLAUDE.md` → AGENTS.md | Symlink |
| CodeBuddy Code | `AGENTS.md` + `.codebuddy/` | Direct |
| Cursor | `.cursorrules` → AGENTS.md | Symlink |
| Windsurf | `.windsurfrules` → AGENTS.md | Symlink |
| GitHub Copilot | `.github/copilot-instructions.md` → AGENTS.md | Symlink |
| VS Code / JetBrains / Zed / Aider / Warp | `AGENTS.md` | Direct |

---

## Spec Loading Protocol (CRITICAL)

**Do NOT try to read all docs at once.** Load specs on-demand based on your current task:

| Task Type | MUST Read First | Also Useful |
|-----------|----------------|-------------|
| **Starting any dev work** | `docs/workflow/agent-team-workflow.md` | `docs/workflow/dev-workflow.md` |
| **Frontend development** | `docs/specs/frontend-spec.md`, `docs/specs/api-spec.md` | `docs/specs/agent-spec.md` |
| **Backend development** | `docs/specs/backend-spec.md`, `docs/specs/api-spec.md`, `docs/specs/database-spec.md` | |
| **Agent integration** | `docs/specs/agent-spec.md`, `docs/specs/api-spec.md` | `docs/specs/frontend-spec.md` |
| **Runtime development** | `docs/specs/runtime-spec.md`, `docs/specs/backend-spec.md` | |
| **MCP development** | `docs/specs/mcp-spec.md`, `docs/specs/backend-spec.md` | |
| **Testing** | `docs/specs/testing-spec.md` + spec for the module being tested | |
| **Database changes** | `docs/specs/database-spec.md` | `docs/specs/backend-spec.md` |
| **Architecture decisions** | `docs/specs/architecture.md`, `docs/references/tech-decisions.md` | |
| **New to project** | This file → `docs/progress.md` → `docs/workflow/agent-team-workflow.md` | |
| **Workflow questions** | `docs/workflow/dev-workflow.md`, `docs/workflow/knowledge-base.md` | |

**How to use**: Before writing code, identify your task type above, then read the listed files using your IDE's file reading capability. The specs contain complete interface definitions, code patterns, SQL schemas, and API signatures you'll need.

---

## Agent Startup Protocol (MANDATORY)

When any AI Agent starts working on this project, follow these steps **in order**:

```
1. Read AGENTS.md                  → Project overview, tech stack, navigation
2. Read docs/progress.md           → Current phase, what's done, what's next
3. Read the "Quick Resume" section → Exact instructions for what to do next
4. DETERMINE CURRENT PHASE         → See "Phase Detection" below
5. EXECUTE PHASE PROTOCOL          → See "Phase Execution Rules" below
```

### Phase Detection (CRITICAL)

Check `docs/progress.md` "当前状态" → "阶段" field to determine the current phase:

| Phase | Condition | Action |
|-------|-----------|--------|
| 文档阶段 (Phase 1) | 16 docs not all completed | Create 3-agent doc team (architect, product-manager, frontend-architect) |
| 开发阶段 (Phase 2) | Docs done, code in progress | Create dev team per `agent-team-workflow.md` §1.1 |
| 测试阶段 (Phase 3) | Core features done | Create test team per `agent-team-workflow.md` §1.1 |

### Phase Execution Rules (CRITICAL — DO NOT SKIP)

**Rule 1: ALL development work MUST use Agent Teams.**
- Never write code as a single agent. Always create a team (TeamCreate) and spawn specialized agents.
- Team composition is defined in `docs/workflow/agent-team-workflow.md` §1.1.

**Rule 2: Team lead coordinates, does NOT implement.**
- The main agent acts as team-lead: creates tasks, spawns teammates, monitors progress.
- Teammates (backend-dev, frontend-dev, tester, etc.) do the actual implementation.

**Rule 3: Follow the Agent Team Workflow.**
- Read `docs/workflow/agent-team-workflow.md` BEFORE creating any team.
- Use TaskCreate with owner assignment and blockedBy dependencies.
- Teammates must follow the Agent Startup Protocol (read AGENTS.md → spec → TaskList → work).

**Rule 4: Quick Resume = executable instructions.**
- The "Quick Resume" section in `docs/progress.md` tells you EXACTLY what to do next.
- Follow it literally. Do not re-plan or re-explore if the instructions are clear.

**Rule 5: Architect Agent MUST verify spec compliance at EVERY step.**
- Every Agent Team MUST include an `architect` agent (spawn with subagent_type=Explore or general-purpose).
- The architect agent's SOLE responsibility is to **read the relevant spec documents and verify that every implementation detail matches the spec exactly**.
- The architect MUST run after each sub-task (not just at the end) to catch deviations early.
- Architect verification checklist (executed for EACH completed task):
  1. Read the relevant spec section(s) for the task
  2. Read the actual code files produced
  3. Compare EVERY interface, type, function signature, file path, component name, dependency version against the spec
  4. Report any deviation as a BLOCKING issue — the task is NOT complete until all deviations are fixed
  5. Verify no files exist that are NOT defined in the spec (no unauthorized additions)
  6. Verify no spec-defined items are missing from the implementation
- If the architect finds that spec itself has an error (e.g., references a non-existent API), the architect MUST report it to team-lead, who will ask the human for approval before modifying the spec. **Code must match spec; spec is NEVER modified to match code without human approval.**

**Rule 6: Strict Spec Compliance — ZERO tolerance for deviation.**
- Spec documents (`docs/specs/*.md`) are the **source of truth**. All code must match spec exactly.
- NO agent may "simplify", "defer", "skip", or "partially implement" any spec-defined item without explicit human approval.
- If a spec defines 14 tools, ALL 14 must be implemented. If a spec defines 4 hooks, ALL 4 must exist.
- If a dependency is listed in the spec (e.g., Streamdown), it MUST be installed and used.
- If a component is defined in the spec (e.g., TerminalView.tsx), it MUST be created.
- When the spec's interface definition conflicts with a third-party library's actual API, this is a spec issue — report to team-lead for human decision. Do NOT silently adapt.
- Protected files (pre-existing, NOT to be modified by dev agents): `.github/`, `.githooks/`, `assets/`, `scripts/`, `website/`, `proto/`, `LICENSE`, `.gitignore`, `.nvmrc`, symlinks (`.cursorrules`, `CLAUDE.md`, `GEMINI.md`, `.windsurfrules`, `.github/copilot-instructions.md`)

**Cross-machine recovery**: After `git pull` on a new machine, read this file + `docs/progress.md`. The progress file contains a "Quick Resume" section with executable step-by-step instructions.

---

## Performance Budget

Validated by `scripts/bench-perf.sh` and CI:

- Binary size: <20MB
- Startup time: <3s
- Idle RAM: <200MB

---

## Website

Official website: [cratebay.io](https://cratebay.io) (GitHub Pages, auto-deployed from `website/`)

- CI: `.github/workflows/pages.yml` — auto-deploys on `website/**` changes to master
- Brand: `assets/logo.png` (1024×1024, source of truth)
- doc-keeper is responsible for website updates during development

---

## Git Hooks

Hooks are in `.githooks/`. Run `scripts/setup-dev.sh` to activate.

- `pre-commit`: upstream sync + docs/i18n checks + `cargo check`
- `pre-push`: local CI gate via `scripts/ci-local.sh`
- `commit-msg`: validates Conventional Commits format

---

## CI/CD

- **ci.yml**: check + test + clippy + fmt + size-check + perf-bench (macOS + Linux + Windows)
- **release.yml**: triggered by `v*` tags, builds for all platforms, creates GitHub Release
- **pages.yml**: deploys `website/` to GitHub Pages
