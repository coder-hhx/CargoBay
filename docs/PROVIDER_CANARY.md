# Controlled Provider Canaries

CrateBay uses **controlled provider canaries** to prove that the AI provider + CLI bridge surfaces work against *real* dependencies, without turning the main CI gate into a secrets-dependent job.

These canaries are intentionally small, low-cost, and should only be run with locked-down test credentials.

## What It Validates

- **OpenAI / Anthropic**: `ai_test_connection` with a minimal prompt (`PONG`).
- **Codex / Claude CLI bridges**: runs the built-in presets via `agent_cli_run`.
- **Ollama daemon** (separate smoke): real daemon status + list models (+ optional pull/delete).

## Local Run

### Provider + CLI canaries

```bash
export CRATEBAY_CANARY_OPENAI_API_KEY="..."
export CRATEBAY_CANARY_ANTHROPIC_API_KEY="..."

# Optional overrides
export CRATEBAY_CANARY_OPENAI_BASE_URL="https://api.openai.com/v1"
export CRATEBAY_CANARY_OPENAI_MODEL="gpt-4.1-mini"
export CRATEBAY_CANARY_OPENAI_TIMEOUT_SEC="25"
export CRATEBAY_CANARY_ANTHROPIC_BASE_URL="https://api.anthropic.com/v1"
export CRATEBAY_CANARY_ANTHROPIC_MODEL="claude-3-7-sonnet-latest"
export CRATEBAY_CANARY_ANTHROPIC_TIMEOUT_SEC="25"

# CLI bridges: can be an absolute path or a command available in PATH
export CRATEBAY_CANARY_CODEX_BIN="/path/to/codex"
export CRATEBAY_CANARY_CLAUDE_BIN="/path/to/claude"
export CRATEBAY_CANARY_CODEX_PROMPT="Reply with PONG and exit."
export CRATEBAY_CANARY_CLAUDE_PROMPT="Reply with PONG and exit."
export CRATEBAY_CANARY_CODEX_TIMEOUT_SEC="60"
export CRATEBAY_CANARY_CLAUDE_TIMEOUT_SEC="60"

./scripts/provider-canary-smoke.sh
```

This script runs the ignored tests under:
- `crates/cratebay-gui/src-tauri/src/lib.rs` (`mod ai_runtime_tests`)

and writes artifacts to:
- `dist/provider-canary/`

### Ollama daemon canary

```bash
# Optional: can be an absolute path or a command available in PATH
export CRATEBAY_CANARY_OLLAMA_BIN="/path/to/ollama"

# Optional overrides
export CRATEBAY_CANARY_OLLAMA_BASE_URL="http://127.0.0.1:11434/v1"
export CRATEBAY_CANARY_OLLAMA_MODELS_DIR="/path/to/ollama/models"
export CRATEBAY_CANARY_OLLAMA_EXPECT_MODEL="qwen2.5:7b"
export CRATEBAY_CANARY_OLLAMA_PULL_MODEL="tiny-model:latest"

./scripts/ollama-daemon-smoke.sh
```

Artifacts:
- `dist/ollama-daemon-smoke/`

## GitHub Actions (Self-hosted)

See:
- `.github/workflows/provider-canary.yml`

Notes:
- These jobs are intended for **self-hosted** Linux runners with explicit labels.
- Secrets are provided via GitHub Actions secrets/vars and should be locked down.

