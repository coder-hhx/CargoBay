#!/usr/bin/env bash
set -euo pipefail

version="${1:-${CRATEBAY_ZIG_VERSION:-0.13.0}}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux) zig_os="linux" ;;
  Darwin) zig_os="macos" ;;
  *)
    echo "ERROR: unsupported host OS: $os" >&2
    exit 1
    ;;
esac

case "$arch" in
  x86_64|amd64) zig_arch="x86_64" ;;
  arm64|aarch64) zig_arch="aarch64" ;;
  *)
    echo "ERROR: unsupported host arch: $arch" >&2
    exit 1
    ;;
esac

base_name="zig-${zig_os}-${zig_arch}-${version}"
url="https://ziglang.org/download/${version}/${base_name}.tar.xz"

install_root="${CRATEBAY_ZIG_INSTALL_ROOT:-$HOME/.local/opt}"
bin_root="${CRATEBAY_ZIG_BIN_ROOT:-$HOME/.local/bin}"
target_dir="${install_root}/${base_name}"
tmp_dir="$(mktemp -d)"

cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

mkdir -p "$install_root" "$bin_root"

if [[ ! -x "${target_dir}/zig" ]]; then
  archive_path="${tmp_dir}/${base_name}.tar.xz"
  echo "== Download Zig ${version} (${zig_os}/${zig_arch}) =="
  curl -fL --retry 3 --retry-delay 1 -o "$archive_path" "$url"

  rm -rf "$target_dir"
  tar -xJf "$archive_path" -C "$install_root"
fi

ln -sfn "${target_dir}/zig" "${bin_root}/zig"

if [[ -n "${GITHUB_PATH:-}" ]]; then
  printf '%s\n' "$bin_root" >>"$GITHUB_PATH"
fi

echo "Installed Zig: ${bin_root}/zig"
"${bin_root}/zig" version
