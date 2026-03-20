# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0-alpha.1] — 2026-03-20

Complete rewrite of CrateBay as a desktop AI development control plane.

### Added

#### Project Foundation (Step 0-1)
- Rust workspace with 4 crates: `cratebay-core`, `cratebay-gui`, `cratebay-cli`, `cratebay-mcp`
- 16 technical specification documents covering architecture, API, frontend, backend, database, runtime, MCP, agent, and testing
- AGENTS.md as universal AI agent entry point (compatible with 20+ AI coding tools)
- Spec-Driven development workflow with mandatory spec-first approach
- MIT license

#### Core Infrastructure (Step 2-3)
- **SQLite Storage Layer**: 12 tables with WAL mode, migration system, and full CRUD operations
  - Provider, Model, Conversation, Message, Settings, API Key (encrypted), Audit Log
- **Docker Integration**: Container CRUD via bollard SDK
- **LLM Proxy**: Streaming proxy supporting 3 API formats (Anthropic, OpenAI Responses, OpenAI Completions)
  - Dual header authentication (Authorization + x-api-key)
  - SSE streaming with cancellation support (CancellationToken)
  - `/v1/models` endpoint passthrough
- **30 Tauri Commands**: Container(9) + LLM(9) + Storage(6) + MCP(8) + System(3)
- **85 integration tests** for storage, LLM, and audit layers

#### Frontend (Step 4)
- **React 19 + Vite 6.x** frontend with TypeScript strict mode
- **Chat Interface**: ChatPage with Streamdown markdown rendering, @mention autocomplete
- **Container Management**: ContainerList, ContainerDetail components with useContainerActions hook
- **MCP Management**: McpServerList, McpToolList, McpPage components
- **6 Zustand Stores**: app, chat, container, mcp, settings, workflow
- **17 Agent Tools**: 8 container + 3 filesystem + 1 shell + 2 mcp + 3 system tools
- **i18n System**: Internationalization support
- **shadcn/ui + Radix UI** component library with Tailwind CSS v4
- **pi-agent-core** integration for chat-first AI agent interface

#### Built-in Container Runtime (Step 5)
- **RuntimeManager trait** with unified API across platforms
- **macOS**: VZ.framework (Virtualization.framework) VM runtime
- **Linux**: KVM/QEMU VM runtime with complete command-line builder and /proc resource monitoring
- **Windows**: WSL2 runtime with distro management and socket forwarding
- **External Docker detection** for pre-existing Docker installations
- **Health monitor** for runtime status tracking

#### MCP Server + Client (Step 6)
- **MCP Server** (`cratebay-mcp`): Standalone binary with JSON-RPC stdio transport
  - 11 sandbox tools (list, create, inspect, exec, start, stop, delete, cleanup, templates, put_path, get_path)
  - 4 sandbox templates (node-dev, python-dev, rust-dev, go-dev)
  - Path validation (CRATEBAY_MCP_WORKSPACE_ROOT) to prevent traversal attacks
  - JSONL audit logging
- **MCP Client** (`cratebay-core/mcp/`): 5-module architecture
  - stdio + SSE dual transport support
  - `.mcp.json` configuration parsing with `${VAR}` environment variable expansion
  - Server Discovery with config merging (project + user level)
  - McpManager for server lifecycle and tool inventory
  - Retry strategy for resilient connections
- **MCP Tauri Commands**: 8 commands (list, start, stop, add, remove, export, save, logs)
- **MCP Tool Bridge**: Frontend integration via mcpTools.ts + useMcpToolSync.ts hook

#### Testing & CI/CD (Step 7)
- **Rust Tests**: 269 passed, 0 failed, 5 ignored (Docker-dependent)
  - Storage, LLM proxy, audit, Docker integration, runtime, MCP server/client
- **Frontend Tests**: 197 Vitest tests passed, 2 skipped
  - Stores (chatStore, containerStore, mcpStore, settingsStore), components (ContainerList, McpServerList, ToolCallCard), hooks (useContainerActions, useMcpToolSync)
- **E2E Tests**: 68 Playwright tests with 5 Page Object Models
  - Navigation (9), chat-flow (12), settings (12), containers (16), mcp-servers (18), example (1)
  - Tauri webview-optimized Playwright config with CI/retry support
- **Agent Tests**: Mock LLM/Tauri + golden file tests + canary tests (conditional real LLM)
- **Security Tests**: 24 tests covering 4 attack categories
  - Path traversal (8 tests), JSON-RPC injection (9 tests), SQL injection (4 tests), API key leakage prevention (3 tests)
- **Performance Benchmarks**: Criterion benchmarks
  - startup_bench (5 benchmarks): AppState init, Docker connect, SQLite init, runtime init, full startup
  - storage_bench (4 benchmarks): message insert, message query, provider CRUD, conversation CRUD
- **CI/CD Pipelines**: 3 GitHub Actions workflows
  - `ci.yml`: 4-stage pipeline (check/fmt/clippy/lint → test → size-check/perf-bench → canary), 3-platform matrix (macOS/Linux/Windows), coverage reporting
  - `release.yml`: Triggered by `v*` tags, code signing, SHA256 checksums, multi-platform builds
  - `pages.yml`: Automatic website deployment from `website/` directory
- **Coverage**: @vitest/coverage-v8 with configurable thresholds

### Changed

- License changed from previous license to MIT
- Architecture rewritten from v1 monolith to 4-crate workspace
- Frontend migrated to React 19 + Tailwind CSS v4 (from v1 stack)
- Deferred Ollama integration and gRPC daemon to v2.1+

### Spec Documents Updated

| Document | Version | Changes |
|----------|---------|---------|
| api-spec.md | 1.2.0 | 9 missing commands added, `settings_set` → `settings_update` |
| backend-spec.md | 1.2.0 | MCP Client directory structure (5-module), new command signatures |
| architecture.md | 1.1.0 | MCP Client module description |
| agent-spec.md | 1.2.0 | Tool definitions, pi-agent-core execute signature adaptation |
| database-spec.md | 1.1.0 | Schema refinements |
| frontend-spec.md | 1.1.0 | Vite 6.x correction, store definitions |
| mcp-spec.md | 1.1.0 | execute signature adaptation, setTools API |
| testing-spec.md | 1.1.0 | Step 7 implementation results, updated test counts and CI/CD details |

### Known Issues

- tauri-specta/specta uses RC versions (2.0.0-rc.21/rc.22)
- Docker-dependent tests are ignored when Docker is unavailable (5 tests)
- AGENTS.md previously stated "Vite 7.x" which does not exist; corrected to Vite 6.x

---

[2.0.0-alpha.1]: https://github.com/nicepkg/CrateBay/releases/tag/v2.0.0-alpha.1
