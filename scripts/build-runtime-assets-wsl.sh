#!/usr/bin/env bash
set -euo pipefail

arch="${1:-}"
if [[ -z "$arch" ]]; then
  echo "Usage: $0 <aarch64|x86_64> [dest_dir]" >&2
  exit 2
fi

case "$arch" in
  aarch64|x86_64) ;;
  *)
    echo "ERROR: invalid arch '$arch' (expected aarch64 or x86_64)" >&2
    exit 2
    ;;
esac

dest_dir="${2:-crates/cratebay-gui/src-tauri/runtime-wsl}"
alpine_version="${CRATEBAY_ALPINE_VERSION:-v3.19}"
minirootfs_version="${CRATEBAY_ALPINE_MINIROOTFS_VERSION:-3.19.0}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

python_cmd=""
if command -v python3 >/dev/null 2>&1; then
  python_cmd="python3"
elif command -v python >/dev/null 2>&1; then
  python_cmd="python"
else
  echo "ERROR: python3 or python is required." >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "ERROR: curl is required." >&2
  exit 1
fi

if ! command -v tar >/dev/null 2>&1; then
  echo "ERROR: tar is required." >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

image_id="cratebay-runtime-wsl-${arch}"
image_dir="${dest_dir}/${image_id}"
rootfs_dir="$tmp_dir/rootfs"
apk_dir="$tmp_dir/apks"
mkdir -p "$image_dir" "$rootfs_dir" "$apk_dir"
rm -f "$image_dir/rootfs.tar"

download() {
  local url="$1"
  local out="$2"
  echo "Downloading ${url} -> ${out}"
  curl -fL --retry 3 --retry-delay 1 -o "${out}" "${url}"
}

echo "== Download Alpine minirootfs (${alpine_version}, ${arch}) =="
release_base="https://dl-cdn.alpinelinux.org/alpine/${alpine_version}/releases/${arch}"
download \
  "${release_base}/alpine-minirootfs-${minirootfs_version}-${arch}.tar.gz" \
  "$tmp_dir/minirootfs.tar.gz"
tar -xzf "$tmp_dir/minirootfs.tar.gz" -C "$rootfs_dir"

echo ""
echo "== Resolve Alpine package dependencies (docker-engine + iproute2) =="
"$python_cmd" - "$alpine_version" "$arch" >"$tmp_dir/pkglist.txt" <<'PY'
import io
import re
import sys
import tarfile
import urllib.request

alpine_version = sys.argv[1]
arch = sys.argv[2]

repos = [
    ("main", f"https://dl-cdn.alpinelinux.org/alpine/{alpine_version}/main/{arch}/APKINDEX.tar.gz"),
    ("community", f"https://dl-cdn.alpinelinux.org/alpine/{alpine_version}/community/{arch}/APKINDEX.tar.gz"),
]

pkg = {}
provides = {}

def fetch_index(url: str) -> str:
    with urllib.request.urlopen(url) as resp:
        data = resp.read()
    tf = tarfile.open(fileobj=io.BytesIO(data), mode="r:gz")
    raw = tf.extractfile("APKINDEX").read()
    return raw.decode("utf-8", "replace")

for repo, url in repos:
    idx = fetch_index(url)
    for block in idx.strip().split("\n\n"):
        name_match = re.search(r"^P:(.+)$", block, re.M)
        if not name_match:
            continue
        version_match = re.search(r"^V:(.+)$", block, re.M)
        if not version_match:
            continue
        deps_match = re.search(r"^D:(.+)$", block, re.M)
        provides_match = re.search(r"^p:(.+)$", block, re.M)

        name = name_match.group(1).strip()
        version = version_match.group(1).strip()
        deps = deps_match.group(1).split() if deps_match else []
        provided = [token.split("=", 1)[0] for token in provides_match.group(1).split()] if provides_match else []

        pkg[name] = {"repo": repo, "ver": version, "deps": deps, "provides": provided}
        for token in provided:
            provides.setdefault(token, set()).add(name)

def base_token(token: str) -> str:
    for sep in (">=", "<=", ">", "<", "=", "~"):
        if sep in token:
            return token.split(sep, 1)[0]
    return token

def resolve(token: str):
    token = base_token(token)
    if not token or token.startswith("/"):
        return None
    if token in pkg:
        return token
    if token in provides:
        return sorted(provides[token])[0]
    return None

roots = [
    "docker-engine",
    "containerd-ctr",
    "iproute2",
    "ca-certificates",
]

want = set()
stack = list(roots)

while stack:
    name = stack.pop()
    if name in want:
        continue
    if name not in pkg:
        print(f"ERROR: package not found in index: {name}", file=sys.stderr)
        sys.exit(2)
    want.add(name)
    for dep in pkg[name]["deps"]:
        resolved = resolve(dep)
        if resolved and resolved not in want:
            stack.append(resolved)

for name in sorted(want):
    info = pkg[name]
    print(f"{info['repo']}|{name}|{info['ver']}")
PY

echo "Resolved $(wc -l <"$tmp_dir/pkglist.txt" | tr -d ' ') packages."

download_apk() {
  local repo="$1"
  local name="$2"
  local version="$3"
  local out="$4"
  local url="https://dl-cdn.alpinelinux.org/alpine/${alpine_version}/${repo}/${arch}/${name}-${version}.apk"
  curl -fL --retry 3 --retry-delay 1 -o "$out" "$url"
}

echo ""
echo "== Download + extract Alpine packages =="
while IFS='|' read -r repo name version; do
  apk="$apk_dir/${name}-${version}.apk"
  echo "  - ${repo}/${name}-${version}.apk"
  download_apk "$repo" "$name" "$version" "$apk"
  tar -xf "$apk" -C "$rootfs_dir"
done <"$tmp_dir/pkglist.txt"

find "$rootfs_dir" -maxdepth 1 \
  \( -name '.PKGINFO' \
  -o -name '.pre-install' \
  -o -name '.post-install' \
  -o -name '.post-upgrade' \
  -o -name '.post-deinstall' \
  -o -name '.trigger' \
  -o -name '.SIGN.*' \) \
  -delete

echo ""
echo "== Write WSL runtime configuration =="
mkdir -p \
  "$rootfs_dir/etc/docker" \
  "$rootfs_dir/etc/profile.d" \
  "$rootfs_dir/run" \
  "$rootfs_dir/var" \
  "$rootfs_dir/var/lib/docker" \
  "$rootfs_dir/var/log"

cat >"$rootfs_dir/etc/docker/daemon.json" <<'JSON'
{
  "features": {
    "containerd-snapshotter": false
  }
}
JSON

cat >"$rootfs_dir/etc/wsl.conf" <<'CONF'
[boot]
systemd=false

[interop]
appendWindowsPath=false

[automount]
enabled=true
mountFsTab=false
options=metadata,uid=0,gid=0,umask=022,fmask=0111
CONF

cat >"$rootfs_dir/etc/profile.d/cratebay.sh" <<'SH'
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
SH
chmod 0755 "$rootfs_dir/etc/profile.d/cratebay.sh"

echo ""
echo "== Pack deterministic WSL rootfs.tar =="
"$python_cmd" - "$rootfs_dir" "$image_dir/rootfs.tar" <<'PY'
import os
import sys
import tarfile

root = os.path.abspath(sys.argv[1])
out = sys.argv[2]

with tarfile.open(out, "w") as tar:
    for current, dirs, files in os.walk(root, topdown=True, followlinks=False):
        dirs.sort()
        files.sort()

        rel_current = os.path.relpath(current, root)
        if rel_current != ".":
            info = tar.gettarinfo(current, arcname=rel_current)
            info.uid = 0
            info.gid = 0
            info.uname = "root"
            info.gname = "root"
            info.mtime = 0
            tar.addfile(info)

        for name in files:
            path = os.path.join(current, name)
            arcname = os.path.relpath(path, root)
            info = tar.gettarinfo(path, arcname=arcname)
            info.uid = 0
            info.gid = 0
            info.uname = "root"
            info.gname = "root"
            info.mtime = 0
            if info.isreg():
                with open(path, "rb") as fp:
                    tar.addfile(info, fp)
            else:
                tar.addfile(info)
PY

echo "WSL runtime assets ready: ${image_dir}"
