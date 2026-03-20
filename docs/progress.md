# CrateBay 开发进度

## 当前状态
- **阶段**: 开发阶段 (Phase 2) — Step 7 完成，v2.0.0-alpha.1 准备就绪
- **日期**: 2026-03-20
- **团队模式**: 开发阶段 6 人团队（见 agent-team-workflow.md §1.1）

## 已完成 ✅
- [x] Step 0: 分支准备 (v1-archive tag, rewrite/v2 分支, 旧代码清理) — 2026-03-20
- [x] Step 1a: 目录结构 + 持久化配置 — 2026-03-20
- [x] Step 1b: 16份技术文档编写 — 2026-03-20
- [x] Step 1c: AGENTS.md 优化 (Spec Loading Protocol + Agent Team 强制规则) — 2026-03-20
- [x] LICENSE 改为 MIT — 2026-03-20
- [x] Step 2: 项目骨架初始化 — 2026-03-20
  - ✅ Rust workspace (4 crate) + 前端 (React 19 + Vite 6.4 + Tailwind v4 + shadcn/ui) + 测试框架
- [x] Step 3: Core 基础设施 — 2026-03-20
  - ✅ backend-dev: SQLite 存储层 (12 张表 + 迁移系统 + CRUD) + Docker 集成 + Container CRUD + Provider/Model 管理
  - ✅ ai-engineer: LLM Proxy (3 种 API 格式 + 双 header + SSE 流式 + /v1/models) ~950 行
  - ✅ backend-dev: 30 个 Tauri Commands 全部注册 + AppState 初始化
  - ✅ tester: 85 个集成测试全部通过 (存储层 30 + LLM 15 + 审计 12 + 内联 28 + Docker 5 ignored)
  - ✅ ai-spec-writer: 5 份 spec 文档更新 (api/agent/database/frontend/backend-spec → v1.1.0)
- [x] Step 4: 前端基础 — 2026-03-20
  - ✅ 第一轮实现 + Spec 偏差修复（step4-fix 团队）
  - ✅ frontend-dev: 4 类型文件 + 3 Chat 组件 + 2 Container 组件 + 3 MCP 组件 + McpPage 完整实现
  - ✅ frontend-dev: Streamdown 集成 + @mention 自动补全 + useContainerActions hook + 6 Store 补齐 + i18n 系统
  - ✅ ai-engineer: 17 个 Agent Tools (8 container + 3 filesystem + 1 shell + 2 mcp + 3 system) + systemPrompt 独立文件
  - ✅ backend-dev: 8 个新 Tauri Commands + 5 项签名/命名修复 + llm_proxy_cancel (CancellationToken)
  - ✅ architect: 8/8 大类零偏差验收通过
  - ✅ tester: 85 cargo tests + 50 vitest tests 全部通过，修复 4 个问题
  - ✅ Spec 文档同步更新: agent-spec.md v1.1.0→v1.2.0, api-spec.md 命名统一, AGENTS.md/frontend-spec.md Vite 6.x
- [x] Step 5: 内置 Runtime (VZ/KVM/WSL2) — 2026-03-20
  - ✅ runtime-dev: RuntimeManager trait + 核心类型 (RuntimeState/ProvisionProgress/HealthStatus/RuntimeConfig) + detect_external_docker + start_health_monitor
  - ✅ runtime-dev: MacOSRuntime (VZ.framework) — detect/provision/start/stop/health_check 完整框架
  - ✅ backend-dev: LinuxRuntime (KVM/QEMU) — build_qemu_args 完整命令行 + /proc 资源监控
  - ✅ backend-dev-2: WindowsRuntime (WSL2) — WSL2 distro 管理 + socket 转发
  - ✅ integrator: Tauri 集成 — AppState + runtime_status + docker.rs + health monitor
  - ✅ architect: 20/20 零偏差验收通过
  - ✅ tester: 121 cargo tests + 50 vitest tests 全部通过
- [x] Step 6: MCP (Server + Client) — 2026-03-20
  - ✅ mcp-dev: cratebay-mcp 独立二进制 — 11 个 sandbox 工具 + 4 模板 + 路径校验 + 审计日志 + JSON-RPC stdio
  - ✅ backend-dev: cratebay-core MCP Client — stdio/SSE 双传输 + .mcp.json 解析 + McpManager + 重试策略
  - ✅ integrator: Tauri MCP Commands — 5 个 stub 替换 + 3 个新命令 (add/remove/export) + AppState.mcp_manager
  - ✅ frontend-dev: MCP Tool Bridge — mcpTools.ts + useMcpToolSync.ts + mcpStore 修正
  - ✅ backend-dev: McpToolInfo 增加 server_name + serde camelCase
  - ✅ architect: 28/30 验收通过，2 个低严重度偏差已修复 spec (pi-agent-core API 适配)
  - ✅ tester: 253+ cargo tests + 67 vitest tests 全部通过
- [x] Step 7: 测试 + CI/CD + 打磨 — 2026-03-20
  - ✅ frontend-dev: 前端组件/Store/Hook 测试补充 (197 Vitest tests, 2 skipped + coverage 配置)
  - ✅ frontend-dev: E2E 测试 (68 个 Playwright 用例: navigation 9 + chat-flow 12 + settings 12 + containers 16 + mcp-servers 18 + example 1, 5 个 Page Object Models)
  - ✅ frontend-dev: Agent 测试 (Mock LLM/Tauri + golden file + canary tests)
  - ✅ backend-dev: 性能基准测试 (Criterion: startup_bench 5 个 + storage_bench 4 个)
  - ✅ tester: CI/CD Pipeline (ci.yml 4 阶段 + 三平台 matrix + coverage + canary)
  - ✅ tester: release.yml (v* tag 触发 + code signing + SHA256) + pages.yml (website 自动部署)
  - ✅ tester: 安全测试 (路径穿越 8 个 + JSON-RPC 注入 9 个 + SQL 注入 4 个 + API Key 泄露 3 个 = 24 个)
  - ✅ doc-keeper: Spec 文档一致性修复 (api-spec 1.2.0, backend-spec 1.2.0, architecture 1.1.0)
  - ✅ doc-keeper: CHANGELOG.md v2.0.0-alpha.1 + progress.md 最终更新
  - ✅ 最终测试结果: cargo test 269 passed / 5 ignored, vitest 197 passed / 2 skipped, Playwright 68 E2E

### Step 3 验证结果
```
cargo check --workspace     → ✅ 零错误 (3 个 dead_code 警告)
cargo test --workspace      → ✅ 85 passed, 0 failed, 5 ignored (Docker)
pnpm run test               → ✅ 4 passed (Vitest)
```

### Step 3 产出统计
- 30 个 Tauri Commands: Container(9) + LLM(9) + Storage(6) + MCP(5 stub) + System(3)
- LLM Proxy: Anthropic + OpenAI Responses + OpenAI Completions, 双 header, SSE 流式
- SQLite: 12 张表, WAL 模式, 迁移系统, Provider/Model/Conversation/Message CRUD
- 5 份 spec 文档更新至 v1.1.0 (AI 需求细化: 3 种 API 格式 + Provider 设置 + 模型勾选 + reasoning effort)

### Step 2 已知偏差
- AGENTS.md 写的 "Vite 7.x" 实际不存在，使用 Vite 6.4.1（稳定版）
- tauri-specta/specta 使用 rc 版本 (2.0.0-rc.21/rc.22)

## 进行中 🔄
- （无）

## 待开始 📋
- （无）

## 阻塞/问题 ⚠️
- 无

## 文档完成明细

16 份文档 + 5 份已更新至 v1.1.0：

| 文档 | 版本 | 路径 |
|------|------|------|
| architecture.md | **1.1.0** | docs/specs/architecture.md |
| backend-spec.md | **1.2.0** | docs/specs/backend-spec.md |
| runtime-spec.md | **1.0.0** | docs/specs/runtime-spec.md |
| api-spec.md | **1.2.0** | docs/specs/api-spec.md |
| database-spec.md | **1.1.0** | docs/specs/database-spec.md |
| frontend-spec.md | **1.1.0** | docs/specs/frontend-spec.md |
| agent-spec.md | **1.2.0** | docs/specs/agent-spec.md |
| mcp-spec.md | **1.1.0** | docs/specs/mcp-spec.md |
| testing-spec.md | **1.1.0** | docs/specs/testing-spec.md |
| dev-workflow.md | 1.0.0 | docs/workflow/dev-workflow.md |
| agent-team-workflow.md | 1.0.0 | docs/workflow/agent-team-workflow.md |
| knowledge-base.md | 1.0.0 | docs/workflow/knowledge-base.md |
| tech-decisions.md | 1.0.0 | docs/references/tech-decisions.md |
| glossary.md | 1.0.0 | docs/references/glossary.md |
| docs/README.md | 1.0.0 | docs/README.md |
| progress.md | — | docs/progress.md (this file) |

---

## 下次继续（Quick Resume）

> **给 AI 的可执行指令** — 换电脑后读取此段，按步骤执行，不要重新规划。

### 所有 7 个步骤已完成。v2.0.0-alpha.1 准备就绪。

Step 0 到 Step 7 全部完成。CrateBay v2 桌面应用开发阶段结束。

### Step 7 完成总结

**测试成果**:
- cargo test: 269 passed, 0 failed, 5 ignored (Docker-dependent)
- vitest: 197 passed, 2 skipped
- Playwright E2E: 68 用例 + 5 Page Object Models
- Criterion benchmarks: startup_bench (5 bench) + storage_bench (4 bench)
- 安全测试: 路径穿越 8 + JSON-RPC 注入 9 + SQL 注入 4 + API Key 泄露 3 = 24 个

**CI/CD 管线**:
- ci.yml: 4 阶段 (check/fmt/clippy/lint → test → size-check/perf-bench → canary), 三平台 matrix, coverage
- release.yml: v* tag 触发, code signing, SHA256 checksums
- pages.yml: website/ 自动部署到 GitHub Pages

**Spec 更新**:
- testing-spec.md: v1.0.0 → v1.1.0
- api-spec.md: v1.1.0 → v1.2.0 (9 个缺失命令补充)
- backend-spec.md: v1.1.0 → v1.2.0 (MCP Client 5 文件目录结构)
- architecture.md: v1.0.0 → v1.1.0 (MCP Client 模块描述)

### 下一步建议

```
1. git tag v2.0.0-alpha.1 && git push --tags    → 触发 release.yml
2. 验证 GitHub Release 产物 (macOS/Linux/Windows 二进制)
3. 验证 cratebay.io 网站自动部署
4. 规划 v2.0.0-beta.1 路线图 (用户反馈 + 性能调优)
```

### 历史偏差修复记录

> Step 3/4 偏差已在 Step 4 修复轮中全部解决（step4-fix 团队）。
> Step 5 架构师验收 20/20 零偏差。
> Step 6 架构师验收 28/30，2 个低严重度 spec 偏差已修复 (mcp-spec.md v1.1.0)。
> Step 7 所有测试通过，CI/CD 管线就绪，文档一致性验证完毕。
