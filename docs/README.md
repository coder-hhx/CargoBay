# CrateBay 文档索引

> 版本: 1.0.0 | 更新日期: 2026-03-20 | 作者: product-manager

---

## 1. 文档总览

CrateBay 的文档体系由 **16 份文档** 组成，分为四大类别：

```
docs/
├── README.md                     ← 本文件（文档索引）
├── progress.md                   ← 开发进度追踪（跨机器恢复）
│
├── specs/                        ← 技术规范（9 份，英文，有版本号）
│   ├── architecture.md
│   ├── frontend-spec.md
│   ├── backend-spec.md
│   ├── agent-spec.md
│   ├── database-spec.md
│   ├── runtime-spec.md
│   ├── api-spec.md
│   ├── mcp-spec.md
│   └── testing-spec.md
│
├── workflow/                     ← 流程文档（3 份，中文）
│   ├── dev-workflow.md
│   ├── agent-team-workflow.md
│   └── knowledge-base.md
│
└── references/                   ← 参考资料（2 份，英文）
    ├── tech-decisions.md
    └── glossary.md
```

此外，项目根目录的 `AGENTS.md` 是 AI Agent 的主入口文件，汇总了项目全景和导航信息。

---

## 2. 技术规范

技术规范是开发的核心依据。所有代码实现必须与 spec 保持一致。

| # | 文档 | 版本 | 语言 | 用途 |
|---|------|------|------|------|
| 1 | [architecture.md](specs/architecture.md) | 1.0.0 | EN | 系统架构总览，crate 依赖关系，设计原则，跨平台策略 |
| 2 | [frontend-spec.md](specs/frontend-spec.md) | 1.0.0 | EN | 前端技术规范：React/shadcn/Zustand 标准，页面架构，组件设计 |
| 3 | [backend-spec.md](specs/backend-spec.md) | 1.0.0 | EN | 后端技术规范：Rust 编码规范，Tauri commands 设计，CLI 设计 |
| 4 | [agent-spec.md](specs/agent-spec.md) | 1.0.0 | EN | Agent 集成规范：pi-agent-core 集成，工具定义，LLM Proxy |
| 5 | [database-spec.md](specs/database-spec.md) | 1.0.0 | EN | 数据库设计：SQLite schema，迁移策略，加密方案 |
| 6 | [runtime-spec.md](specs/runtime-spec.md) | 1.0.0 | EN | 内置容器运行时：VZ.framework/KVM/WSL2 设计 |
| 7 | [api-spec.md](specs/api-spec.md) | 1.0.0 | EN | Tauri Commands API 完整目录，参数，返回值，错误定义 |
| 8 | [mcp-spec.md](specs/mcp-spec.md) | 1.0.0 | EN | MCP Server + Client 规范，MCP Bridge 设计 |
| 9 | [testing-spec.md](specs/testing-spec.md) | 1.0.0 | EN | 测试策略：测试金字塔，CI/CD 流水线，覆盖率要求 |

---

## 3. 流程文档

流程文档定义了开发团队的工作方式和协作规范。

| # | 文档 | 版本 | 语言 | 用途 |
|---|------|------|------|------|
| 1 | [dev-workflow.md](workflow/dev-workflow.md) | 1.0.0 | 中文 | Spec-Driven 开发流程：需求→spec→设计→实现→测试→文档更新 |
| 2 | [agent-team-workflow.md](workflow/agent-team-workflow.md) | 1.0.0 | 中文 | Agent 团队协作：动态组队、人类介入模式、任务并行、代码审查 |
| 3 | [knowledge-base.md](workflow/knowledge-base.md) | 1.0.0 | 中文 | 知识库管理：六层结构、更新触发映射、自动更新机制、版本号管理 |

---

## 4. 参考资料

| # | 文档 | 版本 | 语言 | 用途 |
|---|------|------|------|------|
| 1 | [tech-decisions.md](references/tech-decisions.md) | 1.0.0 | EN | 技术决策记录（ADR 格式），12 个初始决策 |
| 2 | [glossary.md](references/glossary.md) | 1.0.0 | EN | 术语表：项目核心概念和技术术语定义 |

---

## 5. 按角色推荐阅读

### 全体 Agent

所有 Agent 加入项目时必须阅读：

1. `AGENTS.md` -- 项目全景和导航
2. `.codebuddy/project.yaml` -- 当前阶段和团队
3. `docs/progress.md` -- 进度和断点
4. [agent-team-workflow.md](workflow/agent-team-workflow.md) -- 协作规范

### architect / team-lead

| 文档 | 优先级 |
|------|--------|
| [architecture.md](specs/architecture.md) | 必读 |
| [backend-spec.md](specs/backend-spec.md) | 必读 |
| [api-spec.md](specs/api-spec.md) | 必读 |
| [tech-decisions.md](references/tech-decisions.md) | 必读 |
| 其他全部 spec | 通读 |

### frontend-dev

| 文档 | 优先级 |
|------|--------|
| [frontend-spec.md](specs/frontend-spec.md) | 必读 |
| [api-spec.md](specs/api-spec.md) | 必读 |
| [agent-spec.md](specs/agent-spec.md) | 推荐 |
| [dev-workflow.md](workflow/dev-workflow.md) | 推荐 |

### backend-dev

| 文档 | 优先级 |
|------|--------|
| [backend-spec.md](specs/backend-spec.md) | 必读 |
| [database-spec.md](specs/database-spec.md) | 必读 |
| [api-spec.md](specs/api-spec.md) | 必读 |
| [mcp-spec.md](specs/mcp-spec.md) | 推荐 |
| [dev-workflow.md](workflow/dev-workflow.md) | 推荐 |

### ai-engineer

| 文档 | 优先级 |
|------|--------|
| [agent-spec.md](specs/agent-spec.md) | 必读 |
| [api-spec.md](specs/api-spec.md) | 必读 |
| [frontend-spec.md](specs/frontend-spec.md) | 推荐 |
| [mcp-spec.md](specs/mcp-spec.md) | 推荐 |

### runtime-dev

| 文档 | 优先级 |
|------|--------|
| [runtime-spec.md](specs/runtime-spec.md) | 必读 |
| [backend-spec.md](specs/backend-spec.md) | 必读 |
| [architecture.md](specs/architecture.md) | 推荐 |

### tester

| 文档 | 优先级 |
|------|--------|
| [testing-spec.md](specs/testing-spec.md) | 必读 |
| [api-spec.md](specs/api-spec.md) | 必读 |
| [frontend-spec.md](specs/frontend-spec.md) | 推荐 |
| [backend-spec.md](specs/backend-spec.md) | 推荐 |

### doc-keeper

| 文档 | 优先级 |
|------|--------|
| [knowledge-base.md](workflow/knowledge-base.md) | 必读 |
| [dev-workflow.md](workflow/dev-workflow.md) | 必读 |
| [glossary.md](references/glossary.md) | 必读 |
| 全部 spec | 通读 |

---

## 6. 文档版本号规范

### 版本格式

所有文档顶部标注版本号，遵循语义化版本（SemVer）：

```
MAJOR.MINOR.PATCH
```

| 递增类型 | 触发条件 | 示例 |
|---------|---------|------|
| **MAJOR** | 不兼容的破坏性变更 | API 删除、schema 迁移、架构重构 |
| **MINOR** | 新增功能（向后兼容） | 新增 API、新增组件、新增表 |
| **PATCH** | 修复和补充 | 文档错误修正、描述补充、示例更新 |

### 文档头格式

英文文档：
```markdown
# Document Title
> Version: 1.0.0 | Last Updated: 2026-03-20 | Author: <role>
```

中文文档：
```markdown
# 文档标题
> 版本: 1.0.0 | 更新日期: 2026-03-20 | 作者: <role>
```

### 版本号维护规则

- 每份文档独立管理版本号
- 更新文档时必须同时递增版本号和更新日期
- 多份文档因同一需求更新时，各自独立递增
- 文档版本号与项目版本号（CrateBay v2.0.0）无直接关联
