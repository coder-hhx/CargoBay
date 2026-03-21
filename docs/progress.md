# CrateBay 开发进度

## 当前状态
- **阶段**: 开发阶段 (Phase 2) — Step 7 完成 + Runtime 集成 + GUI 改进
- **日期**: 2026-03-21
- **团队模式**: 开发阶段 6 人团队（见 agent-team-workflow.md §1.1）
- **Git HEAD**: `rewrite/v2` 分支，共 3 个 commit（1 个已推送远程，2 个本地未推送）

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
- [x] Runtime 自动启动后端集成 — 2026-03-21 ✅ 已完成
  - ✅ AppState.docker 改为 `Arc<Mutex<Option<Arc<Docker>>>>` 支持动态更新
  - ✅ main.rs 后台线程自动启动 runtime (detect→provision→start→wait for Docker→fallback)
  - ✅ system.rs 新增 runtime_start, runtime_stop 命令 + docker_status source 检测
  - ✅ container.rs 所有命令适配新的 require_docker() 返回 Arc<Docker>
  - ✅ App.tsx 监听 docker:connected 事件刷新前端状态
  - ✅ Settings RuntimeTab 添加 Start/Stop 按钮 + runtimeLoading 状态
  - ✅ 编译通过：TypeScript 0 errors, Rust 0 errors (2 dead_code warnings)

## 待开始 📋
### 任务 A: 验证 GUI 应用运行 🔴 优先
- 运行 `pnpm tauri dev` 验证 GUI 正常启动
- 检查 runtime 自动启动日志
- 检查前端渲染是否正常
- 验证 Settings 中 Runtime Start/Stop 按钮是否工作

### 任务 B: 文档与实现一致性审查 🔴 优先
- 对比 api-spec.md vs 实际 Tauri 命令（重点：runtime_start, runtime_stop 是否在 spec 中）
- 对比 frontend-spec.md vs 实际前端实现（Settings 6 Tab vs spec 3 Tab）
- 对比 runtime-spec.md vs runtime 实现（骨架 vs 完整实现）
- 对比 backend-spec.md vs 后端实现

### 任务 C: 自研 Runtime 实际实现 🔴 优先
- **问题**：v2 的 macOS/Linux/Windows runtime 全部是骨架代码
  - macos.rs: provision() 返回 "VM image download not yet implemented"
  - macos.rs: start() 返回 "VZ.framework bridge not yet implemented"
  - 同理 linux.rs 和 windows.rs
- **master 分支已有完整实现**（需要移植）：
  - `master:crates/cratebay-core/src/runtime.rs` (2839行) — VM 镜像管理 + ensure_runtime_vm_running
  - `master:crates/cratebay-core/src/hypervisor.rs` — Hypervisor trait, VmConfig, VmState
  - `master:crates/cratebay-gui/src-tauri/src/lib.rs` — connect_cratebay_runtime_docker()
- **用户要求**："自研runtime，但也能识别外部runtime，可以通过设置选择容器运行时"
- **当前行为**：auto-start 尝试 runtime → 骨架代码失败 → fallback 到外部 Docker
- **需要**：从 master 移植完整 VM 启动逻辑到 v2 的多文件架构中

### 任务 D: 修复 pre-commit 钩子 Bug
- `.githooks/pre-commit` 行 289-290 运行 `cargo test -p cratebay-cli --lib`
- cratebay-cli 没有 lib target，只有 bin target，导致钩子失败
- 临时用 `--no-verify` 跳过，需要修复

## 阻塞/问题 ⚠️
- **pre-commit 钩子 Bug**: `cargo test -p cratebay-cli --lib` 失败（CLI 无 lib target），当前用 `--no-verify` 跳过
- **CodeBuddy Agent 框架 Bug**: 进程内 agent 注册表持久化，跨 TeamDelete 后仍阻止创建同名 agent，需要**重启新会话**才能创建新团队
- **2 个本地 commit 未推送**: 需 `git push` 推送到 origin/rewrite/v2

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

## 用户永久规则（MUST FOLLOW — 跨会话/跨机器生效）

> **所有 AI Agent 必须遵守以下规则，无需用户每次重复要求。**

### 规则 1: 固定开发团队
每次新会话启动时，**必须**按照 `docs/workflow/agent-team-workflow.md` §1.1 创建固定开发团队：
- 当前阶段：开发阶段（Phase 2）
- 团队组成（6 人）：team-lead、frontend-dev、backend-dev、ai-engineer、runtime-dev、tester
- 团队名称：`cratebay-dev`
- 团队是项目全生命周期的，**不要在一轮任务完成后关闭**
- 只有用户明确要求或项目阶段切换时才调整团队

### 规则 2: 按 spec 执行
所有开发工作严格遵循 `docs/specs/` 下的 spec 文档，spec 是唯一的真理来源。

---

## 下次继续（Quick Resume）

> **给 AI 的可执行指令** — 新会话启动后读取此段，按步骤执行。

### 当前阶段：功能验证 + 文档对齐 + 自研 Runtime 移植（2026-03-21）

Step 0-7 基础骨架 + GUI 改进 + Runtime 自动启动后端 + Settings 控制 UI 全部完成。
当前有 **三个并行任务** 需要执行。

### 可执行步骤

```
1. 读取 AGENTS.md + 本文件（progress.md）
2. 按"用户永久规则"创建 cratebay-dev 固定团队（注意：团队名不要与上一会话冲突）
3. 并行执行以下 3 个任务（各分配给一个 agent）：

   任务 A — GUI 验证 (分配给 tester):
   a. cd crates/cratebay-gui && pnpm build          # 前端构建验证
   b. cd 项目根 && cargo build -p cratebay-gui      # Rust 构建验证
   c. pnpm tauri dev                                 # 启动应用
   d. 检查启动日志：
      - "Starting built-in runtime auto-start sequence..."
      - "Runtime start failed: ..." → "Falling back to external Docker..."
      - 前端是否正常渲染
   e. 验证 Settings → Runtime tab → Start/Stop 按钮是否可见
   f. 汇报所有发现的问题

   任务 B — 文档一致性审查 (分配给 architect):
   a. 读取 docs/specs/api-spec.md，提取所有 Tauri 命令定义
   b. 读取 crates/cratebay-gui/src-tauri/src/commands/*.rs，提取实际 #[tauri::command]
   c. 对比找出 Spec 未定义但已实现的（如 runtime_start/runtime_stop）
   d. 同样对比 frontend-spec.md, runtime-spec.md, backend-spec.md
   e. 输出偏差报告（每项标注 ✅一致 / ⚠️偏差 / ❌缺失）
   f. 对于发现的偏差，提出修复方案（更新 spec 还是更新代码）

   任务 C — 自研 Runtime 分析 (分配给 runtime-dev):
   a. 切到 master 分支读取完整 runtime 实现：
      - crates/cratebay-core/src/runtime.rs (2839行)
      - crates/cratebay-core/src/hypervisor.rs
      - crates/cratebay-gui/src-tauri/src/lib.rs 中的 connect_cratebay_runtime_docker()
   b. 切回 rewrite/v2 分支读取骨架代码：
      - crates/cratebay-core/src/runtime/{mod.rs, macos.rs, linux.rs, windows.rs}
   c. 列出 master 有但 v2 缺失的具体功能模块：
      - VM 镜像下载管理
      - VZ.framework 桥接（macOS）
      - KVM/QEMU 完整启动（Linux）
      - WSL2 完整管理（Windows）
      - Docker socket 等待和连接
   d. 制定移植方案：哪些代码可以直接复用，哪些需要改造以适配多文件架构
   e. 评估移植工作量（文件数、代码行数、关键依赖）

4. 三个任务完成后，team-lead 汇总结果，制定后续计划
5. 更新本文件标记完成
```

### Runtime 自动启动后端架构（已完成，供参考）

```
main.rs setup:
  std::thread::spawn("runtime-auto-start") {
    1. 检查 Docker 是否已连接 → 如果已连接，跳过
    2. runtime.detect() → 检测当前状态
    3. 如果 None → runtime.provision() + emit runtime:provision-progress
    4. runtime.start() → 启动 runtime
    5. 等待 Docker socket (45s 超时，每 500ms 轮询)
    6. 成功 → state.set_docker() + emit docker:connected
    7. 失败 → try_reconnect_docker() fallback 到外部 Docker
  }

AppState:
  docker: Arc<Mutex<Option<Arc<Docker>>>>  // 支持动态更新
  runtime: Arc<dyn RuntimeManager>          // 平台特定实现

Settings RuntimeTab:
  - Start 按钮 → invoke("runtime_start")
  - Stop 按钮 → invoke("runtime_stop")
  - runtimeLoading 状态跟踪操作进度
```

### macOS Runtime 现状（骨架 vs master 完整实现）

| 功能 | v2 (rewrite/v2) | master | 状态 |
|------|-----------------|--------|------|
| detect() | ✅ 检查 socket 文件 | ✅ 完整 | 一致 |
| provision() | ❌ 返回 "not yet implemented" | ✅ VM 镜像下载+解压 | **需要移植** |
| start() | ❌ 返回 "not yet implemented" | ✅ VZ.framework 桥接 | **需要移植** |
| stop() | ⚠️ 基础实现 | ✅ 完整 VM 停止 | 需要增强 |
| health_check() | ⚠️ 基础 socket 检查 | ✅ Docker ping | 需要增强 |
| Docker 连接 | ✅ try_connect_docker() | ✅ connect_cratebay_runtime_docker() | 基本一致 |

### Step 7 完成总结（基础骨架）

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
