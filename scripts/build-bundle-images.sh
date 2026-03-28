#!/bin/bash
# Build CrateBay sandbox images and export as tar.gz for offline bundling.
#
# Uses the CrateBay built-in runtime Docker (via ~/.cratebay/runtime/docker.sock).
# Falls back to default Docker if the runtime socket is not available.
#
# Usage:
#   ./scripts/build-bundle-images.sh [image...]
#
# Examples:
#   ./scripts/build-bundle-images.sh              # build all
#   ./scripts/build-bundle-images.sh python node   # build only python and node

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/crates/cratebay-gui/src-tauri/bundle-images"

mkdir -p "$OUTPUT_DIR"

# Use CrateBay runtime Docker socket if available
RUNTIME_SOCK="$HOME/.cratebay/runtime/docker.sock"
if [ -S "$RUNTIME_SOCK" ]; then
  export DOCKER_HOST="unix://$RUNTIME_SOCK"
  echo "Using CrateBay runtime Docker at $RUNTIME_SOCK"
else
  echo "Using default Docker"
fi

# Verify Docker is reachable
if ! docker info >/dev/null 2>&1; then
  echo "ERROR: Docker is not reachable. Start CrateBay runtime or Docker daemon first."
  exit 1
fi

pull_and_save() {
  local name="$1"
  local pull_image="$2"
  local tag_image="$3"
  local tarfile="$4"

  echo "=== Pulling $pull_image ==="
  docker pull "$pull_image"

  echo "=== Tagging as $tag_image ==="
  docker tag "$pull_image" "$tag_image"

  echo "=== Exporting $tag_image → $tarfile ==="
  docker save "$tag_image" | gzip > "$tarfile"

  local size
  size=$(du -h "$tarfile" | cut -f1)
  echo "=== Done: $tarfile ($size) ==="
  echo
}

# Determine which images to build
if [ $# -gt 0 ]; then
  TARGETS=("$@")
else
  TARGETS=(python node rust ubuntu)
fi

for name in "${TARGETS[@]}"; do
  case "$name" in
    python)
      pull_and_save "$name" "python:3.12-slim-bookworm" "cratebay-python-dev:v1" "$OUTPUT_DIR/python-dev.tar.gz"
      ;;
    node)
      pull_and_save "$name" "node:20-slim" "cratebay-node-dev:v1" "$OUTPUT_DIR/node-dev.tar.gz"
      ;;
    rust)
      pull_and_save "$name" "rust:1-slim-bookworm" "cratebay-rust-dev:v1" "$OUTPUT_DIR/rust-dev.tar.gz"
      ;;
    ubuntu)
      pull_and_save "$name" "ubuntu:24.04" "cratebay-ubuntu-base:v1" "$OUTPUT_DIR/ubuntu-base.tar.gz"
      ;;
    *)
      echo "ERROR: Unknown image '$name'. Available: python node rust ubuntu"
      exit 1
      ;;
  esac
done

echo "All images built successfully:"
ls -lh "$OUTPUT_DIR"/*.tar.gz 2>/dev/null || echo "(no tar.gz files found)"
