#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
gui_dir="$repo_root/crates/cratebay-gui"
target_app="$repo_root/target/release/bundle/macos/CrateBay.app"
install_app="/Applications/CrateBay.app"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "ERROR: This script only supports macOS."
  exit 1
fi

if ! command -v rsync >/dev/null 2>&1; then
  echo "ERROR: rsync is required."
  exit 1
fi

echo "== Build app bundle (app only, skip dmg) =="
(
  cd "$gui_dir"
  npm run tauri:build:app
)

if [[ ! -d "$target_app" ]]; then
  echo "ERROR: App bundle not found: $target_app"
  exit 1
fi

echo "== Install to /Applications =="
rsync -a "$target_app/" "$install_app/"

echo "Installed: $install_app"
if [[ "${1:-}" == "--open" ]]; then
  open "$install_app"
fi
