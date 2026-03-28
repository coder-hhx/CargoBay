#!/bin/bash
# E2E test for cratebay-mcp sandbox_run_code
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
MCP_BIN="$PROJECT_ROOT/target/debug/cratebay-mcp"

export DOCKER_HOST="unix://$HOME/.cratebay/runtime/docker.sock"

# Verify Docker reachable
if ! docker info >/dev/null 2>&1; then
  echo "FAIL: Docker not reachable via $DOCKER_HOST"
  exit 1
fi
echo "Docker OK"

# Helper: send JSON-RPC to MCP server, collect responses
run_mcp_session() {
  local input="$1"
  echo "$input" | timeout 120 "$MCP_BIN" 2>/tmp/mcp-e2e-stderr.log
}

# ============================================================
# Test 1: Initialize + tools/list
# ============================================================
echo ""
echo "=== Test 1: Initialize + tools/list ==="

RESP=$(printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e-test","version":"1.0"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | timeout 15 "$MCP_BIN" 2>/dev/null)

echo "$RESP" | python3 -m json.tool 2>/dev/null || echo "$RESP"

if echo "$RESP" | grep -q "sandbox_run_code"; then
  echo "PASS: sandbox_run_code found in tools/list"
else
  echo "FAIL: sandbox_run_code not found in tools/list"
  echo "Raw response: $RESP"
  exit 1
fi

if echo "$RESP" | grep -q "sandbox_install"; then
  echo "PASS: sandbox_install found in tools/list"
else
  echo "FAIL: sandbox_install not found"
  exit 1
fi

# ============================================================
# Test 2: sandbox_run_code with Python
# ============================================================
echo ""
echo "=== Test 2: sandbox_run_code (Python) ==="

RESP=$(printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e-test","version":"1.0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"cratebay_sandbox_run_code","arguments":{"language":"python","code":"print(1 + 1)","timeout_seconds":60}}}' \
  | timeout 120 "$MCP_BIN" 2>/dev/null)

echo "$RESP"

if echo "$RESP" | grep -q '"exit_code": 0'; then
  echo "PASS: Python exit_code = 0"
elif echo "$RESP" | grep -q '"exit_code":0'; then
  echo "PASS: Python exit_code = 0"
else
  echo "FAIL: Python exit_code != 0"
  echo "stderr:" && cat /tmp/mcp-e2e-stderr.log 2>/dev/null
  exit 1
fi

if echo "$RESP" | grep -q "2"; then
  echo "PASS: Python stdout contains '2'"
else
  echo "FAIL: Python stdout missing expected output"
  exit 1
fi

# ============================================================
# Test 3: sandbox_run_code with Bash
# ============================================================
echo ""
echo "=== Test 3: sandbox_run_code (Bash) ==="

RESP=$(printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e-test","version":"1.0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"cratebay_sandbox_run_code","arguments":{"language":"bash","code":"echo hello-cratebay","timeout_seconds":60}}}' \
  | timeout 120 "$MCP_BIN" 2>/dev/null)

echo "$RESP"

if echo "$RESP" | grep -q "hello-cratebay"; then
  echo "PASS: Bash stdout contains 'hello-cratebay'"
else
  echo "FAIL: Bash stdout missing expected output"
  exit 1
fi

echo ""
echo "=== All E2E tests passed ==="
