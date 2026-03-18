#!/usr/bin/env bash
set -euo pipefail

marker="${1:-${CRATEBAY_MCP_FIXTURE_MARKER:-cratebay-tauri-mcp-ready}}"

echo "$marker"
echo "cwd=$(pwd)"
trap 'exit 0' INT TERM

while true; do
  sleep 1
done
