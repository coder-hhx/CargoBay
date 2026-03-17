#!/usr/bin/env bash
set -euo pipefail

arch="${1:-}"
if [[ -z "$arch" ]]; then
  echo "Usage: $0 <aarch64|x86_64> [dest_dir]" >&2
  exit 2
fi

dest_dir="${2:-crates/cratebay-gui/src-tauri/runtime-linux}"

case "$arch" in
  x86_64)
    qemu_bin="qemu-system-x86_64"
    ;;
  aarch64)
    qemu_bin="qemu-system-aarch64"
    ;;
  *)
    echo "ERROR: invalid arch '$arch' (expected aarch64 or x86_64)" >&2
    exit 2
    ;;
esac

host_arch="$(uname -m)"
case "$host_arch" in
  x86_64|amd64)
    host_arch="x86_64"
    ;;
  aarch64|arm64)
    host_arch="aarch64"
    ;;
esac

if [[ "$host_arch" != "$arch" ]]; then
  echo "ERROR: building Linux runtime helper for '$arch' requires a matching '$arch' host." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: required command '$1' not found" >&2
    exit 1
  fi
}

require_cmd "$qemu_bin"
require_cmd ldd
require_cmd patchelf
require_cmd python3

image_id="cratebay-runtime-linux-${arch}"
image_dir="${dest_dir}/${image_id}"
rm -rf "$image_dir"
mkdir -p "$image_dir/lib" "$image_dir/share"

src_qemu="$(command -v "$qemu_bin")"
cp -L "$src_qemu" "$image_dir/$qemu_bin"
chmod 0755 "$image_dir/$qemu_bin"

python3 - "$src_qemu" "$image_dir/lib" <<'PY'
import os
import shutil
import subprocess
import sys

binary = sys.argv[1]
dest = sys.argv[2]
os.makedirs(dest, exist_ok=True)

output = subprocess.check_output(["ldd", binary], text=True)
for line in output.splitlines():
    line = line.strip()
    if not line or line.startswith("linux-vdso"):
        continue

    soname = None
    target = None

    if "=>" in line:
        left, right = line.split("=>", 1)
        soname = left.strip()
        target = right.strip().split(" ", 1)[0]
        if target == "not":
            raise SystemExit(f"missing dependency: {line}")
    else:
        target = line.split(" ", 1)[0]
        soname = os.path.basename(target)

    if not target or not os.path.isabs(target):
        continue
    if soname.startswith("ld-linux") or soname == "linux-vdso.so.1":
        continue

    shutil.copy2(target, os.path.join(dest, soname))
PY

if [[ -d /usr/share/qemu ]]; then
  cp -a /usr/share/qemu "$image_dir/share/"
fi

patchelf --set-rpath '$ORIGIN/lib' "$image_dir/$qemu_bin"

echo "Linux runtime helper ready: ${image_dir}"
