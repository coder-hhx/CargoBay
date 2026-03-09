# 受控 Provider Canary

CrateBay 用 **受控 provider canary** 来证明 AI provider / CLI bridge 等关键面向真实依赖的路径是可用的，同时避免把主 CI 门禁变成“必须注入 secrets 才能跑”的任务。

这些 canary 的原则：
- Prompt 尽量短（例如 `PONG`）
- Timeout 尽量短
- 只使用锁定权限的测试凭据

## 验证内容

- **OpenAI / Anthropic**：通过 `ai_test_connection` 做最小化连通性验证（`PONG`）。
- **Codex / Claude CLI bridges**：通过内置 preset 的 `agent_cli_run` 跑一次只读/最小 prompt。
- **Ollama daemon**（单独 smoke）：检查真实 daemon 状态 + 列模型（可选 pull/delete）。

## 本地运行

### Provider + CLI canary

```bash
export CRATEBAY_CANARY_OPENAI_API_KEY="..."
export CRATEBAY_CANARY_ANTHROPIC_API_KEY="..."

# 可选覆盖项
export CRATEBAY_CANARY_OPENAI_BASE_URL="https://api.openai.com/v1"
export CRATEBAY_CANARY_OPENAI_MODEL="gpt-4.1-mini"
export CRATEBAY_CANARY_OPENAI_TIMEOUT_SEC="25"
export CRATEBAY_CANARY_ANTHROPIC_BASE_URL="https://api.anthropic.com/v1"
export CRATEBAY_CANARY_ANTHROPIC_MODEL="claude-3-7-sonnet-latest"
export CRATEBAY_CANARY_ANTHROPIC_TIMEOUT_SEC="25"

# CLI bridge：可以是绝对路径，也可以是 PATH 中可执行命令名
export CRATEBAY_CANARY_CODEX_BIN="/path/to/codex"
export CRATEBAY_CANARY_CLAUDE_BIN="/path/to/claude"
export CRATEBAY_CANARY_CODEX_PROMPT="Reply with PONG and exit."
export CRATEBAY_CANARY_CLAUDE_PROMPT="Reply with PONG and exit."
export CRATEBAY_CANARY_CODEX_TIMEOUT_SEC="60"
export CRATEBAY_CANARY_CLAUDE_TIMEOUT_SEC="60"

./scripts/provider-canary-smoke.sh
```

该脚本会运行以下 ignored tests：
- `crates/cratebay-gui/src-tauri/src/lib.rs`（`mod ai_runtime_tests`）

产物输出到：
- `dist/provider-canary/`

提示：`./scripts/release-readiness.sh` 也会在你配置了凭据/二进制时自动跑这些 canary（以及 Ollama daemon smoke）；未配置时会安全跳过。

### Ollama daemon canary

```bash
# 可选：可以是绝对路径，也可以是 PATH 中可执行命令名
export CRATEBAY_CANARY_OLLAMA_BIN="/path/to/ollama"

# 可选覆盖项
export CRATEBAY_CANARY_OLLAMA_BASE_URL="http://127.0.0.1:11434/v1"
export CRATEBAY_CANARY_OLLAMA_MODELS_DIR="/path/to/ollama/models"
export CRATEBAY_CANARY_OLLAMA_EXPECT_MODEL="qwen2.5:7b"
export CRATEBAY_CANARY_OLLAMA_PULL_MODEL="tiny-model:latest"

./scripts/ollama-daemon-smoke.sh
```

产物输出到：
- `dist/ollama-daemon-smoke/`

## GitHub Actions（自建 runner）

参考：
- `.github/workflows/provider-canary.yml`

说明：
- 该 workflow 预期运行在 **self-hosted Linux runner** 上，并使用明确的 labels 做隔离。
- secrets/vars 通过 GitHub Actions 注入，务必限制权限与可见范围。
