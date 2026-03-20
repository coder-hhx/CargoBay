# 知识库管理与自动更新规范

> 版本: 1.0.0 | 更新日期: 2026-03-20 | 作者: product-manager

---

## 1. 知识库结构

CrateBay 采用六层持久化知识库体系，确保任何 AI Agent 在任何电脑上都能自动加载项目上下文。

### 1.1 六层结构总览

```
L1  AGENTS.md                    ← 全局指南（单一源头）
      |
L2  docs/specs/*.md              ← 技术规范（按需读取，有版本号）
      |
L3  docs/workflow/*.md           ← 流程文档（协作规范）
      |
L4  docs/references/*.md         ← 参考资料（ADR、术语表）
      |
L5  .codebuddy/                  ← 项目配置（机器可读）
      |   project.yaml               项目元数据、阶段、团队
      |   rules/*.mdc                编码规则增强
      |   tasks/                     任务快照
      |
L6  .mcp.json                   ← 工具配置（MCP Server）
```

### 1.2 各层详细说明

#### L1: AGENTS.md -- 全局指南

| 属性 | 说明 |
|------|------|
| 位置 | 项目根目录 |
| 语言 | 英文 |
| 用途 | AI Agent 启动时读取的主入口文件 |
| 链接 | `.cursorrules`, `CLAUDE.md`, `.github/copilot-instructions.md` |
| 更新频率 | 仓库结构变化时 |

内容包含：
- 项目标识与定位
- 技术栈总表
- 仓库目录结构
- 架构图
- 构建和测试命令
- 文档索引
- 编码规范
- Spec-Driven 工作流摘要
- 知识库更新协议摘要

#### L2: docs/specs/ -- 技术规范

| 文档 | 语言 | 关注角色 |
|------|------|---------|
| architecture.md | EN | 全体 |
| frontend-spec.md | EN | frontend-dev, ai-engineer |
| backend-spec.md | EN | backend-dev, runtime-dev |
| agent-spec.md | EN | ai-engineer, frontend-dev |
| database-spec.md | EN | backend-dev |
| runtime-spec.md | EN | runtime-dev |
| api-spec.md | EN | frontend-dev, backend-dev, tester |
| mcp-spec.md | EN | backend-dev, ai-engineer |
| testing-spec.md | EN | tester, 全体 |

每份 spec 顶部包含：
```
> Version: x.y.z | Last Updated: YYYY-MM-DD | Author: <role>
```

#### L3: docs/workflow/ -- 流程文档

| 文档 | 语言 | 用途 |
|------|------|------|
| dev-workflow.md | 中文 | Spec-Driven 开发流程 |
| agent-team-workflow.md | 中文 | Agent 团队协作规范 |
| knowledge-base.md | 中文 | 知识库管理（本文档） |

#### L4: docs/references/ -- 参考资料

| 文档 | 语言 | 用途 |
|------|------|------|
| tech-decisions.md | EN | 技术决策记录（ADR 格式） |
| glossary.md | EN | 术语表 |

#### L5: .codebuddy/ -- 项目配置

```
.codebuddy/
├── project.yaml          # 项目元数据、当前阶段、团队配置
├── rules/                # 项目级编码规则
│   ├── coding-standards.mdc
│   ├── spec-driven.mdc
│   └── team-guidelines.mdc
└── tasks/                # 任务列表快照
    └── .tasklist.json
```

`project.yaml` 是机器可读的项目状态文件，包含：
- 项目名称和版本
- 当前开发阶段
- 各阶段的团队成员配置
- 上次工作会话信息（跨机器恢复）

#### L6: .mcp.json -- 工具配置

MCP Server 配置文件，Agent 读取后自动连接可用工具：
```json
{
  "mcpServers": {
    "shadcn": {
      "command": "npx",
      "args": ["shadcn@latest", "mcp"]
    }
  }
}
```

### 1.3 进度追踪文件

`docs/progress.md` 是跨机器恢复的关键文件，包含：

| 区块 | 内容 |
|------|------|
| 当前状态 | 阶段、日期、活跃 Agent |
| 已完成 | 完成的功能列表及日期 |
| 进行中 | 各 Agent 正在做的工作及进度百分比 |
| 待开始 | 排队中的功能及依赖关系 |
| 阻塞/问题 | 已知的阻塞项和问题 |
| 下次继续 | 换电脑恢复的关键信息 |

---

## 2. 更新触发条件映射表

当代码发生变更时，必须同步更新对应的知识库文档。以下映射表定义了完整的触发关系。

### 2.1 完整映射表

| 代码变更路径 | 需要更新的文档 | 版本递增 |
|-------------|---------------|---------|
| `crates/cratebay-gui/src-tauri/src/commands/*.rs` | api-spec.md | Minor (新增) / Patch (修改) |
| `crates/cratebay-core/src/storage.rs` | database-spec.md | Minor (新表) / Patch (字段) |
| migration 文件 | database-spec.md | Minor |
| `crates/cratebay-gui/src/stores/*.ts` | frontend-spec.md | Minor (新 Store) / Patch (修改) |
| `crates/cratebay-gui/src/pages/*.tsx` | frontend-spec.md | Minor (新页面) / Patch (修改) |
| `crates/cratebay-gui/src/components/**` | frontend-spec.md | Patch |
| `crates/cratebay-gui/src/tools/*.ts` | agent-spec.md | Minor (新 Tool) / Patch (修改) |
| `crates/cratebay-mcp/src/**` | mcp-spec.md | Minor / Patch |
| `Cargo.toml`（workspace 依赖） | architecture.md | Minor (新增) / Patch (升级) |
| `package.json`（前端依赖） | frontend-spec.md + architecture.md | Minor / Patch |
| `crates/` 目录结构变化 | AGENTS.md | Patch |
| `crates/cratebay-core/src/runtime/**` | runtime-spec.md | Minor / Patch |
| 测试配置文件 (vitest.config.ts, etc.) | testing-spec.md | Patch |
| `.github/workflows/*.yml` | testing-spec.md | Patch |
| 技术选型变更 | tech-decisions.md（新增 ADR） | -- |
| 新术语引入 | glossary.md | -- |

### 2.2 特殊触发规则

| 事件 | 触发操作 |
|------|---------|
| 任何功能完成 | 更新 progress.md |
| 阶段切换 | 更新 project.yaml + progress.md + AGENTS.md |
| 破坏性 API 变更 | api-spec.md Major 版本递增 |
| 新增外部依赖 | architecture.md + 对应 spec + tech-decisions.md (ADR) |
| 安全相关变更 | 相关 spec + AGENTS.md security section |

### 2.3 反向映射（文档 → 代码检查）

当 spec 文档被更新时，应确认对应代码已同步：

| spec 更新 | 应检查的代码 |
|-----------|-------------|
| api-spec.md 新增命令 | Tauri command 是否已实现 |
| database-spec.md 新增表 | migration 是否已创建 |
| frontend-spec.md 新增组件 | 组件文件是否已创建 |
| agent-spec.md 新增工具 | Tool 定义是否已实现 |

---

## 3. 自动更新机制

### 3.1 三层更新保障

```
第一层：pre-commit hook（实时检测）
    |
    v
代码提交时自动检测变更范围，
匹配映射表，提醒未更新的文档
    |
第二层：doc-keeper Agent（定期审查）
    |
    v
每个功能完成后，doc-keeper 审查：
    - 代码与 spec 是否一致
    - 版本号是否已递增
    - 交叉引用是否正确
    |
第三层：CI Pipeline（自动化门禁）
    |
    v
CI 中检查文档完整性：
    - spec 版本号格式正确
    - 链接有效性
    - 术语一致性
```

### 3.2 Pre-commit Hook 实现

`.githooks/pre-commit` 中的文档同步检查：

```bash
#!/bin/bash
# 文档同步检查

CHANGED_FILES=$(git diff --cached --name-only)
WARNINGS=0

check_doc_sync() {
  local pattern="$1"
  local doc="$2"
  local label="$3"
  
  if echo "$CHANGED_FILES" | grep -q "$pattern"; then
    if ! echo "$CHANGED_FILES" | grep -q "$doc"; then
      echo "WARNING: $label changed but $doc not updated"
      WARNINGS=$((WARNINGS + 1))
    fi
  fi
}

# 映射检查
check_doc_sync "src/commands/"    "docs/specs/api-spec.md"      "Tauri commands"
check_doc_sync "storage.rs"       "docs/specs/database-spec.md" "Database schema"
check_doc_sync "src/stores/"      "docs/specs/frontend-spec.md" "Zustand store"
check_doc_sync "src/tools/"       "docs/specs/agent-spec.md"    "Agent tool"
check_doc_sync "cratebay-mcp/"    "docs/specs/mcp-spec.md"      "MCP server"
check_doc_sync "src/runtime/"     "docs/specs/runtime-spec.md"  "Runtime code"

if [ $WARNINGS -gt 0 ]; then
  echo ""
  echo "Found $WARNINGS documentation sync warning(s)."
  echo "Please update the corresponding docs before committing."
  echo "Use --no-verify to skip this check (not recommended)."
fi

# 警告但不阻塞提交（Agent 应主动处理）
exit 0
```

### 3.3 doc-keeper 审查流程

在测试打磨阶段，doc-keeper Agent 执行以下审查：

```
git diff <last-release>..HEAD --name-only
    |
    v
分析所有变更文件
    |
    v
逐项检查映射表
    |
    v
生成"文档更新报告"：
    - 已正确更新的文档 ✅
    - 需要补充更新的文档 ⚠️
    - 发现的不一致项 ❌
    |
    v
自动修复可修复项 → 手动标记需人工审核项
```

---

## 4. 版本号管理

### 4.1 语义化版本规则

每份 spec 文档顶部标注版本号，遵循 [SemVer](https://semver.org) 语义化版本：

```
MAJOR.MINOR.PATCH
  |     |     |
  |     |     +--- 修复/补充（无功能变化）
  |     +--------- 新增功能（向后兼容）
  +--------------- 破坏性变更（不兼容）
```

### 4.2 具体递增规则

| 变更类型 | 版本递增 | 示例场景 |
|---------|---------|---------|
| **Major** | x.0.0 | API 不兼容变更、schema 迁移、删除 API |
| **Minor** | x.y.0 | 新增 Tauri command、新增数据表、新增组件 |
| **Patch** | x.y.z | 修复文档错误、补充描述、更新示例 |

### 4.3 版本号格式

文档头部格式：

```markdown
# Document Title
> Version: 1.2.3 | Last Updated: 2026-03-20 | Author: <role>
```

中文文档：
```markdown
# 文档标题
> 版本: 1.2.3 | 更新日期: 2026-03-20 | 作者: <role>
```

### 4.4 版本同步

当多份文档因同一需求而更新时：

- 各文档独立管理版本号
- 在 `progress.md` 中记录"本次变更涉及的文档及版本变化"
- 不要求所有文档版本号一致

---

## 5. 文档质量检查

### 5.1 一致性检查

确保 spec 文档与实际代码一致：

| 检查项 | 检查方式 | 检查频率 |
|--------|---------|---------|
| API 签名一致性 | spec 中的函数签名 vs 实际 Rust 代码 | 每次 API 变更 |
| 数据模型一致性 | spec 中的表结构 vs 实际 migration | 每次 schema 变更 |
| 组件列表一致性 | spec 中的组件清单 vs 实际文件列表 | 每次组件新增/删除 |
| 工具目录一致性 | spec 中的 Tool 列表 vs 实际 Tool 定义 | 每次 Tool 变更 |
| 命令列表一致性 | AGENTS.md 命令速查 vs 实际可用命令 | 每次命令变更 |

### 5.2 链接有效性

文档中的内部链接和交叉引用必须有效：

```
检查所有 Markdown 链接：
    - [text](relative/path.md) → 文件是否存在
    - [text](path.md#anchor)   → 锚点是否存在
    - [text](../other/doc.md)  → 相对路径是否正确
```

CI 中可使用 `markdown-link-check` 工具自动验证。

### 5.3 术语一致性

所有文档中使用的技术术语必须与 `glossary.md` 一致：

| 规则 | 示例 |
|------|------|
| 使用标准术语名称 | "pi-agent-core" 而非 "agent core" |
| 首次出现时全称 | "Model Context Protocol (MCP)" |
| 后续使用缩写 | "MCP" |
| 产品名不翻译 | "CrateBay", "Streamdown", "Zustand" |
| 技术名不翻译 | "Tauri Commands", "bollard", "VZ.framework" |

### 5.4 格式规范检查

```
[ ] 文档顶部包含版本号和更新日期
[ ] 章节编号连续无跳跃
[ ] 表格对齐正确
[ ] 代码块标注语言类型
[ ] ASCII 流程图在等宽字体下对齐
[ ] 中文文档使用中文标点
[ ] 英文文档使用英文标点
[ ] 文件末尾有且仅有一个换行符
```

---

## 6. 知识库恢复流程

### 6.1 新 Agent 加入

```
新 Agent 加入项目
    |
    v
读取 AGENTS.md               → 了解项目全景
    |
    v
读取 .codebuddy/project.yaml  → 了解当前阶段和团队
    |
    v
读取 docs/progress.md         → 了解进度和断点
    |
    v
读取角色对应的 spec 文档       → 了解技术细节
    |
    v
读取 agent-team-workflow.md    → 了解协作规范
    |
    v
检查 TaskList                 → 领取任务
    |
    v
开始工作（无需人类交代上下文）
```

### 6.2 跨机器恢复

```
新电脑打开项目
    |
    v
git pull（获取最新代码和文档）
    |
    v
AI 读取 project.yaml → 识别项目身份和阶段
    |
    v
AI 读取 progress.md → 读取"下次继续"段落
    |
    v
从断点恢复：
    - 重建 TaskList
    - 确定活跃 Agent
    - 继续未完成的任务
```

### 6.3 知识库完整性验证

定期执行知识库完整性检查：

```
[ ] AGENTS.md 存在且链接有效
[ ] 所有 spec 文档存在（9 份）
[ ] 所有流程文档存在（3 份）
[ ] 所有参考文档存在（2 份）
[ ] docs/README.md 索引完整
[ ] docs/progress.md 状态最新
[ ] .codebuddy/project.yaml 配置正确
[ ] .mcp.json 配置有效
[ ] 软链接完好（.cursorrules, CLAUDE.md, copilot-instructions.md）
```
