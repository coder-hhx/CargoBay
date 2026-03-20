# Spec-Driven 开发流程

> 版本: 1.0.0 | 更新日期: 2026-03-20 | 作者: product-manager

---

## 1. 流程总览

CrateBay 采用 **Spec-Driven**（规范驱动）开发流程。任何功能的实现都必须以 spec 文档为起点，经过设计、实现、测试、文档更新的完整闭环。

```
需求输入 ──→ Spec 更新 ──→ 设计审核 ──→ 编码实现 ──→ 测试验证 ──→ 文档更新 ──→ 人类验收
   │              │              │              │              │              │              │
 人类描述      Agent 编写     人类审核      Agent 开发     Agent 测试    Agent 更新     人类确认
 或决策变更    spec 草案      (破坏性变更)   遵循 spec     CI 通过       知识库同步     交付物完整
```

### 核心原则

1. **Spec First** -- 先更新规范文档，再写代码
2. **Single Source of Truth** -- spec 文档是唯一权威，代码必须与 spec 一致
3. **Human-in-the-Loop** -- 人类只做决策（需求、审核、验收），AI 负责全部执行
4. **Knowledge Base Sync** -- 每次功能完成后，强制更新相关知识库文档

---

## 2. 需求阶段

### 2.1 需求来源

需求由人类以自然语言描述，通常包含：

- **功能需求** -- 新功能描述、用户故事
- **技术决策** -- 技术选型变更、架构调整
- **缺陷修复** -- Bug 报告、性能问题
- **改进建议** -- 用户体验优化、代码重构

### 2.2 Agent 需求分析

team-lead 接收需求后执行分析：

```
人类描述需求
    |
    v
team-lead 分析：
    1. 解读需求意图（不臆想、不扩展，忠实于原始描述）
    2. 确认影响范围（涉及哪些 crate、哪些模块）
    3. 识别需要更新的 spec 文档列表
    4. 评估是否有破坏性变更（breaking change）
    5. 识别前置依赖和阻塞项
    |
    v
输出：需求分析摘要 + 待更新 spec 列表
```

### 2.3 Spec 更新（Spec-First）

根据需求分析结果，由对应角色更新 spec 文档：

| 需求类型 | 更新的 spec | 负责人 |
|---------|------------|--------|
| 前端功能 | frontend-spec.md | frontend-dev |
| 后端功能 | backend-spec.md, api-spec.md | backend-dev |
| Agent 工具 | agent-spec.md | ai-engineer |
| 数据库变更 | database-spec.md | backend-dev |
| 运行时功能 | runtime-spec.md | runtime-dev |
| MCP 变更 | mcp-spec.md | backend-dev |
| 架构变更 | architecture.md | team-lead |

**Spec 更新规范**：

- 递增 spec 文档版本号（语义化版本）
- 更新 `Last Updated` 时间戳
- 新增章节或修改现有章节，保持结构完整
- 如果是破坏性变更，在 spec 顶部标注 `BREAKING CHANGE`

---

## 3. 设计阶段

### 3.1 方案设计

开发 Agent 根据 spec 设计实现方案：

```
读取更新后的 spec
    |
    v
设计实现方案：
    1. 确认 API 接口签名（参数、返回值、错误类型）
    2. 确认数据模型（struct 定义、数据库表结构）
    3. 确认前端组件结构（页面、组件、Store 变更）
    4. 确认测试策略（单元测试、集成测试、E2E）
    5. 识别可复用的现有代码（以复用为荣，以创造为耻）
    |
    v
输出：实现方案摘要
```

### 3.2 人类审核

以下情况需要人类审核方案：

| 场景 | 审核内容 | 是否阻塞 |
|-----|---------|---------|
| **破坏性 API 变更** | spec diff | 是，必须通过 |
| **数据库 schema 迁移** | 迁移脚本设计 | 是，必须通过 |
| **新增外部依赖** | 依赖评估 | 是，必须通过 |
| **架构层面调整** | architecture.md diff | 是，必须通过 |
| **UI 视觉变更** | 设计描述/截图 | 否，AI 先自评 |
| **内部重构** | 无需审核 | 否 |

---

## 4. 实现阶段

### 4.1 编码规范

所有代码必须遵循对应的技术规范：

- **Rust 代码** -- 遵循 `backend-spec.md` 中的编码规范
  - 错误处理：`thiserror` + `Result<T, AppError>`，禁止 `unwrap()` 
  - Mutex：使用 `lock_or_recover()`，禁止 `.lock().unwrap()`
  - 平台代码：使用 `#[cfg(target_os = "...")]` 门控
  - 异步：tokio runtime，禁止在异步上下文中同步阻塞
- **TypeScript 代码** -- 遵循 `frontend-spec.md` 中的编码规范
  - 严格类型：禁止 `any`
  - 组件：使用 shadcn/ui 模式 + Radix 原语
  - 状态：Zustand stores，禁止 prop drilling
  - 样式：Tailwind CSS v4 + CSS 变量
- **Commit 规范** -- Conventional Commits 格式
  - `feat:` / `fix:` / `docs:` / `test:` / `refactor:` / `chore:`
  - 主题行不超过 72 字符

### 4.2 开发流程

```
Agent 领取任务（TaskList claim）
    |
    v
标记任务 in_progress（TaskUpdate）
    |
    v
读取相关 spec 文档
    |
    v
按 spec 编写代码
    |
    v
编写对应的测试代码
    |
    v
本地运行测试确认通过
    |
    v
提交代码（Conventional Commits）
    |
    v
标记任务 completed（TaskUpdate）
    |
    v
检查是否触发文档更新（见第 7 节）
```

### 4.3 代码质量要求

- **编译通过**：`cargo check --workspace` 无 error
- **Lint 通过**：`cargo clippy` 无 warning（`#[allow]` 需注释原因）
- **格式化**：`cargo fmt` 已执行
- **前端 Lint**：ESLint + Prettier 通过
- **类型检查**：TypeScript `tsc --noEmit` 通过

---

## 5. 测试阶段

### 5.1 测试金字塔

遵循 `testing-spec.md` 定义的测试分层策略：

```
          /  E2E 测试  \          ← Playwright（少量关键流程）
         / 集成测试     \         ← Tauri invoke 模拟
        / 组件测试       \        ← Vitest + Testing Library
       / 单元测试（前端）  \      ← Vitest
      / 单元测试（后端）    \     ← cargo test
     / 性能基准测试         \    ← Criterion
    /_________________________\
```

### 5.2 测试执行

| 阶段 | 命令 | 执行者 |
|------|------|--------|
| 开发中 | `cargo test -p <crate>` | 开发 Agent |
| 开发中 | `pnpm test -- <file>` | 开发 Agent |
| PR 前 | `cargo test --workspace` | tester |
| PR 前 | `pnpm test && pnpm test:e2e` | tester |
| CI | `./scripts/ci-local.sh` | CI pipeline |

### 5.3 测试失败处理

```
测试失败
    |
    v
tester 报告失败信息
    |
    v
原开发 Agent 自动修复
    |
    v
重新运行测试
    |
    +--→ 通过 → 继续流程
    |
    +--→ 仍失败 → team-lead 介入，分析原因
```

---

## 6. 文档更新阶段

**这是 Spec-Driven 流程中最关键的环节。** 每个功能完成后，必须执行文档更新检查。

### 6.1 必须更新的文档

| 完成的工作 | 需要更新的文档 |
|-----------|---------------|
| 任何功能完成 | `docs/progress.md` -- 标记完成状态 |
| Tauri Command 变更 | `docs/specs/api-spec.md` -- 更新 API 目录 |
| 数据库 Schema 变更 | `docs/specs/database-spec.md` -- 更新表结构 |
| 前端页面/组件变更 | `docs/specs/frontend-spec.md` -- 更新组件文档 |
| Agent 工具变更 | `docs/specs/agent-spec.md` -- 更新工具目录 |
| MCP 工具变更 | `docs/specs/mcp-spec.md` -- 更新 MCP 文档 |
| 仓库结构变更 | `AGENTS.md` -- 更新目录树 |
| 依赖变更 (Cargo.toml) | `docs/specs/architecture.md` -- 更新依赖表 |
| 依赖变更 (package.json) | `docs/specs/frontend-spec.md` + `architecture.md` |
| 平台运行时变更 | `docs/specs/runtime-spec.md` |
| 测试策略变更 | `docs/specs/testing-spec.md` |
| 技术决策变更 | `docs/references/tech-decisions.md` -- 新增 ADR |

### 6.2 版本号递增规则

每份 spec 文档顶部包含版本号，更新时遵循语义化版本：

| 变更类型 | 版本递增 | 示例 |
|---------|---------|------|
| 破坏性变更（API 不兼容） | Major | 1.0.0 → 2.0.0 |
| 新增功能（向后兼容） | Minor | 1.0.0 → 1.1.0 |
| 修复/补充（无功能变化） | Patch | 1.0.0 → 1.0.1 |

### 6.3 文档更新检查清单

功能完成后，执行以下检查：

```
[ ] progress.md 已更新（标记完成状态）
[ ] 相关 spec 文档已更新（根据映射表）
[ ] spec 版本号已递增
[ ] spec 更新日期已刷新
[ ] AGENTS.md 仓库结构是否需要更新
[ ] 新增的技术决策是否需要记录 ADR
```

---

## 7. 知识库更新触发器

### 7.1 代码变更 → 文档更新映射表

以下映射表定义了代码变更自动触发的文档更新需求：

| 变更文件路径 | 触发更新 |
|------------|---------|
| `crates/cratebay-gui/src-tauri/src/commands/*.rs` | api-spec.md |
| `crates/cratebay-core/src/storage.rs` 或 migration 文件 | database-spec.md |
| `crates/cratebay-gui/src/stores/*.ts` | frontend-spec.md |
| `crates/cratebay-gui/src/tools/*.ts` | agent-spec.md |
| `crates/cratebay-mcp/src/**` | mcp-spec.md |
| `Cargo.toml`（workspace 级别依赖） | architecture.md |
| `package.json`（前端依赖） | frontend-spec.md, architecture.md |
| `crates/` 目录结构变化 | AGENTS.md |
| `crates/cratebay-core/src/runtime/**` | runtime-spec.md |
| 测试配置文件变化 | testing-spec.md |

### 7.2 触发机制

```
Agent 提交代码
    |
    v
pre-commit hook 检测变更文件
    |
    v
根据映射表判断需要更新的文档
    |
    +--→ 有文档需要更新 → 提醒 Agent 或 doc-keeper 执行更新
    |
    +--→ 无文档需要更新 → 正常提交
```

### 7.3 Pre-commit Hook 示例

```bash
#!/bin/bash
# .githooks/pre-commit (文档同步检查部分)

CHANGED_FILES=$(git diff --cached --name-only)

# Tauri commands 变更检查
if echo "$CHANGED_FILES" | grep -q "src/commands/"; then
  if ! echo "$CHANGED_FILES" | grep -q "docs/specs/api-spec.md"; then
    echo "WARNING: Tauri commands changed but api-spec.md not updated"
  fi
fi

# 数据库 schema 变更检查
if echo "$CHANGED_FILES" | grep -q "storage.rs\|migration"; then
  if ! echo "$CHANGED_FILES" | grep -q "docs/specs/database-spec.md"; then
    echo "WARNING: Database schema changed but database-spec.md not updated"
  fi
fi

# Zustand store 变更检查
if echo "$CHANGED_FILES" | grep -q "src/stores/"; then
  if ! echo "$CHANGED_FILES" | grep -q "docs/specs/frontend-spec.md"; then
    echo "WARNING: Zustand store changed but frontend-spec.md not updated"
  fi
fi

# Agent tool 变更检查
if echo "$CHANGED_FILES" | grep -q "src/tools/"; then
  if ! echo "$CHANGED_FILES" | grep -q "docs/specs/agent-spec.md"; then
    echo "WARNING: Agent tool changed but agent-spec.md not updated"
  fi
fi
```

---

## 8. 完整流程示例

以"新增容器模板功能"为例，展示完整的 Spec-Driven 流程：

```
1. [人类] 描述需求："支持自定义容器模板，用户可以创建和管理自己的模板"

2. [team-lead] 需求分析：
   - 涉及 crate: cratebay-core, cratebay-gui
   - 需要更新 spec: database-spec.md, api-spec.md, frontend-spec.md
   - 无破坏性变更

3. [backend-dev] 更新 database-spec.md:
   - 新增 custom_templates 表定义
   - 版本: 1.0.0 → 1.1.0

4. [backend-dev] 更新 api-spec.md:
   - 新增 template_create, template_update, template_delete 命令
   - 版本: 1.0.0 → 1.1.0

5. [frontend-dev] 更新 frontend-spec.md:
   - 新增 TemplateEditor 组件描述
   - 更新 containerStore 接口
   - 版本: 1.0.0 → 1.1.0

6. [人类] 审核 spec 变更 → 通过

7. [backend-dev] 实现后端:
   - 新增 SQLite migration
   - 新增 Tauri commands
   - 编写单元测试

8. [frontend-dev] 实现前端:
   - 新增 TemplateEditor 组件
   - 更新 containerStore
   - 编写组件测试

9. [tester] 运行测试 → 全部通过

10. [doc-keeper / team-lead] 文档更新:
    - progress.md → 标记完成
    - 确认 api-spec, database-spec, frontend-spec 已在步骤 3-5 更新

11. [人类] 验收 → 通过
```

---

## 9. 常见问题

### Q: Spec 文档与代码不一致怎么办？

以 spec 为准。发现不一致时，先更新 spec（如果代码的实现更合理），或修改代码以符合 spec。不允许"代码已经这样了，就不改 spec 了"的情况。

### Q: 紧急修复是否需要走完整流程？

是的，但可以简化。紧急 hotfix 可以先实现代码、同时更新 spec，但文档更新步骤不可省略。commit message 使用 `fix:` 前缀。

### Q: 多个 Agent 同时修改同一份 spec 怎么办？

通过 TaskList 的 `blockedBy` 机制避免冲突。如果确实需要并行修改同一份 spec，各 Agent 修改不同章节，team-lead 负责合并。

### Q: 人类长时间不响应审核怎么办？

Agent 可以继续处理不依赖该审核的其他任务。将阻塞项记录在 `progress.md` 的"阻塞/问题"section 中，等待人类介入。
