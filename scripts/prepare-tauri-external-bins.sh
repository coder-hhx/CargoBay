#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "SKIP: prepare-tauri-external-bins.sh is macOS-only."
  exit 0
fi

target="${1:-}"
if [[ -z "$target" ]]; then
  target="$(rustc -vV | awk '/^host:/ {print $2}' | head -n 1)"
fi
if [[ -z "$target" ]]; then
  echo "ERROR: failed to resolve Rust host target." >&2
  exit 1
fi

bin_dir="crates/cratebay-gui/src-tauri/bin"
mkdir -p "$bin_dir"

echo "== Build cratebay-vz (${target}) =="
cargo build --release --target "$target" -p cratebay-vz

src="target/${target}/release/cratebay-vz"
if [[ ! -f "$src" ]]; then
  # Fallback for builds without explicit --target.
  src="target/release/cratebay-vz"
fi
if [[ ! -f "$src" ]]; then
  echo "ERROR: cratebay-vz binary not found (checked: target/${target}/release/cratebay-vz, target/release/cratebay-vz)" >&2
  exit 1
fi

dst="${bin_dir}/cratebay-vz-${target}"
cp "$src" "$dst"
chmod +x "$dst"

echo "External bin ready: ${dst}"

