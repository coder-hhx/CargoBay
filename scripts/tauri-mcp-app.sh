#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
artifact_dir="$repo_root/dist/tauri-mcp"
config_dir="$artifact_dir/config"
data_dir="$artifact_dir/data"
log_dir="$artifact_dir/logs"
pid_file="$artifact_dir/app.pid"
app_log="$artifact_dir/app.log"
context_file="$artifact_dir/run-context.txt"
app_path="$repo_root/target/debug/cratebay-gui"
gui_dir="$repo_root/crates/cratebay-gui"
launch_cwd="${CRATEBAY_MCP_LAUNCH_CWD:-${TMPDIR:-/tmp}}"
default_bind="${CRATEBAY_MCP_BIND:-127.0.0.1}"
default_port="${CRATEBAY_MCP_PORT:-9223}"
action="${1:-start}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: required command '$1' not found"
    exit 1
  fi
}

ensure_node_runtime() {
  if command -v node >/dev/null 2>&1; then
    local current_major
    current_major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
    if (( current_major >= 22 )); then
      return 0
    fi
  fi

  export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
  if [[ -s "$NVM_DIR/nvm.sh" ]]; then
    . "$NVM_DIR/nvm.sh"
    for candidate in 24 22 --lts; do
      if nvm use "$candidate" >/dev/null 2>&1; then
        local nvm_major
        nvm_major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
        if (( nvm_major >= 22 )); then
          return 0
        fi
      fi
    done
  fi

  return 1
}

read_pid() {
  if [[ -f "$pid_file" ]]; then
    tr -d '[:space:]' < "$pid_file"
  fi
}

pid_is_running() {
  local pid="${1:-}"
  [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1
}

port_is_ready() {
  local host="$1"
  local port="$2"
  if command -v nc >/dev/null 2>&1; then
    nc -z "$host" "$port" >/dev/null 2>&1
  else
    (exec 3<>"/dev/tcp/$host/$port") >/dev/null 2>&1
  fi
}

wait_for_port() {
  local host="$1"
  local port="$2"
  local timeout_sec="${3:-30}"
  local started_at
  started_at="$(date +%s)"
  while true; do
    if port_is_ready "$host" "$port"; then
      return 0
    fi
    if (( "$(date +%s)" - started_at >= timeout_sec )); then
      return 1
    fi
    sleep 1
  done
}

write_context() {
  cat > "$context_file" <<CTX
artifact_dir=$artifact_dir
config_dir=$config_dir
data_dir=$data_dir
log_dir=$log_dir
app_log=$app_log
app_path=$app_path
gui_dir=$gui_dir
launch_cwd=$launch_cwd
mcp_bind=$default_bind
mcp_port=$default_port
pid_file=$pid_file
skip_asset_prep=${CRATEBAY_MCP_SKIP_ASSET_PREP:-0}
skip_npm_ci=${CRATEBAY_MCP_SKIP_NPM_CI:-0}
skip_build=${CRATEBAY_MCP_SKIP_BUILD:-0}
timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
CTX
}

print_status() {
  local pid
  pid="$(read_pid)"
  local state="stopped"
  if pid_is_running "$pid"; then
    state="running"
  elif [[ -n "$pid" ]]; then
    state="stale-pid"
  fi

  echo "state=$state"
  echo "pid=${pid:-}"
  echo "mcp_endpoint=ws://$default_bind:$default_port"
  if [[ -f "$context_file" ]]; then
    cat "$context_file"
  fi
  if port_is_ready "$default_bind" "$default_port"; then
    echo "bridge_ready=1"
  else
    echo "bridge_ready=0"
  fi
}

start_app() {
  require_cmd cargo
  require_cmd npm

  if ! ensure_node_runtime; then
    if command -v node >/dev/null 2>&1; then
      echo "ERROR: Node.js 22+ is required. Current: $(node -v)"
    else
      echo "ERROR: Node.js 22+ is required."
    fi
    echo "Use: nvm install 24 && nvm use 24"
    exit 1
  fi

  local existing_pid
  existing_pid="$(read_pid)"
  if pid_is_running "$existing_pid"; then
    echo "CrateBay Tauri MCP app is already running (pid=$existing_pid)"
    print_status
    exit 0
  fi

  mkdir -p "$artifact_dir" "$config_dir" "$data_dir" "$log_dir" "$launch_cwd"
  : > "$app_log"
  write_context

  if [[ "${CRATEBAY_MCP_SKIP_ASSET_PREP:-0}" != "1" ]]; then
    echo "== Prepare runtime assets =="
    node "$gui_dir/scripts/prepare-runtime-assets.mjs" build
  else
    echo "== Prepare runtime assets skipped =="
  fi

  if [[ "${CRATEBAY_MCP_SKIP_NPM_CI:-0}" != "1" ]]; then
    echo "== Install frontend dependencies =="
    npm --prefix "$gui_dir" ci
  else
    echo "== Install frontend dependencies skipped =="
  fi

  if [[ "${CRATEBAY_MCP_SKIP_BUILD:-0}" != "1" ]]; then
    echo "== Build frontend =="
    npm --prefix "$gui_dir" run build
  else
    echo "== Build frontend skipped =="
  fi

  if [[ "${CRATEBAY_MCP_SKIP_BUILD:-0}" != "1" ]]; then
    echo "== Build Tauri debug app =="
    cargo build --manifest-path "$repo_root/Cargo.toml" -p cratebay-gui --features custom-protocol
  else
    echo "== Build Tauri debug app skipped =="
  fi

  if [[ ! -x "$app_path" ]]; then
    echo "ERROR: built app not found at $app_path"
    exit 1
  fi

  echo "== Start Tauri app =="
  nohup env \
    CRATEBAY_CONFIG_DIR="$config_dir" \
    CRATEBAY_DATA_DIR="$data_dir" \
    CRATEBAY_LOG_DIR="$log_dir" \
    CRATEBAY_MCP_BIND="$default_bind" \
    CRATEBAY_MCP_PORT="$default_port" \
    RUST_LOG="${RUST_LOG:-info}" \
    CRATEBAY_MCP_APP_PATH_INTERNAL="$app_path" \
    CRATEBAY_MCP_LAUNCH_CWD_INTERNAL="$launch_cwd" \
    bash -lc 'cd "$CRATEBAY_MCP_LAUNCH_CWD_INTERNAL" && exec "$CRATEBAY_MCP_APP_PATH_INTERNAL"' \
    >"$app_log" 2>&1 < /dev/null &
  local app_pid=$!
  echo "$app_pid" > "$pid_file"

  sleep 2
  if ! pid_is_running "$app_pid"; then
    echo "ERROR: CrateBay exited during startup"
    tail -n 80 "$app_log" || true
    exit 1
  fi

  if ! wait_for_port "$default_bind" "$default_port" 30; then
    echo "ERROR: MCP bridge did not listen on $default_bind:$default_port"
    tail -n 80 "$app_log" || true
    exit 1
  fi

  print_status
}

stop_app() {
  local pid
  pid="$(read_pid)"
  if ! pid_is_running "$pid"; then
    rm -f "$pid_file"
    echo "CrateBay Tauri MCP app is not running"
    print_status
    exit 0
  fi

  kill "$pid" >/dev/null 2>&1 || true
  for _ in $(seq 1 10); do
    if ! pid_is_running "$pid"; then
      break
    fi
    sleep 1
  done

  if pid_is_running "$pid"; then
    kill -9 "$pid" >/dev/null 2>&1 || true
  fi

  rm -f "$pid_file"
  print_status
}

tail_logs() {
  if [[ ! -f "$app_log" ]]; then
    echo "No log file yet: $app_log"
    exit 1
  fi
  tail -n "${CRATEBAY_MCP_LOG_LINES:-80}" "$app_log"
}

case "$action" in
  start)
    start_app
    ;;
  stop)
    stop_app
    ;;
  status)
    print_status
    ;;
  logs)
    tail_logs
    ;;
  *)
    echo "Usage: $0 {start|stop|status|logs}"
    exit 2
    ;;
esac
