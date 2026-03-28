# CrateBay v2.0 → v2.1-Alpha 用户可用版本执行计划

> **目标**：从当前的"功能完整但无法使用"升级到"用户装好就能用"的第一个可用版本。
>
> **定位**：本地 AI Sandbox — 让任何 AI Agent 安全地在你的机器上运行代码。
>
> **交付物**：CrateBay v2.1-Alpha — Desktop App + MCP Server，用户可以通过 Claude Desktop / Cursor / Windsurf 给 AI 指示运行代码。
>
> **时间估计**：2-3 周的高效开发

---

## 执行阶段分解

### **Phase 1: MCP Server 核心工具扩展（P0 — 5-7 天）**

**目标**：让 MCP Server 暴露真正的"代码执行"接口，不只是"容器操作"。

#### P1.1 新增 `sandbox_run_code` 工具（2 天）

**现状**：MCP Server 有 `cratebay_sandbox_exec`（需要容器已存在）

**缺口**：没有"一键：创建沙盒 → 写入代码 → 执行 → 返回结果 → 清理"

**实现**：
```rust
// crates/cratebay-mcp/src/tools.rs 新增工具

cratebay_sandbox_run_code {
  language: "python" | "javascript" | "bash" | "rust",
  code: string,           // 代码文本
  timeout_seconds: u64?,  // 可选超时（默认 60s）
  environment: {
    key: string,
    value: string
  }[]?                    // 环境变量
}

returns {
  sandbox_id: string,
  exit_code: i32,
  stdout: string,
  stderr: string,
  duration_ms: u64
}
```

**工作流**：
1. 根据 `language` 选择模板（python-dev / node-dev / rust-dev / ubuntu-base）
2. 创建沙盒
3. 写入代码到 `~/run.py` (或 `.js`/`.rs`)
4. 执行 `python ~/run.py` (或对应语言)
5. 返回 exit_code + stdout + stderr
6. 可选：自动清理（或用户指定保留）

**关键点**：
- 用户可选是否清理沙盒（allow_cleanup flag）
- 返回 sandbox_id 便于后续操作（install deps, upload files 等）
- 完整的 error propagation

#### P1.2 增强 `sandbox_install` 工具（1 天）

**现状**：无此工具

**需求**：在沙盒内安装包（pip / npm / cargo）

```rust
cratebay_sandbox_install {
  sandbox_id: string,
  package_manager: "pip" | "npm" | "cargo" | "apt",
  packages: string[],  // ["numpy", "pandas"] 或 ["lodash", "express"]
  version?: string     // 可选版本约束
}

returns {
  sandbox_id: string,
  installed: string[],
  failed: {name: string, error: string}[]?,
  duration_ms: u64
}
```

**工作流**：
1. 在沙盒内执行 `pip install <packages>` / `npm install <packages>` 等
2. 追踪成功/失败的包
3. 返回详细结果

#### P1.3 新增 `sandbox_upload_file` / `sandbox_download_file` 工具简化 API（1.5 天）

**现状**：有 `sandbox_put_path` / `sandbox_get_path`（使用 base64 编码）

**问题**：base64 编码对大文件不友好，文件路径处理复杂

**改进**：
```rust
cratebay_sandbox_upload {
  sandbox_id: string,
  file_content: binary,    // base64 或二进制流
  target_path: string      // "/app/data.csv"
}

cratebay_sandbox_download {
  sandbox_id: string,
  source_path: string      // "/app/result.json"
}
returns {
  file_content: binary,
  size_bytes: u64
}
```

**工作流**：处理大文件时分块传输（可选第二阶段优化）

#### P1.4 MCP Server CLI 包装 (1 day)

**需求**：用户能通过命令行启动 MCP Server

```bash
cratebay-mcp start                    # 启动 MCP Server
cratebay-mcp status                   # 检查状态
cratebay-mcp test-tool sandbox_run_code --language python --code "print('hi')"
```

---

### **Phase 2: 离线容器镜像打包（P0 — 3-4 天）**

**目标**：用户装好 CrateBay，启动时自动有可用的沙盒镜像。

#### P2.1 制作"最小化"Sandbox 镜像（1.5 天）

**现状**：用户需要 `image_pull alpine` 或 `image_pull python:3.12`，但 VZ NAT 限制无法直接拉取

**需求**：打包一个离线的、优化大小的容器镜像

**方案**：

1. **Python Sandbox 镜像** (`python-sandbox:latest`)
   - 基础：Python 3.12 官方镜像
   - 大小：~200-300 MB
   - 包含：pip, venv, 常用科学包 (numpy/pandas/requests)
   - 生成方式：
     ```dockerfile
     FROM python:3.12-slim-bookworm
     RUN apt-get update && apt-get install -y curl wget git build-essential
     RUN pip install --no-cache numpy pandas requests
     WORKDIR /app
     ```

2. **Node.js Sandbox 镜像** (`node-sandbox:latest`)
   - 基础：Node.js 20 官方镜像
   - 大小：~150-200 MB
   - 包含：npm, yarn, common packages
   - 生成方式：
     ```dockerfile
     FROM node:20-bookworm
     RUN npm install -g pnpm yarn @types/node
     WORKDIR /app
     ```

3. **Rust Sandbox 镜像** (`rust-sandbox:latest`)
   - 基础：Rust 官方镜像
   - 大小：~400-500 MB
   - 包含：rustup, cargo, commonly used crates
   - 生成方式：
     ```dockerfile
     FROM rust:1-bookworm
     RUN rustup component add clippy
     WORKDIR /app
     ```

4. **通用 Sandbox 镜像** (`ubuntu-sandbox:latest`)
   - 基础：Ubuntu 24.04
   - 大小：~80-100 MB
   - 包含：基本工具 (curl, wget, git, build-essential)

**打包流程**：

```bash
# 1. 在开发机上构建所有镜像
docker build -t cratebay-python-sandbox:v1 -f images/python.Dockerfile .
docker build -t cratebay-node-sandbox:v1 -f images/node.Dockerfile .
docker build -t cratebay-rust-sandbox:v1 -f images/rust.Dockerfile .
docker build -t cratebay-ubuntu-sandbox:v1 -f images/ubuntu.Dockerfile .

# 2. 导出为 OCI tar
docker save cratebay-python-sandbox:v1 | gzip > crates/cratebay-gui/runtime-images/python.tar.gz
docker save cratebay-node-sandbox:v1 | gzip > crates/cratebay-gui/runtime-images/node.tar.gz
docker save cratebay-rust-sandbox:v1 | gzip > crates/cratebay-gui/runtime-images/rust.tar.gz
docker save cratebay-ubuntu-sandbox:v1 | gzip > crates/cratebay-gui/runtime-images/ubuntu.tar.gz

# 3. Tauri 打包时包含这些镜像
# 修改 src-tauri/tauri.conf.json 的 bundle.resources
```

**总大小**：~800-1200 MB (可以压缩到 ~300-400 MB with gzip)

#### P2.2 GUI 启动时自动导入镜像（1.5 day）

**现状**：用户启动 GUI，需要手动 `image pull`

**需求**：
1. 检测镜像是否存在
2. 如果不存在，自动从 runtime-images/ 导入
3. 显示进度，不阻塞 UI

**实现**：

```typescript
// crates/cratebay-gui/src/hooks/useImageBootstrap.ts (新建)

export function useImageBootstrap() {
  useEffect(() => {
    const bootstrap = async () => {
      const imagesToImport = [
        { name: 'cratebay-python-sandbox:v1', path: 'runtime-images/python.tar.gz' },
        { name: 'cratebay-node-sandbox:v1', path: 'runtime-images/node.tar.gz' },
        { name: 'cratebay-rust-sandbox:v1', path: 'runtime-images/rust.tar.gz' },
        { name: 'cratebay-ubuntu-sandbox:v1', path: 'runtime-images/ubuntu.tar.gz' },
      ];

      for (const image of imagesToImport) {
        const exists = await invoke<boolean>('image_exists', { name: image.name });
        if (!exists) {
          // 后台导入，显示进度
          await invoke('image_import_from_asset', { assetPath: image.path });
        }
      }
    };

    bootstrap().catch(console.error);
  }, []);
}

// App.tsx 中调用
useImageBootstrap();
```

**后端实现**：

```rust
// crates/cratebay-gui/src-tauri/src/commands/image.rs 新增命令

#[tauri::command]
pub async fn image_exists(state: State<'_, AppState>, name: String) -> Result<bool> {
  let docker = state.require_docker().await?;
  match docker.inspect_image(&name).await {
    Ok(_) => Ok(true),
    Err(bollard::errors::Error::DockerResponseServerError {..}) => Ok(false),
    Err(e) => Err(e.into()),
  }
}

#[tauri::command]
pub async fn image_import_from_asset(
  state: State<'_, AppState>,
  asset_path: String,
) -> Result<()> {
  let docker = state.require_docker().await?;
  
  // 1. 读取资源文件
  let asset_bytes = tauri::api::fs::read_binary(&asset_path).await?;
  
  // 2. 通过 docker load 导入
  let mut reader = std::io::Cursor::new(asset_bytes);
  docker.import_image(ImportImageOptions::default(), reader).await?;
  
  Ok(())
}
```

---

### **Phase 3: Desktop App 优化（P1 — 3 天）**

**目标**：让 ChatPage 和容器管理界面更好地展示"代码执行"工作流。

#### P3.1 ChatPage 为 Sandbox 工作流优化（1.5 days）

**现状**：ChatPage 是通用的容器管理助手

**改进**：
1. 新增 "Sandbox" Tab（vs. 当前的通用 Chat）
2. 系统提示优化，强调"代码执行"
3. 常用操作快捷按钮（"Run Python code", "Install packages", etc.)

```typescript
// crates/cratebay-gui/src/pages/SandboxPage.tsx (新建)

export function SandboxPage() {
  // 专门为代码执行优化的 Chat 页面
  // - 系统提示强调代码执行
  // - Tool 列表中突出 sandbox_run_code / sandbox_install / sandbox_upload
  // - 预置常用的代码片段模板
}
```

#### P3.2 Sandbox Dashboard（1 day）

**新增页面**：简化版的沙盒管理面板

```
┌─────────────────────────────────────────┐
│  Running Sandboxes                      │
├─────────────────────────────────────────┤
│  🟢 python-dev-001  (Python 3.12)       │
│     CPU: 2 cores | Memory: 2 GB         │
│     Created: 2 mins ago                 │
│     [Shell] [Upload] [Stop] [Delete]    │
│                                         │
│  🟢 node-dev-002  (Node.js 20)          │
│     CPU: 2 cores | Memory: 1 GB         │
│     Created: 5 mins ago                 │
│     [Shell] [Upload] [Stop] [Delete]    │
└─────────────────────────────────────────┘
```

#### P3.3 MCP 配置导出与一键连接（1 day）

**功能**：帮助用户快速连接 Claude Desktop / Cursor / Windsurf

**实现**：

1. **自动生成 MCP 配置**
   - 检测用户系统中的 Claude Desktop / Cursor / Windsurf 配置文件
   - 生成 `cratebay-mcp` 服务器配置
   - 显示"Copy to Clipboard"按钮

2. **一键导出**
   ```typescript
   // CrateBay GUI 中的按钮
   "Export MCP Config for Claude Desktop" 
   → 复制到剪贴板 → 用户手动粘贴到 claude_desktop_config.json
   ```

3. **验证连接**
   ```bash
   # 提供测试脚本
   cratebay-mcp test-connection
   ```

---

### **Phase 4: 文档与示例（P1 — 2-3 天）**

**目标**：用户知道如何使用。

#### P4.1 Getting Started 文档（1 day）

```markdown
# Getting Started with CrateBay

## 1. Install
- Download from releases (macOS / Linux / Windows)
- Or: `brew install --cask cratebay`

## 2. Verify Installation
- Open CrateBay app
- Go to Settings > Runtime
- Verify status shows "Ready"

## 3. Connect Claude Desktop
- Copy MCP config from CrateBay Settings
- Paste into Claude Desktop config
- Restart Claude Desktop
- Verify "CrateBay" appears in Claude's Tools

## 4. Run Your First Code
In Claude Desktop:
> "Create a Python sandbox and print hello world"

Expected output:
- Claude creates a Python sandbox
- Executes `print("Hello CrateBay")`
- Returns: `Hello CrateBay`
```

#### P4.2 例子与示例（1 day）

```markdown
# Examples

## Example 1: Data Analysis
User: "Analyze this CSV file with pandas"
Claude:
1. Creates python-dev sandbox
2. Uploads CSV file
3. Writes Python script for analysis
4. Downloads result.json
5. Summarizes findings

## Example 2: Web Server
User: "Create a simple FastAPI server"
Claude:
1. Creates node-dev sandbox
2. Installs express
3. Writes server code
4. Runs it
5. Returns localhost URL

## Example 3: File Processing
User: "Convert this image to grayscale using PIL"
Claude:
1. Creates python-dev sandbox
2. Installs Pillow
3. Uploads image
4. Processes with PIL
5. Downloads result
```

#### P4.3 故障排除指南（0.5 day）

```markdown
# Troubleshooting

## Runtime won't start
- Check if virtualization is enabled in BIOS
- Try: Settings > Runtime > Start Runtime (manual)

## MCP connection fails
- Verify Claude Desktop installed
- Check MCP config path
- Run: `cratebay-mcp test-connection`

## Image import fails
- Check disk space (need ~1 GB)
- Verify Docker socket connectivity
```

---

### **Phase 5: 质量保证与发布（P0 — 4-5 days）**

#### P5.1 端到端测试（2 days）

**验证链路**：

```
[A] Claude Desktop → MCP request → cratebay-mcp → Docker
    ├─ sandbox_run_code("python", "print('hi')")
    └─ verify stdout = "hi"

[B] Claude Desktop → sandbox_install("pip", ["numpy"])
    └─ verify numpy available in next exec

[C] Upload file → Run analysis → Download result
    └─ verify file transformation

[D] Multiple concurrent sandboxes
    └─ verify isolation & resource limits

[E] macOS / Linux / Windows (each platform)
    └─ verify runtime starts and Docker ready
```

**自动化测试**：

```rust
// tests/e2e_sandbox.rs (新建)

#[tokio::test]
async fn test_sandbox_run_code_python() {
  let client = McpClient::connect_stdio("cratebay-mcp");
  
  let result = client.call_tool("cratebay_sandbox_run_code", json!({
    "language": "python",
    "code": "print('hello')",
  })).await.unwrap();
  
  assert_eq!(result["stdout"], "hello\n");
  assert_eq!(result["exit_code"], 0);
}
```

#### P5.2 性能验证（1 day）

**指标**：

| 操作 | 目标 | 测试方法 |
|-----|------|---------|
| 启动 sandbox | < 10s | `time cratebay sandbox create` |
| 执行代码 | < 5s (Python) | `time sandbox_run_code(...)` |
| 安装包 | < 30s (pip install numpy) | `time sandbox_install(...)` |
| 文件上传 | < 2s (10 MB) | `time sandbox_upload(...)` |
| 内存占用 | < 2 GB (idle) | `free -h` |
| 磁盘占用 | < 5 GB (all images) | `du -sh ~/.cratebay/` |

#### P5.3 跨平台验证（1 day）

**平台清单**：
- macOS (Intel + Apple Silicon)
- Ubuntu 22.04 LTS
- Windows 11 (WSL2)

**验证项**：
- ✅ 安装成功
- ✅ Runtime 启动
- ✅ Docker 连接
- ✅ MCP Server 启动
- ✅ 容器创建/执行/删除
- ✅ 镜像导入
- ✅ GUI 功能完整

#### P5.4 文档完成度检查（0.5 day）

**清单**：
- ✅ README 更新（已完成）
- ✅ Getting Started 文档
- ✅ API 文档（MCP 工具参数）
- ✅ 故障排除指南
- ✅ Contributing 指南更新

#### P5.5 发布准备（1 day）

**检查清单**：
- ✅ CHANGELOG.md 更新
- ✅ Version bump (2.0.0 → 2.1.0-alpha)
- ✅ Git tag `v2.1.0-alpha`
- ✅ GitHub Release 创建
- ✅ 签名和附件上传 (macOS/Linux/Windows)
- ✅ 官网更新

---

## 时间总览

| Phase | 任务 | 天数 | 优先级 |
|-------|------|------|-------|
| **P1** | MCP Server 工具扩展 | 5-7 | 🔴 P0 |
| **P2** | 离线镜像打包 | 3-4 | 🔴 P0 |
| **P3** | Desktop App 优化 | 3 | 🟡 P1 |
| **P4** | 文档与示例 | 2-3 | 🟡 P1 |
| **P5** | QA 与发布 | 4-5 | 🔴 P0 |
| | **总计** | **18-22 天** | |

**建议周期**：2-3 周密集开发（如果配置 dev team）

---

## 成功标准

用户第一次使用时：

1. ✅ 装好 CrateBay Desktop App（一键安装）
2. ✅ 启动时自动导入 Python/Node.js/Rust 镜像（无需手动 pull）
3. ✅ 配置 Claude Desktop 连接 CrateBay MCP（copy-paste，5 分钟）
4. ✅ 在 Claude 中说 "Run this Python code: print('hello')"
5. ✅ Claude 通过 MCP 调用 `sandbox_run_code`
6. ✅ CrateBay 创建沙盒、执行代码、返回结果
7. ✅ 用户看到输出 `hello`

**这整个流程 ≤ 10 分钟，用户无需理解 Docker / MCP / 容器概念**。

---

## 风险与缓解

| 风险 | 影响 | 缓解方案 |
|-----|------|----------|
| 离线镜像太大 (>1GB) | 装不上或装得很慢 | 制作最小化镜像，提供 streaming download |
| MCP Server 不稳定 | Claude 连接断开 | 添加 heartbeat + reconnection logic |
| 镜像导入失败 | 用户没有可用沙盒 | Fallback 到 online pull (via proxy) |
| 跨平台兼容性 | Windows/Linux 验收失败 | 提前测试，准备平台特定修复 |

---

## 后续迭代（v2.2+）

- [ ] 流式日志输出（real-time stdout/stderr）
- [ ] Web IDE 集成（VS Code 风格的编辑器）
- [ ] Ollama 本地 LLM 支持
- [ ] gRPC daemon 支持远程 sandbox 管理
- [ ] Jupyter Notebook 集成
- [ ] 性能分析与监控面板
