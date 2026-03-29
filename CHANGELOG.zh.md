# 更新日志

本文件记录项目的所有重要变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，
版本遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.9.0] — 2026-03-29

将 CrateBay 重新定位为本地 AI 沙盒。MCP Server 核心工具完成。

### 新增
- `sandbox_run_code` MCP 工具 — 一键创建沙盒→写入代码→执行→返回结果→清理
- `sandbox_install` MCP 工具 — 通过 pip/npm/cargo/apt 安装依赖包
- `exec_with_timeout`、`exec_put_text`、`exec_get_file` 核心函数
- `image_load_from_tar` 离线镜像加载
- 打包镜像自动加载（应用启动时）
- CLI `cratebay mcp export` 命令（生成 Claude Desktop/Cursor 配置）
- 4 个 Dockerfile 和镜像构建脚本（python/node/rust/ubuntu）
- Getting Started 文档
- v1.0.0 发布路线图

### 变更
- 产品定位：从"Chat-First 控制面板"改为"本地 AI 沙盒"
- MCP Server 工具数：11 → 13（新增 run_code + install）
- 模板镜像更新为 slim 版本
- 版本号：2.0.0-alpha.1 → 0.9.0（v1.0.0 = ChatPage 补齐 + UI 优化）
- README 重写，突出 AI Sandbox 定位和竞品对比

## [2.0.0-alpha.1] — 2026-03-20

CrateBay 完全重写为本地 AI 沙盒。

### 新增

#### 项目基础（Step 0-1）
- Rust workspace（4 个 crate）：cratebay-core、cratebay-gui、cratebay-cli、cratebay-mcp
- 16 份技术规范文档
- AGENTS.md（兼容 20+ AI 编程工具）
- MIT 许可证

#### 核心基础设施（Step 2-3）
- SQLite 存储层（12 张表 + WAL 模式 + 迁移系统）
- Docker 集成（bollard SDK）
- LLM 代理（3 种 API 格式 + SSE 流式）
- 30 个 Tauri 命令
- 85 个集成测试

#### 前端（Step 4）
- React 19 + Vite 6.x + TypeScript
- Chat 界面 + 容器管理 + MCP 管理
- 6 个 Zustand Store + 17 个 Agent 工具 + i18n

#### 内置容器运行时（Step 5）
- macOS: VZ.framework VM
- Linux: KVM/QEMU VM
- Windows: WSL2

#### MCP Server + Client（Step 6）
- MCP Server：11 个沙盒工具 + 审计日志
- MCP Client：stdio/SSE 双传输
- 工具桥接：MCP 工具自动注入 Agent

#### 测试 & CI/CD（Step 7）
- 269 Rust 测试 + 197 前端测试 + 68 E2E 测试 + 24 安全测试
- 3 个 GitHub Actions 工作流（CI + Release + Pages）
