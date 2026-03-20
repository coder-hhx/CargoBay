# CrateBay v2 Spec 一致性检查清单

> 生成日期: 2026-03-20 | 作者: doc-keeper
> 基于 9 个 spec 文档提取的关键检查点

---

## 1. 版本号一致性

| 文档 | 当前版本 | 路径 |
|------|---------|------|
| architecture.md | 1.0.0 | docs/specs/architecture.md |
| backend-spec.md | 1.1.0 | docs/specs/backend-spec.md |
| frontend-spec.md | 1.1.0 | docs/specs/frontend-spec.md |
| api-spec.md | 1.1.0 | docs/specs/api-spec.md |
| agent-spec.md | 1.2.0 | docs/specs/agent-spec.md |
| database-spec.md | 1.1.0 | docs/specs/database-spec.md |
| mcp-spec.md | 1.1.0 | docs/specs/mcp-spec.md |
| runtime-spec.md | 1.0.0 | docs/specs/runtime-spec.md |
| testing-spec.md | 1.0.0 | docs/specs/testing-spec.md |

**检查项:**
- [ ] progress.md 中版本号与实际文档头部一致
- [ ] AGENTS.md 文档索引表中版本号与实际一致

---

## 2. Workspace & Crate 结构

**Spec 定义 (architecture.md §3):**
- 4 个 crate: `cratebay-core`, `cratebay-gui/src-tauri`, `cratebay-cli`, `cratebay-mcp`

**检查项:**
- [ ] `Cargo.toml` workspace members 包含全部 4 个 crate
- [ ] 每个 crate 的 `Cargo.toml` 存在且可编译
- [ ] workspace 依赖版本与 architecture.md Appendix A 一致
  - tokio 1.x, serde 1.x, thiserror 2.x, bollard 0.18, rusqlite 0.32, reqwest 0.12

---

## 3. Tauri Commands (api-spec.md)

**Spec 定义 31 个命令:**

### Container Commands (9)
- [ ] `container_list` — 存在且签名匹配
- [ ] `container_create` — 存在且签名匹配
- [ ] `container_start` — 存在且签名匹配
- [ ] `container_stop` — 存在且签名匹配
- [ ] `container_delete` — 存在且签名匹配
- [ ] `container_exec` — 存在且签名匹配
- [ ] `container_exec_stream` — 存在且签名匹配
- [ ] `container_logs` — 存在且签名匹配
- [ ] `container_inspect` — 存在且签名匹配

### LLM Commands (9)
- [ ] `llm_proxy_stream` — 存在且签名匹配
- [ ] `llm_provider_list` — 存在且签名匹配
- [ ] `llm_provider_create` — 存在且签名匹配
- [ ] `llm_provider_update` — 存在且签名匹配
- [ ] `llm_provider_delete` — 存在且签名匹配
- [ ] `llm_provider_test` — 存在且签名匹配
- [ ] `llm_models_fetch` — 存在且签名匹配
- [ ] `llm_models_list` — 存在且签名匹配
- [ ] `llm_models_toggle` — 存在且签名匹配

### Storage Commands (6)
- [ ] `api_key_save` — 存在且签名匹配
- [ ] `api_key_delete` — 存在且签名匹配
- [ ] `conversation_list` — 存在且签名匹配
- [ ] `conversation_get_messages` — 存在且签名匹配
- [ ] `settings_get` — 存在且签名匹配
- [ ] `settings_update` — 存在且签名匹配

### MCP Commands (5)
- [ ] `mcp_server_list` — 存在且签名匹配
- [ ] `mcp_server_start` — 存在且签名匹配
- [ ] `mcp_server_stop` — 存在且签名匹配
- [ ] `mcp_client_call_tool` — 存在且签名匹配
- [ ] `mcp_client_list_tools` — 存在且签名匹配

### System Commands (3)
- [ ] `docker_status` — 存在且签名匹配
- [ ] `runtime_status` — 存在且签名匹配
- [ ] `system_info` — 存在且签名匹配

**注:** progress.md 提到额外命令 (mcp_server_add, mcp_server_remove, mcp_export_client_config, llm_proxy_cancel)，需确认 api-spec.md 是否需要更新。

---

## 4. 数据模型 (models.rs)

**Spec 定义的关键类型 (api-spec.md + database-spec.md):**
- [ ] `ContainerInfo` — 字段匹配 spec
- [ ] `ContainerCreateRequest` — 字段匹配 spec
- [ ] `ContainerListFilters` — 字段匹配 spec
- [ ] `ContainerStatus` enum — 变体匹配 spec
- [ ] `ContainerDetail` — 字段匹配 spec
- [ ] `ExecResult` — 字段匹配 spec
- [ ] `LogOptions` / `LogEntry` — 字段匹配 spec
- [ ] `ChatMessage` — 字段匹配 spec
- [ ] `LlmOptions` — 字段匹配 spec (含 reasoning_effort)
- [ ] `LlmProvider` — 字段匹配 spec (含 api_format)
- [ ] `ApiFormat` enum — 三个变体: Anthropic, OpenAiResponses, OpenAiCompletions
- [ ] `LlmModelInfo` — 字段匹配 spec (含 supports_reasoning)
- [ ] `McpServerStatus` — 字段匹配 spec
- [ ] `McpToolInfo` — 字段匹配 spec (含 server_name)
- [ ] `DockerStatus` — 字段匹配 spec
- [ ] `RuntimeStatusInfo` — 字段匹配 spec
- [ ] `SystemInfo` — 字段匹配 spec
- [ ] `AppError` enum — 变体匹配 spec (9 个变体)

---

## 5. SQLite Schema (database-spec.md)

**Spec 定义 10 张表:**
- [ ] `ai_providers` — 含 api_format 字段 + CHECK 约束
- [ ] `ai_models` — 复合主键 (id, provider_id) + CASCADE
- [ ] `api_keys` — 含 encrypted_key BLOB + nonce BLOB
- [ ] `conversations` — 含 archived 和 metadata
- [ ] `messages` — 含 tool_calls, tool_call_id, model, usage
- [ ] `container_templates` — 4 个默认模板 seed
- [ ] `mcp_servers` — 含 auto_start 字段
- [ ] `audit_log` — 含 ip_address, session_id 预留字段
- [ ] `settings` — 5 个默认值 seed
- [ ] `_migrations` — 迁移跟踪表

**检查项:**
- [ ] migration 001_initial_schema.sql 内容与 spec 完全一致
- [ ] 索引创建完整 (idx_ai_models_provider, idx_conversations_updated 等)
- [ ] PRAGMA 设置: WAL, foreign_keys=ON, busy_timeout=5000

---

## 6. 前端结构 (frontend-spec.md)

### 目录结构
- [ ] `src/main.tsx` 存在
- [ ] `src/App.tsx` 存在
- [ ] `src/app.css` 存在 (含 Tailwind v4 @theme)
- [ ] `src/lib/utils.ts` 存在 (cn() helper)
- [ ] `src/types/` — index.ts, container.ts, chat.ts, mcp.ts, agent.ts, settings.ts
- [ ] `src/stores/` — 6 个 store: appStore, chatStore, containerStore, mcpStore, settingsStore, workflowStore
- [ ] `src/hooks/` — useTauriEvent, useStreamingMessage, useContainerActions, useAgent
- [ ] `src/components/layout/` — AppLayout, Sidebar, TopBar, StatusBar
- [ ] `src/components/chat/` — ChatInput, MessageList, MessageBubble, AgentThinking, ToolCallCard, ConfirmDialog
- [ ] `src/components/container/` — ContainerList, ContainerCard, ContainerDetail, TerminalView
- [ ] `src/components/mcp/` — McpServerList, McpToolList, McpServerConfig
- [ ] `src/pages/` — ChatPage, ContainersPage, McpPage, SettingsPage
- [ ] `src/tools/` — index.ts, containerTools.ts, filesystemTools.ts, shellTools.ts, mcpTools.ts

### Zustand Stores 接口
- [ ] appStore 包含 spec 定义的所有字段
- [ ] chatStore 包含 spec 定义的所有字段
- [ ] containerStore 包含 spec 定义的所有字段
- [ ] mcpStore 包含 spec 定义的所有字段
- [ ] settingsStore 包含 spec 定义的所有字段
- [ ] workflowStore 包含 spec 定义的所有字段

### 配置文件
- [ ] `components.json` — shadcn/ui 配置, style: "new-york", rsc: false
- [ ] Tailwind v4 CSS-first 配置 (无 tailwind.config.js)
- [ ] i18n: `src/locales/en.ts`, `src/locales/zh-CN.ts`

---

## 7. Agent 工具 (agent-spec.md)

**Spec 定义 17 个工具:**

### Container Tools (8)
- [ ] `container_create` — medium risk
- [ ] `container_list` — low risk
- [ ] `container_inspect` — low risk
- [ ] `container_start` — low risk
- [ ] `container_stop` — medium risk
- [ ] `container_delete` — high risk
- [ ] `container_exec` — medium risk
- [ ] `container_logs` — low risk

### Filesystem Tools (3)
- [ ] `file_read` — low risk
- [ ] `file_write` — medium risk
- [ ] `file_list` — low risk

### Shell Tools (1)
- [ ] `shell_exec` — medium risk

### MCP Tools (2)
- [ ] `mcp_list_tools` — low risk
- [ ] `mcp_call_tool` — varies (dynamic)

### System Tools (3)
- [ ] `docker_status` — low risk
- [ ] `system_info` — low risk
- [ ] `runtime_status` — low risk

**检查项:**
- [ ] tools/index.ts 注册了全部 17 个工具
- [ ] 每个工具使用 TypeBox schema 定义参数
- [ ] 风险级别映射表与 spec 一致
- [ ] systemPrompt.ts 独立文件存在

---

## 8. MCP Server (mcp-spec.md)

**Spec 定义 11 个工具:**
- [ ] `cratebay_sandbox_templates`
- [ ] `cratebay_sandbox_list`
- [ ] `cratebay_sandbox_inspect`
- [ ] `cratebay_sandbox_create`
- [ ] `cratebay_sandbox_start`
- [ ] `cratebay_sandbox_stop`
- [ ] `cratebay_sandbox_delete`
- [ ] `cratebay_sandbox_exec`
- [ ] `cratebay_sandbox_cleanup_expired`
- [ ] `cratebay_sandbox_put_path`
- [ ] `cratebay_sandbox_get_path`

**检查项:**
- [ ] 4 个模板定义: node-dev, python-dev, rust-dev, ubuntu-base
- [ ] 路径验证: validate_path 函数 (CRATEBAY_MCP_WORKSPACE_ROOT)
- [ ] Docker label 元数据系统
- [ ] 审计日志: JSONL 格式写入

---

## 9. MCP Client (mcp-spec.md §3-4)

- [ ] stdio 和 SSE 双传输支持
- [ ] .mcp.json 解析 + ${VAR} 环境变量展开
- [ ] McpManager 生命周期管理
- [ ] 重试策略: 3 次 + 指数退避
- [ ] MCP Tool Bridge (mcpTools.ts)
- [ ] useMcpToolSync.ts 动态工具同步
- [ ] agent.setTools() 调用 (非 updateTools)

---

## 10. Runtime (runtime-spec.md)

- [ ] RuntimeManager trait: detect, provision, start, stop, health_check, docker_socket_path, resource_usage
- [ ] RuntimeState enum: None, Provisioning, Provisioned, Starting, Ready, Stopping, Stopped, Error
- [ ] RuntimeConfig struct: cpu_cores, memory_mb, disk_gb, auto_start, shared_dirs
- [ ] ProvisionProgress struct: stage, percent, bytes_downloaded, bytes_total, message
- [ ] HealthStatus struct: runtime_state, docker_responsive, docker_version, uptime_seconds, last_check

### 平台实现
- [ ] MacOSRuntime (VZ.framework) — macos.rs
- [ ] LinuxRuntime (KVM/QEMU) — linux.rs
- [ ] WindowsRuntime (WSL2) — windows.rs

### Docker 连接
- [ ] 外部 Docker 检测优先
- [ ] Socket 路径优先级: DOCKER_HOST > 平台特定 > 内置 runtime

---

## 11. 错误类型 (backend-spec.md)

**AppError 枚举 (9 个变体):**
- [ ] Docker(bollard::errors::Error)
- [ ] Database(rusqlite::Error)
- [ ] LlmProxy(String)
- [ ] Validation(String)
- [ ] NotFound { entity, id }
- [ ] Mcp(String)
- [ ] Runtime(String)
- [ ] Io(std::io::Error)
- [ ] PermissionDenied(String)

**额外:**
- [ ] Serialization(serde_json::Error) — 在 backend-spec 中定义

---

## 12. Streaming Events (api-spec.md §4)

- [ ] `llm:stream:{channel_id}` — LlmStreamEvent (Token/ToolCall/Done/Error)
- [ ] `exec:stream:{channel_id}` — ExecStreamEvent (Stdout/Stderr/Done/Error)
- [ ] `runtime:health` — HealthStatus
- [ ] `runtime:provision` — ProvisionProgress

---

## 13. 测试基础设施 (testing-spec.md)

- [ ] Vitest 配置: coverage thresholds (lines 70%, functions 70%, branches 60%)
- [ ] Playwright 配置: playwright.config.ts
- [ ] Criterion benchmarks: benches/ 目录
- [ ] Mock 文件: mockStreamFn.ts, mockTauri.ts
- [ ] Golden file 测试目录

---

## 14. CI/CD (testing-spec.md §5)

- [ ] `.github/workflows/ci.yml` — 多 OS 矩阵 (macOS, Linux, Windows)
- [ ] `.github/workflows/release.yml` — v* 标签触发
- [ ] `.github/workflows/pages.yml` — website 部署
- [ ] `scripts/bench-perf.sh` — 性能验证脚本
- [ ] `scripts/ci-local.sh` — 本地 CI 门

---

## 15. 文档交叉引用

- [ ] AGENTS.md "Documentation Index" 表中文档名与路径正确
- [ ] AGENTS.md "Spec Loading Protocol" 表中引用的文档存在
- [ ] progress.md 文档完成明细版本号正确
- [ ] README.md 反映 v2 功能

---

## 16. 已知偏差记录

> 来源: progress.md "已知偏差" 章节

1. AGENTS.md 写的 "Vite 7.x" 实际不存在，使用 Vite 6.4.1
   - [ ] 确认已修正为 "Vite 6.x"
2. tauri-specta/specta 使用 rc 版本
   - [ ] 确认是否影响构建
3. Step 6 pi-agent-core API 适配偏差 (mcp-spec v1.1.0)
   - [ ] execute 签名适配
   - [ ] setTools 替代 updateTools

---

## 检查状态摘要

| 类别 | 检查项数 | 状态 |
|------|---------|------|
| 版本号 | 2 | 待检查 |
| Crate 结构 | 3 | 待检查 |
| Tauri Commands | 31 | 待检查 |
| 数据模型 | 18 | 待检查 |
| SQLite Schema | 13 | 待检查 |
| 前端结构 | 25+ | 待检查 |
| Agent 工具 | 21 | 待检查 |
| MCP Server | 15 | 待检查 |
| MCP Client | 7 | 待检查 |
| Runtime | 10 | 待检查 |
| 错误类型 | 10 | 待检查 |
| Streaming Events | 4 | 待检查 |
| 测试基础设施 | 5 | 待检查 |
| CI/CD | 5 | 待检查 |
| 文档交叉引用 | 4 | 待检查 |
| 已知偏差 | 5 | 待检查 |
| **合计** | **~178** | **待检查** |
