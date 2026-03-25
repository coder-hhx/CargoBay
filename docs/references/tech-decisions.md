# Technical Decision Records

> Version: 1.1.1 | Last Updated: 2026-03-25 | Author: product-manager

This document records Architecture Decision Records (ADRs) for the CrateBay v2 project. Each ADR captures a significant technical decision, the context that led to it, alternatives considered, and the consequences.

---

## ADR Index

| ID | Decision | Status |
|----|----------|--------|
| [ADR-001](#adr-001-use-pi-agent-core-for-agent-engine) | Use pi-agent-core for Agent Engine | Accepted |
| [ADR-002](#adr-002-use-shadcnui--radix-for-frontend-ui) | Use shadcn/ui + Radix for Frontend UI | Accepted |
| [ADR-003](#adr-003-use-sqlite-for-local-storage) | Use SQLite for Local Storage | Accepted |
| [ADR-004](#adr-004-built-in-container-runtime) | Built-in Container Runtime | Accepted |
| [ADR-005](#adr-005-hybrid-agent-architecture) | Hybrid Agent Architecture | Accepted |
| [ADR-006](#adr-006-llm-proxy-through-rust-backend) | LLM Proxy through Rust Backend | Accepted |
| [ADR-007](#adr-007-use-zustand-for-state-management) | Use Zustand for State Management | Accepted |
| [ADR-008](#adr-008-use-tauri-specta-for-type-contract) | Use tauri-specta for Type Contract | Accepted |
| [ADR-009](#adr-009-defer-grpc-daemon-to-v21) | Defer gRPC Daemon to v2.1+ | Accepted |
| [ADR-010](#adr-010-defer-ollama-integration-to-v21) | Defer Ollama Integration to v2.1+ | Accepted |
| [ADR-011](#adr-011-mcp-server--client-dual-support) | MCP Server + Client Dual Support | Accepted |
| [ADR-012](#adr-012-use-streamdown-for-streaming-markdown) | Use Streamdown for Streaming Markdown | Accepted |
| [ADR-013](#adr-013-keep-built-in-runtime-as-the-product-path) | Keep Built-in Runtime as the Product Path | Accepted |

---

## ADR-001: Use pi-agent-core for Agent Engine

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

CrateBay needs an agent engine to orchestrate LLM interactions, tool calls, and multi-turn conversations. The engine must support streaming responses, tool execution with confirmations, and integration with multiple LLM providers.

### Decision

Use `@mariozechner/pi-agent-core` as the TypeScript-based agent engine running in the frontend (React/Tauri webview).

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| LangChain.js | Overly complex for a desktop app; large bundle size; excessive abstraction layers |
| Vercel AI SDK | Tightly coupled with Next.js server patterns; limited tool execution control |
| Custom agent loop | High development cost; pi-agent-core already solves orchestration, tool calling, and streaming |
| Rust-based agent (rig/swarm) | Rust LLM ecosystem is less mature; TS has better ergonomics for rapid agent iteration |

### Consequences

- **Positive**: Mature agent framework with built-in tool calling, streaming, multi-turn support; active maintenance; TypeScript allows rapid iteration on agent behavior.
- **Positive**: Pairs with `@mariozechner/pi-ai` for unified multi-provider LLM access.
- **Negative**: Agent logic lives in the frontend (TS), not the backend (Rust), creating a hybrid architecture.
- **Negative**: Dependency on a third-party package maintained by a single author.

---

## ADR-002: Use shadcn/ui + Radix for Frontend UI

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

CrateBay's desktop UI needs a component library that is lightweight, fully customizable, accessible, and compatible with Tailwind CSS v4.

### Decision

Use `shadcn/ui` (copy-paste component system) built on Radix UI primitives, styled with Tailwind CSS v4.

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| Ant Design | Heavy bundle (~1MB); opinionated styling conflicts with custom design system; not Tailwind-native |
| Material UI (MUI) | Large runtime; CSS-in-JS approach conflicts with Tailwind; over-engineered for desktop app |
| Chakra UI | Better than MUI but still runtime-heavy; Tailwind integration is secondary |
| Headless UI | Less comprehensive than Radix; fewer component primitives available |

### Consequences

- **Positive**: Full source ownership (components live in the project, not node_modules); zero runtime overhead; perfect Tailwind CSS v4 integration.
- **Positive**: shadcn MCP Server enables AI-assisted component discovery and installation.
- **Positive**: Built-in accessibility via Radix primitives (keyboard navigation, screen readers, ARIA).
- **Negative**: More initial setup work compared to pre-built component libraries.
- **Negative**: Requires discipline to follow shadcn patterns consistently across the team.

---

## ADR-003: Use SQLite for Local Storage

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

CrateBay needs persistent local storage for conversations, API keys, settings, container templates, MCP server configs, and audit logs. The storage must be reliable, fast, and require no external database server.

### Decision

Use SQLite via `rusqlite` crate, with the database file at `~/.cratebay/cratebay.db`. API keys are encrypted using a system keyring-derived key.

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| JSON files | No ACID guarantees; poor query performance; concurrent access issues |
| IndexedDB (browser) | Limited to webview sandbox; no access from Rust backend; size limits |
| LevelDB / RocksDB | Over-engineered for this use case; no SQL query support |
| PostgreSQL / MySQL | Requires external server; contradicts zero-dependency philosophy |

### Consequences

- **Positive**: Zero-configuration, single-file database; ACID transactions; fast reads; battle-tested reliability.
- **Positive**: rusqlite integrates naturally with the Rust backend; no ORM needed.
- **Positive**: Migrations via versioned SQL scripts ensure schema evolution.
- **Negative**: Limited concurrent write performance (acceptable for single-user desktop app).
- **Negative**: API key encryption adds complexity to the storage layer.

---

## ADR-004: Built-in Container Runtime

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

CrateBay manages containers as part of its core functionality. Requiring users to install Docker Desktop is a significant barrier to adoption, especially on macOS where Docker Desktop requires a commercial license for enterprise use.

### Decision

Bundle a built-in container runtime that provisions a lightweight Linux VM with Docker inside:
- **macOS**: Virtualization.framework (VZVirtualMachine)
- **Linux**: KVM/QEMU
- **Windows**: WSL2

Fall back to detecting an existing Docker installation if the built-in runtime is unavailable.

**Later clarification:** ADR-013 keeps this decision as the product baseline and clarifies that Podman remains a fallback / escape hatch rather than a parallel product roadmap.

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| Require Docker Desktop | License issues; installation friction; not "open" |
| Podman | Less mature Docker API compatibility; confusing for users expecting Docker |
| containerd directly | Too low-level; no Docker API compatibility without additional shimming |
| Lima (macOS only) | macOS-only; adds another dependency; less control over the VM lifecycle |

### Consequences

- **Positive**: Zero-dependency experience; users get container management out of the box.
- **Positive**: Full control over runtime lifecycle, resource allocation, and socket exposure.
- **Negative**: Significant development effort for cross-platform VM management.
- **Negative**: First-run experience includes VM image download (potentially large).
- **Negative**: Platform-specific code paths increase testing surface.

---

## ADR-005: Hybrid Agent Architecture

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

CrateBay's agent system needs to orchestrate LLM interactions (streaming, multi-turn, tool selection) while executing system-level operations (Docker API, file I/O, SQLite queries, MCP connections).

### Decision

Adopt a hybrid architecture:
- **TypeScript layer** (frontend): pi-agent-core handles agent orchestration, LLM streaming, tool selection, and conversation state.
- **Rust layer** (backend): Tauri commands execute actual system operations (Docker, storage, MCP). Agent tools in TS are thin wrappers around `invoke()`.

```
pi-agent-core (TS)  ──→  AgentTool.execute()  ──→  Tauri invoke  ──→  Rust backend
     |                         |                         |                    |
  Orchestration           TS wrapper              IPC bridge           System ops
  LLM streaming           Type-safe               Auto-gen types      Docker/SQLite
  Tool selection           Parameters              (tauri-specta)      MCP/Runtime
```

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| Pure Rust agent | Rust LLM libraries are immature; slow iteration on agent behavior; poor streaming ergonomics |
| Pure TS agent (no Rust tools) | Cannot access system APIs (Docker, filesystem) directly from webview |
| Separate agent process | Extra process management complexity; IPC overhead; harder to debug |

### Consequences

- **Positive**: Best of both worlds -- rapid agent iteration in TS, safe/fast system operations in Rust.
- **Positive**: tauri-specta auto-generates TypeScript types from Rust, keeping the boundary type-safe.
- **Negative**: Two-language boundary adds architectural complexity.
- **Negative**: Tool execution has IPC overhead (Tauri invoke round-trip), though negligible for desktop use.

---

## ADR-006: LLM Proxy through Rust Backend

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

CrateBay must connect to external LLM providers (OpenAI, Anthropic, etc.) while keeping API keys secure. The frontend (webview) should never have direct access to API keys.

### Decision

Route all LLM API calls through the Rust backend:
1. Frontend invokes `llm_proxy_stream` Tauri command with the prompt/messages.
2. Rust backend reads the encrypted API key from SQLite, constructs the LLM request.
3. Rust streams tokens from the LLM provider.
4. Tokens are emitted as Tauri Events to the frontend.
5. Streamdown renders the streaming markdown in real-time.

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| Direct API calls from frontend | API keys exposed in webview; no encryption; security risk |
| Separate proxy server | Extra process; unnecessary for single-user desktop app |
| Store keys in keychain only | Still need a proxy for streaming; keychain access from webview is limited |

### Consequences

- **Positive**: API keys never leave the Rust backend; encrypted at rest in SQLite; zero exposure to the frontend.
- **Positive**: Centralized streaming logic; consistent error handling across providers.
- **Positive**: Backend can implement rate limiting, caching, and request transformation.
- **Negative**: Adds latency (frontend → Rust → LLM → Rust → frontend), though negligible.
- **Negative**: Backend must handle all provider-specific API quirks.

---

## ADR-007: Use Zustand for State Management

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

CrateBay's React frontend needs state management for multiple domains: app state, chat/conversation, containers, MCP servers, settings, and workflow state.

### Decision

Use Zustand with 6 domain-specific stores:
- `appStore` -- navigation, layout, global UI state
- `chatStore` -- conversations, messages, agent state
- `containerStore` -- container list, status, templates
- `mcpStore` -- MCP server configs, connection status
- `settingsStore` -- user preferences, LLM provider configs
- `workflowStore` -- task tracking, progress state

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| Redux Toolkit | Boilerplate-heavy; action/reducer pattern is overkill for desktop app state |
| Jotai | Atomic model is harder to reason about for complex domain state; less structure |
| MobX | Observable model adds runtime overhead; less popular in modern React ecosystem |
| React Context | Performance issues with frequent updates; no built-in devtools; no middleware |

### Consequences

- **Positive**: ~2KB bundle; zero boilerplate; TypeScript-first; excellent devtools.
- **Positive**: Domain-specific stores prevent prop drilling and keep state isolated.
- **Positive**: Simple API -- `create()` + `useStore()` -- easy for AI agents to generate correct code.
- **Negative**: No built-in persistence (must implement SQLite sync manually).
- **Negative**: 6 stores require discipline to avoid cross-store dependencies.

---

## ADR-008: Use tauri-specta for Type Contract

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

CrateBay has a Rust backend (Tauri commands) and a TypeScript frontend. The IPC boundary must be type-safe to prevent runtime errors from mismatched types.

### Decision

Use `tauri-specta` to auto-generate TypeScript type definitions and invoke wrappers from Rust Tauri command signatures.

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| Manual type definitions | Error-prone; types drift from Rust definitions; maintenance burden |
| JSON Schema generation | Extra build step; no direct Tauri integration; requires custom tooling |
| protobuf/gRPC | Over-engineered for Tauri IPC; adds proto compilation step; gRPC is deferred to v2.1+ |
| ts-rs | Generates TS types but no invoke wrappers; tauri-specta provides both |

### Consequences

- **Positive**: Single source of truth -- Rust types generate TS types automatically; zero drift.
- **Positive**: Generated invoke wrappers provide IDE autocomplete and compile-time safety.
- **Positive**: Integrates natively with Tauri v2 command system.
- **Negative**: Adds a code generation step to the build process.
- **Negative**: Macro-heavy Rust code can slow down `cargo check` in large codebases.

---

## ADR-009: Defer gRPC Daemon to v2.1+

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

The original CrateBay v1 architecture included a standalone gRPC daemon (tonic + prost) for remote container management. This was designed for scenarios where the container runtime runs on a remote machine.

### Decision

Defer the gRPC daemon to v2.1+. For v2.0, all communication uses Tauri commands (IPC between the webview and Rust backend within the same desktop process).

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| Build gRPC daemon in v2.0 | Adds significant complexity; local desktop use case doesn't need network communication |
| REST API instead of gRPC | Same problem -- unnecessary for local IPC; Tauri commands are simpler |
| WebSocket daemon | Same complexity concerns; Tauri Events already provide streaming |

### Consequences

- **Positive**: Reduces v2.0 scope significantly; Tauri commands are simpler to implement and test.
- **Positive**: No proto compilation step; no separate daemon process to manage.
- **Positive**: Reserved proto/ directory and architecture allow clean v2.1+ addition.
- **Negative**: v2.0 cannot support remote container management.
- **Negative**: Migration to gRPC in v2.1+ may require refactoring the command layer.

---

## ADR-010: Defer Ollama Integration to v2.1+

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

Ollama enables running LLMs locally (llama, mistral, etc.). The original v1 included Ollama integration for local model management. However, the mainstream LLM usage pattern is cloud-based API access (OpenAI, Anthropic, etc.).

### Decision

Defer Ollama integration to v2.1+. v2.0 focuses on cloud LLM providers via the LLM Proxy architecture.

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| Include Ollama in v2.0 | Adds GPU detection, model download management, and local inference complexity; niche use case |
| Support only Ollama (no cloud) | Most users prefer cloud APIs; would limit adoption |
| Abstract both behind a unified API | Good long-term design, but implementing both in v2.0 doubles the LLM integration surface |

### Consequences

- **Positive**: Reduced v2.0 scope; LLM Proxy only needs to support cloud API providers.
- **Positive**: The LLM Proxy architecture is extensible -- Ollama can be added as another "provider" in v2.1+.
- **Negative**: Users who prefer local models cannot use CrateBay v2.0 for on-device inference.
- **Negative**: GPU detection and model management UI work is postponed.

---

## ADR-011: MCP Server + Client Dual Support

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

Model Context Protocol (MCP) is emerging as a standard for AI tool interoperability. CrateBay can both expose its container management as MCP tools (server) and consume external MCP tools (client).

### Decision

Implement both:
1. **MCP Server** (`cratebay-mcp` binary): Exposes CrateBay's container/sandbox operations as MCP tools via stdio transport. External AI assistants (Claude, Cursor, etc.) can use CrateBay as a tool provider.
2. **MCP Client** (in `cratebay-core`): Connects to external MCP servers configured in `.mcp.json`. External tools appear as AgentTools in pi-agent-core (MCP Bridge).

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| MCP Server only | Misses the opportunity to extend CrateBay's agent with external tools |
| MCP Client only | Cannot expose CrateBay to external AI assistants |
| Neither (custom protocol) | Ignores the emerging MCP ecosystem; reduces interoperability |
| REST/OpenAPI plugin system | Less standardized than MCP; would require custom plugin discovery |

### Consequences

- **Positive**: Full MCP ecosystem participation -- both as provider and consumer.
- **Positive**: MCP Bridge makes external tools seamlessly available to the built-in agent.
- **Positive**: The standalone MCP Server binary works independently of the GUI app.
- **Negative**: Two MCP codepaths (server + client) to implement and maintain.
- **Negative**: MCP protocol is still evolving; may require updates as the spec stabilizes.

---

## ADR-012: Use Streamdown for Streaming Markdown

- **Status**: Accepted
- **Date**: 2026-03-20

### Context

CrateBay's chat interface must render LLM responses in real-time as tokens arrive. The renderer must handle partial markdown (incomplete code blocks, half-rendered tables) gracefully without visual glitches.

### Decision

Use Streamdown (by Vercel) for streaming markdown rendering in the chat UI.

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| react-markdown | Not designed for streaming; renders complete markdown only; glitches with partial content |
| marked + custom streaming | Requires building a custom streaming layer; significant effort for edge cases |
| MDX runtime | Over-engineered; JSX compilation overhead; not designed for real-time streaming |
| Raw HTML rendering | Security risks (XSS); no markdown parsing; poor developer experience |

### Consequences

- **Positive**: Purpose-built for AI streaming use cases; handles partial markdown gracefully.
- **Positive**: Native shadcn/ui integration -- rendered elements use shadcn components and Tailwind classes.
- **Positive**: Maintained by Vercel; aligned with the React ecosystem.
- **Negative**: Relatively new library; fewer community resources and examples.
- **Negative**: Bundle size impact (to be validated against dependency budget).

---

## ADR-013: Keep Built-in Runtime as the Product Path

- **Status**: Accepted
- **Date**: 2026-03-25

### Context

CrateBay's container and image management stack has seen frequent development-time issues while the built-in runtime, engine orchestration, and cross-platform Docker host handling are still being stabilized. This created a recurring question: should CrateBay stop investing in the built-in runtime and switch to Podman as the primary runtime instead?

The repository already contains a multi-provider engine layer (`external Docker` / `built-in runtime` / `Podman`) and a Docker-compatible control plane built around `bollard`. Without an explicit decision record, AI agents may incorrectly treat Podman and the built-in runtime as two equal roadmap tracks and keep expanding both.

### Decision

CrateBay keeps the **built-in runtime** as the **primary product runtime** and the **only first-class roadmap path**.

Podman remains supported only as a **fallback / escape hatch**, specifically for:

1. temporary recovery when the built-in runtime is unavailable,
2. development or CI environments that need a quick Docker-compatible engine,
3. explicitly requested host or enterprise constraints.

The implementation rules are:

- Container and image management continue to target the **Docker-compatible API boundary** (`bollard`, Docker socket/host semantics).
- AI agents and contributors must **fix the built-in runtime path first** when runtime-related issues appear.
- Do **not** introduce Podman-specific product features, product flows, or architectural branches unless a human explicitly approves that work.
- Podman support is maintained as a **compatibility layer**, not as a second product strategy.

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| Make Podman the default runtime | Weakens the zero-dependency product story; shifts lifecycle complexity to Podman installation and machine management; does not remove major cross-platform complexity on macOS/Windows |
| Maintain built-in runtime and Podman as equal first-class tracks | Doubles testing and product-surface complexity; encourages AI agents to expand both paths; slows stabilization of the primary runtime |
| Remove Podman support immediately | Removes a useful fallback for development, CI, and recovery scenarios before the built-in runtime has fully matured |

### Consequences

- **Positive**: Keeps the product story clear -- install CrateBay and get containers out of the box.
- **Positive**: Gives AI agents an unambiguous rule for runtime-related work, reducing accidental roadmap drift.
- **Positive**: Focuses engineering effort on stabilizing one primary runtime path while preserving a pragmatic fallback.
- **Negative**: The project still carries some multi-provider complexity in `engine.rs`.
- **Negative**: Podman compatibility must still be maintained enough to remain a reliable fallback.
- **Negative**: Teams must resist adding convenient Podman-specific workarounds that bypass built-in runtime issues instead of fixing them.

---

## ADR Template

For future decisions, use the following template:

```markdown
## ADR-NNN: [Decision Title]

- **Status**: Proposed | Accepted | Deprecated | Superseded by ADR-NNN
- **Date**: YYYY-MM-DD

### Context

[Why this decision is needed. What problem are we solving?]

### Decision

[What was decided. Be specific about technologies, patterns, or approaches chosen.]

### Alternatives Considered

| Alternative | Reason for Rejection |
|-------------|---------------------|
| ... | ... |

### Consequences

- **Positive**: ...
- **Negative**: ...
```
