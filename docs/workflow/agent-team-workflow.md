# Agent Team 协作规范

> 版本: 1.0.0 | 更新日期: 2026-03-20 | 作者: product-manager

---

## 1. 动态组队策略

CrateBay 采用动态组队机制，根据项目阶段自动调整团队规模和角色配置。人类只做决策，AI Agent 负责全部执行工作。

### 1.1 三阶段团队配置

#### 阶段一：文档阶段（3 人）

```
人类（决策者）
    |  审核文档
    v
+-------------------+     +---------------------+     +-------------------------+
|    architect       |     |  product-manager     |     |  frontend-architect     |
+-------------------+     +---------------------+     +-------------------------+
| architecture.md   |     | dev-workflow.md      |     | frontend-spec.md        |
| backend-spec.md   |     | agent-team-workflow.md|    | agent-spec.md           |
| runtime-spec.md   |     | knowledge-base.md    |     | mcp-spec.md             |
| api-spec.md       |     | tech-decisions.md    |     | testing-spec.md         |
| database-spec.md  |     | glossary.md          |     | progress.md             |
|                   |     | docs/README.md       |     |                         |
+-------------------+     +---------------------+     +-------------------------+
       5 份                       6 份                         5 份
```

**目标产出**：16 份完整的技术文档体系

#### 阶段二：开发阶段（6 人）

```
人类（决策者）
    |  需求描述 + spec 审核 + 验收
    v
+------------------+
|   team-lead      |  协调者，兼架构把关
+------------------+
    |
    +---→ frontend-dev    ← frontend-spec, api-spec
    |         → React / TS / shadcn / Streamdown / Zustand
    |
    +---→ backend-dev     ← backend-spec, database-spec, api-spec, mcp-spec
    |         → Rust / Tauri / Docker / SQLite / LLM Proxy
    |
    +---→ ai-engineer     ← agent-spec, api-spec, frontend-spec
    |         → pi-agent-core / Tool 定义 / Agent 行为调优
    |
    +---→ runtime-dev     ← runtime-spec, backend-spec
    |         → macOS VZ / Linux KVM / Windows WSL2
    |
    +---→ tester          ← testing-spec, api-spec
              → Vitest / Playwright / cargo test / CI
```

| 角色 | 读取的 spec | 主要产出 |
|------|------------|---------|
| team-lead | 全部 | 任务分解、进度跟踪、冲突解决、架构审查 |
| frontend-dev | frontend-spec, api-spec | 前端代码、UI 组件、Zustand Store |
| backend-dev | backend-spec, database-spec, api-spec, mcp-spec | Rust 代码、Tauri commands、MCP Server |
| ai-engineer | agent-spec, api-spec, frontend-spec | Agent 集成、Tool 实现、LLM Proxy |
| runtime-dev | runtime-spec, backend-spec | 平台运行时代码 (VZ/KVM/WSL2) |
| tester | testing-spec, api-spec | 测试代码、CI 配置、覆盖率报告 |

#### 阶段三：测试打磨阶段（5 人）

```
+------------------+
|   team-lead      |  最终审查、release 管理
+------------------+
    |
    +---→ frontend-dev    → UI 问题修复、性能优化
    |
    +---→ backend-dev     → 后端问题修复、性能优化
    |
    +---→ tester          → 全量测试、覆盖率门禁、安全测试
    |
    +---→ doc-keeper      → 最终文档一致性检查、CHANGELOG
```

### 1.2 阶段切换条件

| 切换 | 条件 | 检查点 |
|------|------|--------|
| 文档 → 开发 | 16 份文档全部完成 + 人类审核通过 | 人类确认 |
| 开发 → 测试 | 核心功能开发完成 + CI 基本通过 | team-lead 汇报 + 人类确认 |
| 测试 → 发布 | 测试覆盖率达标 + 无 P0/P1 bug + 人类最终验收 | 人类验收 |

### 1.3 阶段切换流程

```
team-lead 评估阶段完成度
    |
    v
向人类汇报完成情况
    |
    v
人类决定是否进入下一阶段
    |
    +--→ 通过 → team-lead 更新 .codebuddy/project.yaml
    |              → 更新 docs/progress.md
    |              → 新阶段 Agent 自动加入
    |
    +--→ 不通过 → 继续当前阶段，补充缺失项
```

---

## 2. 人类介入模式（适度介入）

CrateBay 采用"适度介入"模式：人类把控方向和质量，AI 负责全部执行。

### 2.1 必须介入

| 场景 | 介入方式 | 阻塞开发 |
|------|---------|---------|
| **需求描述** | 人类用自然语言描述功能需求 | 是 -- 需求是开发的起点 |
| **Spec 变更审核** | 人类审核 spec 的破坏性变更 | 是 -- 通过后才能编码 |
| **最终验收** | 人类确认交付物符合需求 | 是 -- 通过后才能标记完成 |

### 2.2 可选介入

| 场景 | 默认行为 | 人类何时介入 |
|------|---------|-------------|
| **UI 效果确认** | AI 先自评 + 截图，确认符合 spec | 人类对视觉效果不满意时 |
| **依赖升级评估** | AI 评估兼容性，无问题自动升级 | 升级有风险或 breaking change 时 |

### 2.3 完全自动

以下环节完全由 AI Agent 执行，无需人类参与：

- 代码开发（读 spec → 写代码）
- 测试编写与执行
- 文档读取与更新
- 代码审查（Agent 交叉审查）
- Bug 修复（测试失败 → 自动修复 → 重新测试）
- 依赖版本管理（minor/patch 级别）

### 2.4 人类随时插入

人类可以在任何环节随时插入干预，不必等到流程末尾：

```
开发进行中...
    |
    +--→ [人类] "停一下，这个方案需要改" → Agent 暂停当前任务
    |       → 人类给出新指示 → Agent 调整方案继续
    |
    +--→ [人类] "这个功能不需要了" → Agent 标记任务取消
    |       → 回滚相关代码（如已提交）
    |
    +--→ [人类] "优先做另一个功能" → team-lead 调整任务优先级
            → Agent 切换任务
```

---

## 3. 全自动开发流程

### 3.1 完整流程图

```
人类描述需求
    |
    v
team-lead 分析需求
    |
    v
更新相关 spec 草案
    |
    v
[人类审核 spec]  ←---- 必须介入点
    |
    v
team-lead 分解任务（TaskCreate）
    |
    v
标注任务依赖（blockedBy）
    |
    v
分配给各 Agent
    |
    +--------+--------+--------+--------+
    |        |        |        |        |
    v        v        v        v        v
frontend  backend  ai-eng  runtime  tester
  -dev      -dev              -dev
    |        |        |        |        |
    v        v        v        v        v
读 spec   读 spec   读 spec  读 spec  准备
写代码    写代码    写代码   写代码   测试
写测试    写测试    写测试   写测试   方案
    |        |        |        |        |
    +--------+--------+--------+--------+
                     |
                     v
           tester 运行测试
                     |
              +------+------+
              |             |
           通过          失败
              |             |
              v             v
         继续流程     开发 Agent
                     自动修复
                        |
                        v
                    重新测试
                     |
                     v
    team-lead / doc-keeper 更新文档
                     |
                     v
          team-lead 交叉审查
                     |
                     v
            汇总交付物
                     |
                     v
          [人类验收]  ←---- 必须介入点
```

### 3.2 流程时序

| 步骤 | 执行者 | 动作 | 产出 |
|------|--------|------|------|
| 1 | 人类 | 描述需求 | 需求文本 |
| 2 | team-lead | 分析需求、更新 spec | spec 草案 |
| 3 | 人类 | 审核 spec（破坏性变更） | 审核通过/打回 |
| 4 | team-lead | 分解任务、标注依赖 | TaskList |
| 5 | 各 Agent | 并行开发（读 spec → 写代码 → 写测试） | 代码 + 测试 |
| 6 | tester | 运行测试 | 测试报告 |
| 7 | 开发 Agent | 修复失败测试（如有） | 修复代码 |
| 8 | team-lead | 更新文档 + 交叉审查 | 更新后的 docs |
| 9 | 人类 | 验收 | 通过/打回 |

---

## 4. 任务自动拆解与并行执行

### 4.1 任务拆解原则

team-lead 根据需求将工作拆解为子任务，遵循以下原则：

1. **原子性** -- 每个任务可以由单个 Agent 独立完成
2. **明确性** -- 任务描述包含具体的输入、输出和验收标准
3. **可追踪** -- 每个任务有唯一 ID，状态可查询
4. **依赖最小化** -- 尽量减少任务间的依赖关系

### 4.2 任务创建与依赖

```
team-lead 分析需求后：

TaskCreate: "实现 container_create Tauri command"
    owner: backend-dev
    blockedBy: []                    ← 无依赖，可立即开始

TaskCreate: "实现 container_list Tauri command"
    owner: backend-dev
    blockedBy: []                    ← 无依赖，可立即开始

TaskCreate: "实现 ContainerList 前端组件"
    owner: frontend-dev
    blockedBy: [container_list]      ← 依赖后端 API

TaskCreate: "编写容器 API 集成测试"
    owner: tester
    blockedBy: [container_create, container_list]  ← 依赖两个 API
```

### 4.3 并行执行策略

```
                时间轴 →
                
backend-dev:  [container_create] [container_list] [container_exec]
frontend-dev: [ChatInput 组件]   [ContainerList]  [ContainerDetail]
ai-engineer:  [Agent setup]      [Tool 定义]      [LLM Proxy]
runtime-dev:  [macOS VZ 适配]    [Linux KVM]      [Windows WSL2]
tester:       [测试框架搭建]      [单元测试]        [集成测试]

                ↑                  ↑                ↑
              无依赖              部分依赖          链式依赖
              全并行              交错执行          顺序执行
```

### 4.4 自动 Claim 机制

Agent 完成当前任务后自动领取下一个可用任务：

```
Agent 完成任务 A
    |
    v
TaskUpdate: A → completed
    |
    v
检查 TaskList：
    - 过滤 owner 为空的任务
    - 过滤 blockedBy 已全部 completed 的任务
    - 按优先级排序
    |
    v
TaskUpdate: 领取优先级最高的任务
    |
    v
开始执行
```

---

## 5. 代码审查流程

### 5.1 交叉审查机制

Agent 之间进行交叉审查，确保代码质量：

| 代码提交者 | 审查者 | 审查重点 |
|-----------|--------|---------|
| frontend-dev | backend-dev | API 接口调用是否正确、类型是否匹配 |
| backend-dev | frontend-dev | 返回数据结构是否满足前端需求 |
| ai-engineer | frontend-dev | Tool 定义是否与 UI 交互一致 |
| runtime-dev | backend-dev | 平台适配代码是否符合后端规范 |
| 任何 Agent | tester | 测试覆盖是否充分 |

### 5.2 审查检查清单

#### 通用检查

```
[ ] 代码符合对应 spec 的规范
[ ] 错误处理完整（无 unwrap/panic/any）
[ ] 命名规范一致
[ ] 无未使用的导入或变量
[ ] Commit message 符合 Conventional Commits
[ ] 相关测试已编写并通过
```

#### shadcn/ui 审查检查清单

```
[ ] 使用 shadcn/ui 组件而非手写 HTML
[ ] 遵循 Radix 原语的无障碍性要求
[ ] 使用 Tailwind CSS 类而非内联样式
[ ] 颜色使用 CSS 变量（支持暗黑模式）
[ ] 组件 props 使用 TypeScript 严格类型
[ ] 响应式布局正确（≥1400px / <1400px / <1100px）
```

#### Rust 审查检查清单

```
[ ] 使用 thiserror + Result<T, AppError> 处理错误
[ ] Mutex 使用 lock_or_recover()
[ ] 平台代码使用 #[cfg(target_os)] 门控
[ ] 无同步阻塞出现在异步上下文中
[ ] 生产代码中无 unwrap() 调用
```

### 5.3 审查流程

```
Agent A 提交代码
    |
    v
Agent B 执行审查（读代码 + 检查清单）
    |
    +--→ 通过 → 合并
    |
    +--→ 有问题 → 发送审查意见给 Agent A
                    → Agent A 修复
                    → 重新审查
```

**人类审批**仅限于以下场景：
- 关键架构变更
- 新增外部依赖
- 安全相关代码

---

## 6. 文档同步检查

### 6.1 检查时机

每个功能完成时，在代码提交前执行文档同步检查：

```
功能代码编写完成
    |
    v
运行文档同步检查：
    1. git diff --cached --name-only 获取变更文件列表
    2. 根据映射表判断需要更新的文档
    3. 检查对应文档是否已更新
    |
    +--→ 文档已同步 → 正常提交
    |
    +--→ 文档未同步 → 提醒 Agent 补充更新
```

### 6.2 映射规则

详见 [knowledge-base.md](knowledge-base.md) 中的"更新触发条件映射表"。

### 6.3 pre-commit Hook

`.githooks/pre-commit` 包含自动检测逻辑，在代码变更但文档未更新时发出警告。详细实现见 [dev-workflow.md](dev-workflow.md) 第 7 节。

---

## 7. 知识库自动更新协议

### 7.1 更新时机

功能开发完成后，由 doc-keeper（测试阶段）或 team-lead（开发阶段）执行知识库更新。

### 7.2 更新流程

```
功能开发完成
    |
    v
doc-keeper / team-lead 自动执行：
    |
    1. 检测本次变更涉及的文件（git diff）
    |
    2. 根据映射表确定需要更新的文档
    |
    3. 读取当前 spec 文档
    |
    4. 读取变更的源代码
    |
    5. 更新 spec 中受影响的章节
    |
    6. 递增 spec 版本号（patch 或 minor）
    |
    7. 更新 progress.md（标记完成）
    |
    8. 如果 crate 结构变化 → 更新 AGENTS.md
    |
    9. 如果用户可见变化 → 更新 website/
    |
   10. 输出更新摘要
```

### 7.3 完成需求后的检查清单

每次完成一个需求后，执行以下检查（不可省略）：

```
[ ] spec 文档已更新并递增版本号
[ ] progress.md 已标记为完成
[ ] api-spec.md 与实际 Tauri commands 一致
[ ] database-spec.md 与实际 schema 一致
[ ] frontend-spec.md 与实际组件/Store 一致
[ ] agent-spec.md 与实际 Tool 定义一致
[ ] AGENTS.md 仓库结构无变化（或已更新）
[ ] 测试全部通过
[ ] Commit message 符合规范
```

---

## 8. Agent 启动协议

每个 AI Agent 加入项目时，必须执行以下初始化步骤：

```
1. 读取 AGENTS.md              → 项目全景和导航
    |
2. 读取 .codebuddy/project.yaml → 当前阶段、团队配置
    |
3. 读取 docs/progress.md       → 进度状态、下次继续的断点
    |
4. 读取角色对应的 spec 文档     → 技术规范
    |
5. 读取 agent-team-workflow.md  → 协作规范（本文档）
    |
6. 检查 TaskList               → 领取或创建任务
    |
7. 开始工作
```

### 跨机器恢复（Quick Resume）

当在新电脑上打开项目时：

```
git pull
    |
    v
AI 读取 AGENTS.md + project.yaml + progress.md
    |
    v
根据 progress.md 的"下次继续"段落：
    - 恢复 TaskList
    - 确定活跃 Agent
    - 从断点继续工作
    |
    v
无需人类重新交代上下文
```

---

## 9. 沟通规范

### 9.1 Agent 间沟通

- **直接沟通** -- Agent 之间可以直接发送消息，无需经过 team-lead 中转
- **实质性沟通** -- 只在有实际信息需要传递时发消息，避免空洞的状态更新
- **结果导向** -- 完成任务后向 team-lead 发送完整结果，而非仅状态通知

### 9.2 向人类汇报

- team-lead 负责向人类汇报整体进度
- 汇报内容：完成了什么、进行中的工作、阻塞问题、需要人类决策的事项
- 避免技术细节过载，聚焦于人类关心的进展和决策点

### 9.3 冲突解决

```
Agent A 和 Agent B 对实现方案有分歧
    |
    v
先查看 spec 文档，spec 有明确规定 → 按 spec 执行
    |
    v
spec 未明确 → team-lead 裁决
    |
    v
涉及架构决策 → 记录 ADR + 人类审核
```
