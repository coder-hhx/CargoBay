# Tauri MCP Automation

This runbook establishes a repeatable local smoke path for driving the real CrateBay desktop shell through the Tauri MCP bridge in debug builds.

It complements `scripts/run-desktop-e2e-linux.sh`; it does not replace the Linux `tauri-driver` CI gate.

## Scope

- Start an isolated debug app with a fixed MCP bridge endpoint.
- Connect Codex or another MCP-capable client to the running desktop app.
- Drive a deterministic MCP registry smoke in the real shell: create entry, save, start, inspect logs, stop.
- Keep artifacts under `dist/tauri-mcp/` for failures.

## Prerequisites

- macOS or Linux development machine
- Rust toolchain
- Node.js 22+
- `python3`
- Codex with the Tauri MCP tools enabled

## App Lifecycle

Use the helper script from the repo root:

```bash
./scripts/tauri-mcp-app.sh start
./scripts/tauri-mcp-app.sh status
./scripts/tauri-mcp-app.sh logs
./scripts/tauri-mcp-app.sh stop
```

By default the script uses:

- MCP bind: `127.0.0.1`
- MCP port: `9223`
- app launch cwd: `${TMPDIR:-/tmp}`
- config/data/log dirs: `dist/tauri-mcp/{config,data,logs}`

Useful overrides:

- `CRATEBAY_MCP_PORT=9333 ./scripts/tauri-mcp-app.sh start`
- `CRATEBAY_MCP_LAUNCH_CWD=/tmp ./scripts/tauri-mcp-app.sh start`
- `CRATEBAY_MCP_SKIP_ASSET_PREP=1 ./scripts/tauri-mcp-app.sh start`
- `CRATEBAY_MCP_SKIP_NPM_CI=1 ./scripts/tauri-mcp-app.sh start`
- `CRATEBAY_MCP_SKIP_BUILD=1 ./scripts/tauri-mcp-app.sh start`

Use the skip flags together when you only want to reconnect to an existing debug build.

## Launch Context Caveat

On the current macOS machine, launching the debug binary from the repository working directory can make the process exit almost immediately.

Re-testing on 2026-03-18 showed:

- launching from the repo context can exit in under one second
- launching the same binary from `/tmp` keeps the process alive and the MCP port listening

The helper script now defaults to a clean launch cwd for that reason.

## Recommended Smoke Flow

1. Start the debug app with `./scripts/tauri-mcp-app.sh start`.
2. Connect the Tauri MCP client to port `9223`.
3. Confirm backend connectivity first:
   - `driver_session start`
   - `ipc_get_backend_state`
4. Drive this stable MCP path:
   - `[data-testid='nav-ai']`
   - `[data-testid='aihub-tab-mcp']`
   - `[data-testid='mcp-add-server']`
   - `[data-testid='mcp-input-id']`
   - `[data-testid='mcp-input-command']`
   - `[data-testid='mcp-input-args']`
   - `[data-testid='mcp-input-working-dir']`
   - `[data-testid='mcp-save-registry']`
   - `[data-testid='mcp-selected-status']`
   - `[data-testid='mcp-logs-output']`
5. Prefer `webview_execute_js` for navigation, click dispatch, and form population on macOS.
6. Use `webview_dom_snapshot` only after the main webview is visibly ready.

## Deterministic MCP Fixture

For local smoke, use the bundled fixture process:

- command: `/bin/bash`
- args:
  - `scripts/mcp-fixture-loop.sh`
  - `cratebay-tauri-mcp-ready`
- working dir: repo root

Expected assertions:

- after save, the new row appears and status is `Stopped`
- after start, row status becomes `Running`
- selected details also show `Running`
- logs contain `cratebay-tauri-mcp-ready`
- after stop, row status returns to `Stopped`

## Codex Tauri MCP Sequence

The shortest reliable Codex sequence is:

1. `mcp__tauri-mcp-server__driver_session` with `action=start` and `port=9223`
2. `mcp__tauri-mcp-server__ipc_get_backend_state`
3. `mcp__tauri-mcp-server__manage_window` to verify the main webview is exposed without crashing
4. `mcp__tauri-mcp-server__webview_execute_js` to dispatch clicks and set textarea/input values
5. `mcp__tauri-mcp-server__webview_wait_for` on selector text for `Running`, `Stopped`, and log markers when available
6. `mcp__tauri-mcp-server__webview_dom_snapshot` after the shell is stable and visible

When editing multiline fields, prefer JS value injection plus `input` / `change` dispatch instead of raw keystrokes.

## Current macOS Caveat

With the local `wry` patch rebuilt into the debug app, the earlier macOS panic in `wry::wkwebview::url_from_webview` is no longer reproduced during the basic MCP inspection path.

Observed live evidence on 2026-03-18:

- the bridge listens and the process stays alive when launched from `/tmp`
- `ipc_get_backend_state` succeeds
- `manage_window list` and `manage_window info` both succeed without crashing the app
- `webview_execute_js` succeeds for navigation, click dispatch, and input population
- `webview_dom_snapshot` with `type=structure` succeeds
- a live MCP smoke can create `local-mcp-1`, save it, start it, read logs, and stop it again

The remaining known MCP limitation on this machine is different:

- `webview_find_element` fails because `window.__MCP__.resolveRef` is missing
- `webview_interact` fails for the same reason

So the reliable short-term path is:

- use `manage_window` for window exposure checks
- use `webview_dom_snapshot type=structure` for selector discovery
- use `webview_execute_js` for clicks and input updates

## Artifacts

All local artifacts land in `dist/tauri-mcp/`:

- `run-context.txt` — effective bind/port/path context
- `app.log` — stdout/stderr from the debug app
- `app.pid` — running process id
- `config/` — isolated CrateBay config dir
- `data/` — isolated app data dir
- `logs/` — app log dir exposed through `CRATEBAY_LOG_DIR`

## Failure Triage

- process exits right after launch on macOS:
  - confirm the launch cwd is not the repository working directory
  - retry with `CRATEBAY_MCP_LAUNCH_CWD=/tmp`
- bridge does not come up:
  - inspect `dist/tauri-mcp/app.log`
  - confirm `CRATEBAY_MCP_PORT` is free
  - confirm the app is a debug build; the MCP bridge is disabled in release builds
- selector not found:
  - re-run `webview_dom_snapshot`
  - confirm the app landed on the expected page and tab
- input value not sticking:
  - use `webview_execute_js` to set the native input value and dispatch events
- MCP runtime row does not transition:
  - inspect `mcp-selected-status`
  - inspect `mcp-logs-output`
  - verify the fixture command path and working directory
- app disconnects during UI inspection on macOS:
  - re-check `dist/tauri-mcp/app.log`
  - confirm whether the crash still matches `wry::wkwebview::url_from_webview`; if not, capture the new stack
- `webview_interact` / `webview_find_element` fails immediately:
  - check whether `window.__MCP__.resolveRef` is still missing
  - fall back to `webview_execute_js` + `webview_dom_snapshot type=structure`
