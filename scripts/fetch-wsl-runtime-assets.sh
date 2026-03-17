#!/usr/bin/env bash
set -euo pipefail

arch="${1:-}"
if [[ -z "$arch" ]]; then
  echo "Usage: $0 <aarch64|x86_64> [dest_dir]" >&2
  exit 2
fi

dest_dir="${2:-crates/cratebay-gui/src-tauri/runtime-wsl}"
tag="${CRATEBAY_RUNTIME_TAG:-runtime-v0.1.0}"

image_id="cratebay-runtime-wsl-${arch}"
image_dir="${dest_dir}/${image_id}"

rm -rf "${dest_dir}/cratebay-runtime-wsl-aarch64" "${dest_dir}/cratebay-runtime-wsl-x86_64"
mkdir -p "${image_dir}"

base_url="https://github.com/coder-hhx/CrateBay/releases/download/${tag}"

download() {
  local url="$1"
  local out="$2"
  echo "Downloading ${url} -> ${out}"
  curl -fL --retry 3 --retry-delay 1 -o "${out}" "${url}"
}

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT

if ! download "${base_url}/wsl-rootfs-${arch}.tar" "${tmp_dir}/rootfs.tar"; then
  echo "Remote WSL runtime assets for tag '${tag}' are unavailable; building locally instead."
  bash "$(dirname "$0")/build-runtime-assets-wsl.sh" "${arch}" "${dest_dir}"
  exit 0
fi
mv "${tmp_dir}/rootfs.tar" "${image_dir}/rootfs.tar"

echo "WSL runtime assets ready: ${image_dir}"
