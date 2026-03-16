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

arch="$(uname -m)"
runtime_arch="$arch"
if [[ "$runtime_arch" == "arm64" ]]; then
  runtime_arch="aarch64"
fi
if [[ "$runtime_arch" != "aarch64" && "$runtime_arch" != "x86_64" ]]; then
  echo "ERROR: unsupported arch '$arch' (expected arm64/aarch64 or x86_64)." >&2
  exit 1
fi

rust_target="$(rustc -vV | awk '/^host:/ {print $2}' | head -n 1)"
if [[ -z "$rust_target" ]]; then
  echo "ERROR: failed to resolve Rust host target." >&2
  exit 1
fi

echo "== Prepare bundled runtime assets (${runtime_arch}) =="
bash "$repo_root/scripts/build-runtime-assets-alpine.sh" "$runtime_arch"

echo "== Prepare Tauri external binaries (${rust_target}) =="
bash "$repo_root/scripts/prepare-tauri-external-bins.sh" "$rust_target"

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
rm -rf "$install_app"
rsync -a "$target_app/" "$install_app/"

echo "== Codesign (adhoc) for Virtualization.framework =="
if command -v codesign >/dev/null 2>&1; then
  identity="${CRATEBAY_CODESIGN_IDENTITY:--}"
  entitlements="$repo_root/scripts/macos-entitlements.plist"

  if [[ -f "$entitlements" ]]; then
    # Sign nested code first, then the outer app bundle last.
    if [[ -f "$install_app/Contents/MacOS/cratebay-vz" ]]; then
      codesign --force --sign "$identity" --options runtime --entitlements "$entitlements" "$install_app/Contents/MacOS/cratebay-vz"
    fi
    if [[ -f "$install_app/Contents/MacOS/cratebay-gui" ]]; then
      codesign --force --sign "$identity" --options runtime "$install_app/Contents/MacOS/cratebay-gui"
    fi
    codesign --force --sign "$identity" --options runtime "$install_app"
  else
    echo "WARN: entitlements plist not found: $entitlements"
  fi
else
  echo "WARN: codesign not available; VM runner may fail on newer macOS versions."
fi

echo "Installed: $install_app"
if [[ "${1:-}" == "--open" ]]; then
  open "$install_app"
fi
