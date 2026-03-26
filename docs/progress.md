# CrateBay 开发进度

## 当前状态
- **阶段**: 开发阶段 (Phase 2) — Spec 对齐完成，聚焦 VM 网络修复 + 容器端到端验证 + Runtime 稳定化/移植（built-in runtime 主线 / Podman fallback）
- **日期**: 2026-03-25
- **团队模式**: 开发阶段 6 人团队（见 agent-team-workflow.md §1.1）
- **Git HEAD**: `rewrite/v2` 分支

## Runtime 策略（AI 必读）
- **built-in runtime 是唯一主线**：后续 runtime、container、image 相关开发默认优先修这条链路
- **Podman 只是 fallback / escape hatch**：仅用于兼容恢复、开发/CI 应急、或用户明确要求的特殊环境
- **不要把 Podman 当第二主线继续扩写**：非人工明确批准，不新增 Podman 专属产品能力或分叉架构
- **控制面边界保持 Docker-compatible**：继续围绕 `bollard`、Docker socket/host 语义实现
- **恢复会话时必须先读**：`AGENTS.md`、`docs/specs/runtime-spec.md`、`docs/references/tech-decisions.md` 中的 runtime 策略

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

## 已完成（Quick Resume 验证轮）✅ — 2026-03-21
- [x] Runtime 自动启动后端集成
  - ✅ AppState.docker 改为 `Arc<Mutex<Option<Arc<Docker>>>>` 支持动态更新
  - ✅ main.rs 后台线程自动启动 runtime (detect→provision→start→wait for Docker→fallback)
  - ✅ system.rs 新增 runtime_start, runtime_stop 命令 + docker_status source 检测
  - ✅ container.rs 所有命令适配新的 require_docker() 返回 Arc<Docker>
  - ✅ App.tsx 监听 docker:connected 事件刷新前端状态
  - ✅ Settings RuntimeTab 添加 Start/Stop 按钮 + runtimeLoading 状态
  - ✅ 编译通过：TypeScript 0 errors, Rust 0 errors (2 dead_code warnings)
- [x] 任务 A: GUI 验证 — 通过
  - ✅ pnpm build 成功 (Vite 6.4.1)
  - ✅ cargo build -p cratebay-gui 成功 (27s, 2 dead_code warnings: CONTAINER_STATUS_CHANGE, MCP_CONNECTION_CHANGE)
  - ✅ pnpm tauri dev 启动正常：runtime auto-start → 骨架失败 → fallback 外部 Docker → 前端正常渲染
  - ✅ Settings Runtime tab: Start/Stop 按钮、CPU/Memory 滑块均存在
  - ⚠️ CPU/Memory 滑块值仅保存在本地 state，未持久化到后端
- [x] 任务 B: 文档一致性审查 — 发现 7 个偏差
  - ⚠️ [高] runtime_start/runtime_stop 未在 api-spec.md 中定义 → 需更新 spec
  - ⚠️ [高] ImagesPage 代码存在但 frontend-spec 未定义 → 需人工决定保留或删除
  - ⚠️ [高] Settings 6 Tab vs spec 3 Tab → 需更新 spec
  - ⚠️ [高] AppState.docker 类型 Arc<Mutex<Option<Arc<Docker>>>> vs spec Option<Arc<Docker>> → 需更新 spec
  - ⚠️ [中] appStore 多出 runtimeLoading 字段 → 需更新 spec
  - ⚠️ [中] backend-spec 未列 runtime_start/stop 命令 → 需更新 spec
  - ✅ [低] provision() 回调 Box vs impl (async_trait 对象安全) → 可接受
- [x] 任务 C: 自研 Runtime 移植方案
  - ✅ master runtime 相关代码约 9857 行，v2 骨架 3017 行但核心逻辑缺失
  - ✅ 完全缺失：Hypervisor 实现 (macOS 1749/Linux 1941/Windows 1626 行) + OS 镜像系统 (738 行)
  - ✅ 推荐混合架构：保留 v2 RuntimeManager trait，内部委托 master 逻辑
  - ✅ 移植分 3 Phase，~10 文件，~5200 行，优先级 macOS → Linux → Windows

## 进行中 🔄

### GUI + Docker 集成优化（2026-03-22）

**已完成的修复：**
- ✅ Docker 镜像加速源：内置中国镜像源 + Settings 页自定义管理（添加/删除/恢复默认）
- ✅ image_pull 非阻塞：后端 `tokio::spawn` 后台拉取 + 前端 Tauri Event 监听完成
- ✅ 拉取超时保护：60s 整体超时 + 15s per-chunk 超时（tokio::time::timeout）
- ✅ 实时进度显示：容器卡片 placeholder 动态更新拉取百分比
- ✅ fetchContainers 不再卡死：AbortController 替代 loading guard + 8s 超时
- ✅ initRuntimeStatus 超时保护：5s Promise.race 超时
- ✅ 语言下拉修复：SelectValue 显示"简体中文"/"English"而非 value
- ✅ auto_start 优雅回退：容器创建成功但启动失败时返回容器信息，显示 warning 而非 error
- ✅ health_check 状态回退修复：后端+前端双重保护，防止 Docker ping 偶尔超时导致"运行中"→"启动中"
- ✅ 状态栏稳定性：health monitor 复用 AppState Docker client + 重试/阈值；`runtime_status` reconcile；前端 90s 降级宽限

### GUI 可用性修复（2026-03-23）

**用户阻塞问题修复：**
- ✅ Images 搜索不可用：新增 Settings > Runtime 的 HTTP Proxy 配置 + 重启提示；`runtime_start` 与 runtime auto-start 都会读取持久化设置并应用 `CRATEBAY_RUNTIME_HTTP_PROXY*` 环境变量
- ✅ 容器日志/终端为 mock：ContainerDetail 改为真实 `container_logs` + `container_exec_stream`（可输入命令并实时输出）
- ✅ 容器 CPU/MEM 展示语义纠正：列表卡片展示规格（CPU cores / Memory MB）；详情面板展示监控（`container_stats`，按分配规格计算占用百分比）
- ✅ 语言下拉显示修复：SelectValue 支持展示 label（简体中文/English）而非 raw value（zh-CN/en）
- ✅ LLM Providers / Chat 模型选择可用：修复 invoke 参数命名（snake_case），Providers/Models 在启动时加载；默认 provider/model 通过 settings key 持久化（`default_provider`/`default_model`）
- ✅ Agent 内置镜像工具：补齐 `image_*` AgentTools 并注册到工具表

**回归验证：**
- ✅ `cargo test --workspace`
- ✅ `pnpm -C crates/cratebay-gui typecheck`
- ✅ `pnpm -C crates/cratebay-gui test`
- ✅ `pnpm -C crates/cratebay-gui test:e2e`
- ✅ `pnpm -C crates/cratebay-gui build`
- ✅ macOS runtime HTTP 代理支持：`CRATEBAY_RUNTIME_HTTP_PROXY` + 可选 `CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE=1`
- ✅ Images API 补齐：image_list/search/inspect/remove/tag/pull + GUI commands 注册
- ✅ E2E 稳定性：统一浏览器模式 Tauri mock（`__MOCK_TAURI_INVOKE__` + `listen()` fallback）+ 补全 data-testid
- ✅ TypeScript typecheck：容器状态 badge 映射全覆盖 + i18n 增加 overview/ports
- ✅ 全量验证通过：`cargo test --workspace` / `pnpm test` / `pnpm test:e2e` / `pnpm typecheck` / `pnpm build`

**待处理的问题：**
1. ⚠️ **容器端到端真实运行仍需环境验证** — 若 VM 无法直连拉取，可配置 `CRATEBAY_RUNTIME_HTTP_PROXY`（必要时启用 `CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE=1`）或使用可用的 registry mirrors；需要一个包含 `/bin/sh` 的有效镜像来验证 create→start→exec→stop→delete 全流程

**关键代码变更文件：**
- `crates/cratebay-core/src/container.rs` — PullProgress/PullProgressCallback, image_pull 超时+mirrors+callback, auto_start 优雅回退
- `crates/cratebay-gui/src-tauri/src/commands/container.rs` — image_pull 非阻塞（tokio::spawn + event）
- `crates/cratebay-gui/src-tauri/src/events.rs` — ImagePullProgress struct + event 命名
- `crates/cratebay-gui/src/App.tsx` — initRuntimeStatus 超时, runtime:health 防降级, docker:connected 处理
- `crates/cratebay-gui/src/stores/containerStore.ts` — AbortController, createContainer 重写（pull event + placeholder 动态更新）
- `crates/cratebay-gui/src/pages/SettingsPage.tsx` — 语言修复 + RegistryMirrorsSection
- `crates/cratebay-gui/src/types/settings.ts` — registryMirrors + DEFAULT_REGISTRY_MIRRORS
- `crates/cratebay-core/src/runtime/macos.rs` — health_check 防降级（prev state = Ready → 保持 Ready）

### Engine Ensure + CLI/GUI（2026-03-24）

- ✅ `cratebay-core`: 新增 `engine::ensure_docker()` + `engine.lock` 跨进程互斥，统一“外部 Docker 优先 / 内置 runtime 自动启动”
- ✅ `cratebay-core`: Windows runtime Docker 连接改为 `tcp://127.0.0.1:<CRATEBAY_WSL_DOCKER_PORT>`（WSL localhost forwarding），避免 named pipe 连接失败
- ✅ `cratebay-core`: `DOCKER_HOST` 支持 unix/tcp/http/https/npipe；探测 ping 超时 5s（成功后返回 120s client），避免无效 DOCKER_HOST 卡住启动
- ✅ `cratebay-core`: Linux runtime helper（QEMU）资产发现改为 `runtime-linux/`（与 `scripts/build-runtime-assets-linux.sh` 产物一致）
- ✅ `cratebay-core`: WindowsRuntime health_check 对缓存端点失效自动回退到 localhost，避免“容器可用但 GUI 显示未连接”
- ✅ `cratebay-gui`(Tauri bundle): 资源打包加入 `runtime-linux/**/*` 与 `runtime-wsl/**/*`，为 Win/Linux “安装即用”准备完整 runtime 资产目录
- ✅ `cratebay-gui`(Tauri 后端): `AppState.ensure_docker()`，所有 container/image 命令自动 ensure；GUI 启动 auto-start 复用 core engine ensure
- ✅ `cratebay-cli`: 补齐 `container/*`、`image/*`、`system docker-status` 子命令；`container create` 缺镜像时自动 pull；支持 `image search`
- ✅ 回归验证：`cargo test --workspace`

### 状态栏抖动 + 并发阻塞修复（2026-03-25）

**问题 1：状态栏从"引擎就绪"间歇性回退"启动中"**

根因：health_check() 每轮新建 Docker 连接（非复用共享 client），瞬时 socket 抖动被误判为 runtime 不可用。

修复：
- ✅ **三平台降级阈值上调 2→3**：`READY_DOWNGRADE_FAILURE_THRESHOLD` 更新至 3（macos.rs:67 / linux.rs:65 / windows.rs:52），容忍更多瞬时 ping 失败
- ✅ **health monitor 改"共享连接优先"**：`main.rs` 中 `start_runtime_health_monitor()` 重构；共享 client 可用时直接广播 Ready，跳过 health_check() 新建连接；共享 client 不可用才走 health_check() 慢路径
- ✅ **health monitor 周期 30s→20s**：更快感知真实故障
- ✅ **重试策略增强**：`get_responsive_shared_docker()` 重试次数 3→5，间隔 300ms→200ms

**问题 2：容器操作、镜像操作、状态刷新互相阻塞**

根因：所有命令都调 `ensure_docker()` → 内部 `engine::ensure_docker()` 有跨进程文件锁，默认等待 10 分钟；并发命令在锁上排队；同时每次命令都 ping Docker 导致 ~5s 延迟。

修复：
- ✅ **AppState 新增 `docker_init: Arc<OnceCell<...>>`**：`state.rs` 增加字段，确保同进程内 Docker 初始化只执行一次
- ✅ **新增 `ensure_docker_once()` 方法**：`state.rs` 新方法；已有 client 时零延迟直接返回（无 ping）；无 client 时单例化初始化，并发调用等待同一 future；锁等待超时从 10 分钟缩短到 60 秒
- ✅ **container.rs 所有命令改用 `ensure_docker_once()`**：15 处 `ensure_docker()` 全部替换，消除并发排队
- ✅ **main.rs 初始化补入 `docker_init`**

**回归验证：**
- ✅ `cargo test --workspace`：332 passed / 5 ignored（0 failed）
- ✅ `pnpm typecheck`：0 错误
- ✅ `pnpm test`：216 passed / 2 skipped（0 failed），18 个测试文件
- ✅ `pnpm build`：构建成功；顺带修复 `tw-animate-css` 依赖缺失问题

**关键代码变更文件：**
- `crates/cratebay-core/src/runtime/macos.rs:67` — READY_DOWNGRADE_FAILURE_THRESHOLD 2→3
- `crates/cratebay-core/src/runtime/linux.rs:65` — READY_DOWNGRADE_FAILURE_THRESHOLD 2→3
- `crates/cratebay-core/src/runtime/windows.rs:52` — READY_DOWNGRADE_FAILURE_THRESHOLD 2→3
- `crates/cratebay-gui/src-tauri/src/main.rs` — health monitor 共享连接优先 + 周期 20s + 重试增强
- `crates/cratebay-gui/src-tauri/src/state.rs` — docker_init OnceCell + ensure_docker_once()
- `crates/cratebay-gui/src-tauri/src/commands/container.rs` — 15 处改用 ensure_docker_once()

### Spec 对齐 + 提交钩子修复（2026-03-25）

- ✅ `frontend-spec.md` 补齐 Settings 6 Tab、`appStore` 新字段（`builtinRuntimeReady` / `dockerSource`）、`allowExternalDocker`
- ✅ `backend-spec.md` 补齐 `AppState.docker_source`、`docker_init`、`ensure_docker_once()`、`set_docker_source()`
- ✅ `api-spec.md` 补齐 `allowExternalDocker` 设置键说明
- ✅ `.githooks/pre-commit` 将 `cargo test -p cratebay-cli --lib` 修正为 `cargo test -p cratebay-cli --bins`

## 待开始 📋

### 当前优先项
1. **VM 网络问题** — CrateBay 内置 VM 的 Docker 仍无法联网拉取镜像，所有 mirrors 和直连都超时
2. **容器创建端到端验证** — 需要解决网络问题或准备可用本地测试镜像，完成 create → start → exec → stop → delete 全流程验证
3. **任务 F: 自研 Runtime 稳定化/移植实施**
   - 策略前提：**built-in runtime 为主线，Podman 仅为 fallback**
   - Phase 1: 基础设施（images.rs, fsutil.rs, store 兼容层）
   - Phase 2: 核心 Runtime（common.rs + 重写 macos/linux/windows.rs）
   - Phase 3: 集成（mod.rs + main.rs + system.rs 适配）
   - 优先级：macOS → Linux → Windows

### 本轮已完成
- ✅ 任务 E: Spec 文档对齐更新
- ✅ 任务 D: 修复 pre-commit 钩子 Bug

### 待人工决策
1. **是否立即进入 Runtime 移植实施？** 若进入，按 macOS → Linux → Windows 执行

## 阻塞/问题 ⚠️
- **VM 网络问题**: 内置 VM 中 Docker 拉取镜像超时，阻塞真实容器端到端验证
- **CodeBuddy Agent 框架 Bug**: 进程内 agent 注册表持久化，跨 TeamDelete 后仍阻止创建同名 agent，需要**重启新会话**才能创建新团队

## 文档完成明细

16 份文档（以文件头 Version 为准）：

| 文档 | 版本 | 路径 |
|------|------|------|
| architecture.md | **1.1.2** | docs/specs/architecture.md |
| backend-spec.md | **1.3.4** | docs/specs/backend-spec.md |
| runtime-spec.md | **1.2.5** | docs/specs/runtime-spec.md |
| api-spec.md | **1.5.4** | docs/specs/api-spec.md |
| database-spec.md | **1.1.0** | docs/specs/database-spec.md |
| frontend-spec.md | **1.2.6** | docs/specs/frontend-spec.md |
| agent-spec.md | **1.2.2** | docs/specs/agent-spec.md |
| mcp-spec.md | **1.1.0** | docs/specs/mcp-spec.md |
| testing-spec.md | **1.1.0** | docs/specs/testing-spec.md |
| dev-workflow.md | 1.0.0 | docs/workflow/dev-workflow.md |
| agent-team-workflow.md | 1.0.0 | docs/workflow/agent-team-workflow.md |
| knowledge-base.md | 1.0.0 | docs/workflow/knowledge-base.md |
| tech-decisions.md | **1.1.1** | docs/references/tech-decisions.md |
| glossary.md | **1.0.1** | docs/references/glossary.md |
| docs/README.md | **1.0.1** | docs/README.md |
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

### 规则 3: Runtime 策略固定
- built-in runtime 是产品主线
- Podman 只作为 fallback / escape hatch
- runtime、container、image 相关问题，优先修 built-in runtime 主链路
- 非人工明确批准，不新增 Podman-only 产品能力或把 Podman 提升为默认路径

---

## 下次继续（Quick Resume）

> **给 AI 的可执行指令** — 新会话启动后读取此段，按步骤执行。

### 当前阶段：Spec 对齐已完成，聚焦 VM 网络修复 + 容器端到端验证 + Runtime 移植决策（2026-03-25）

Step 0-7 基础骨架全部完成。GUI 已经能构建安装运行，Docker 基本集成完成。状态栏抖动和命令并发阻塞问题、Spec 对齐与 pre-commit 钩子问题已修复。

**执行约束：** built-in runtime 是主线；Podman 仅作 fallback。新会话恢复后，不要把 Podman 当成并行主路线继续扩展。

### 已解决问题（本次会话）
- ✅ 状态栏从"引擎就绪"回退"启动中"：health monitor 共享连接优先 + 降级阈值上调
- ✅ 容器/镜像/状态操作互相阻塞：OnceCell 单例化初始化 + 快速路径无 ping
- ✅ Spec 文档与代码实现重新对齐：补齐 `frontend-spec.md` / `backend-spec.md` / `api-spec.md`
- ✅ pre-commit 钩子误拦截：`cratebay-cli --lib` 改为 `cratebay-cli --bins`

### 剩余优先修复项
1. **VM 网络问题** — CrateBay 内置 VM 的 Docker 无法联网拉取镜像，所有 mirrors 和直连都超时。这影响容器创建的完整测试
2. **容器创建端到端验证** — 需要解决网络问题或找到其他方式创建有效测试镜像

### 待人工决策（阻塞后续工作）
1. **是否立即进入 Runtime 移植实施？** 若进入，优先 macOS

### 可执行步骤

```
1. 读取 AGENTS.md + 本文件（progress.md）
2. 按"用户永久规则"创建 cratebay-dev 固定团队
3. 先确认并遵守 runtime 策略：built-in runtime 主线，Podman fallback
4. 优先处理 VM 网络问题，确认内置 VM 能拉取镜像
5. 完成容器 create → start → exec → stop → delete 端到端验证
6. 询问用户是否立即开始任务 F（Runtime 移植，macOS 优先）
7. 若用户确认，执行任务 F：
   - Phase 1: 基础设施移植
   - Phase 2: 核心 Runtime 重写
   - Phase 3: 集成
8. 完成后更新本文件
```

### Runtime 移植方案概要（来自 runtime-dev 分析）

```
架构选择：混合架构
  - 保留 v2 的 RuntimeManager trait（async, 进度回调, 健康检查）
  - 不移植 master 的 Hypervisor trait（避免两层抽象）
  - 将 master 的实现逻辑直接嵌入 RuntimeManager impl

master runtime 代码: ~9857 行
v2 骨架代码: ~3017 行（核心逻辑全缺失）
移植新增: ~5200 行，~10 文件

缺失模块:
  - Hypervisor 实现: macOS 1749 + Linux 1941 + Windows 1626 行
  - OS 镜像目录系统: 738 行
  - 文件工具: 39 行
  - runtime.rs 协调层: 2839 行（全局配置+资产查找+镜像安装+VM 生命周期+Docker ping）

关键风险:
  - macOS VZ.framework 通过外部 runner 进程（Swift）桥接，需确认是否继续使用
  - master 是同步代码，v2 是 async trait，需 spawn_blocking 包装
  - VM 状态存储: master 用 JSON 文件，v2 用 SQLite，需决定方案
```

### 文档偏差修复记录（2026-03-25）

| # | 严重性 | 描述 | 当前状态 |
|---|--------|------|----------|
| 1 | 高 | runtime_start/runtime_stop 未在 api-spec.md 中定义 | ✅ 已修复 |
| 2 | 高 | ImagesPage 代码存在但 frontend-spec 未定义 | ✅ 已修复 |
| 3 | 高 | Settings 6 Tab vs spec 3 Tab | ✅ 已修复 |
| 4 | 高 | AppState.docker 类型不匹配 spec | ✅ 已修复 |
| 5 | 中 | appStore 新字段未写入 spec | ✅ 已修复 |
| 6 | 中 | backend-spec 未列 runtime_start/stop 与 ensure 相关变更 | ✅ 已修复 |
| 7 | 低 | provision() 回调 Box vs impl | 可接受 |

### 历史偏差修复记录

> Step 3/4 偏差已在 Step 4 修复轮中全部解决（step4-fix 团队）。
> Step 5 架构师验收 20/20 零偏差。
> Step 6 架构师验收 28/30，2 个低严重度 spec 偏差已修复 (mcp-spec.md v1.1.0)。
> Step 7 所有测试通过，CI/CD 管线就绪，文档一致性验证完毕。
