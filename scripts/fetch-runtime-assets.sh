#!/usr/bin/env bash
set -euo pipefail

arch="${1:-}"
if [[ -z "$arch" ]]; then
  echo "Usage: $0 <aarch64|x86_64> [dest_dir]" >&2
  exit 2
fi

dest_dir="${2:-crates/cratebay-gui/src-tauri/runtime-images}"
tag="${CRATEBAY_RUNTIME_TAG:-runtime-v0.1.0}"

image_id="cratebay-runtime-${arch}"
image_dir="${dest_dir}/${image_id}"

rm -rf "${dest_dir}/cratebay-runtime-aarch64" "${dest_dir}/cratebay-runtime-x86_64"
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

if ! download "${base_url}/vmlinuz-${arch}" "${tmp_dir}/vmlinuz" \
  || ! download "${base_url}/initramfs-${arch}" "${tmp_dir}/initramfs"; then
  echo "Remote runtime assets for tag '${tag}' are unavailable; building locally instead."
  bash "$(dirname "$0")/build-runtime-assets-alpine.sh" "${arch}" "${dest_dir}"
  exit 0
fi

mv "${tmp_dir}/vmlinuz" "${image_dir}/vmlinuz"
mv "${tmp_dir}/initramfs" "${image_dir}/initramfs"

# Optional: older runtime bundles may ship a rootfs image. Newer "Lite" runtimes
# are initramfs-first and create the VM disk on first boot, so rootfs may be absent.
if curl -fL --retry 3 --retry-delay 1 -o "${tmp_dir}/rootfs.img" "${base_url}/rootfs-${arch}.img" >/dev/null 2>&1; then
  mv "${tmp_dir}/rootfs.img" "${image_dir}/rootfs.img"
  echo "Downloaded optional rootfs.img"
else
  echo "No rootfs.img found for ${arch} (initramfs-only runtime)."
fi

echo "Runtime assets ready: ${image_dir}"
