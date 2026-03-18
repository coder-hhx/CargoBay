# CrateBay Runtime (Built-in Docker Engine)

CrateBay Runtime is CrateBay’s built-in Docker-compatible engine path. On macOS it runs as a lightweight Linux VM; on Linux it runs as a bundled QEMU guest; on Windows it runs as a bundled WSL2 distro. CrateBay GUI + CLI can use it without requiring users to install Docker Desktop, Colima, `docker`, or `docker compose`.

## macOS architecture (Virtualization.framework)

- Host VM runner: `cratebay-vz` (spawned by `cratebay-core`)
- Host Docker socket: `~/.cratebay/run/docker.sock`
- Transport: **TCP forwarding over the guest NAT IP**
  - Host side creates the Unix socket and, for each connection, opens a TCP connection to the guest (NAT IP) on port `6237`.
  - Guest side runs `cratebay-guest-agent`, listening on TCP `0.0.0.0:6237`, proxying traffic to the guest Docker socket (`/var/run/docker.sock`).
- Default transport selection:
  - Intel macOS (`x86_64`) defaults to TCP forwarding because Apple Virtualization's virtio-vsock path is not stable enough there.
  - Apple Silicon keeps the lower-overhead vsock path by default.
  - `CRATEBAY_RUNTIME_SOCKET_FORWARD=tcp|vsock` can override the default for debugging.

### macOS signing note (required on newer macOS)

On newer macOS versions, Virtualization.framework requires the VM runner process to be code-signed with:

- `com.apple.security.virtualization`
- `com.apple.security.hypervisor`

Local development builds can use ad-hoc signing (what `scripts/install-local-macos-app.sh` does).

### Guest requirements (runtime image)

The runtime OS image must include and start on boot:

- Docker Engine (`dockerd`) listening on **Unix socket** `/var/run/docker.sock`
- `cratebay-guest-agent` listening on **TCP** `0.0.0.0:6237`

CrateBay exposes the host-side socket via `docker`-compatible clients by setting:

```bash
export DOCKER_HOST=unix://$HOME/.cratebay/run/docker.sock
```

## Linux architecture (bundled QEMU / KVM)

On Linux, CrateBay Runtime is implemented as a bundled **QEMU** runner that boots the same minimal CrateBay runtime guest image.

- Host helper: `runtime-linux/<arch>/qemu-system-*`
- Guest image: `cratebay-runtime-aarch64` / `cratebay-runtime-x86_64`
- Host connection:
  - Default: `DOCKER_HOST=tcp://127.0.0.1:2475`
  - Override: `CRATEBAY_LINUX_DOCKER_PORT=<port>`
- Networking:
  - QEMU user-mode NAT for guest egress
  - Host TCP forward `127.0.0.1:<host-port>` → guest TCP `6237`
- Acceleration:
  - Uses `/dev/kvm` automatically when available
  - Falls back to QEMU TCG when KVM is unavailable

Linux release builds stage that helper into `runtime-linux/<arch>/` together with the shared libraries and QEMU data files it needs, so end users do not need to install `qemu-system-*` separately.

Local Linux `tauri build` now stages the same helper automatically before packaging, so self-built installers keep the same install-and-use behavior.

## Windows architecture (WSL2)

On Windows, CrateBay Runtime is implemented as a bundled **WSL2 distro** that runs `dockerd`.

- WSL distro name: `cratebay-runtime` (configurable via `CRATEBAY_RUNTIME_VM_NAME`)
- Docker Engine: `dockerd` inside WSL
  - Unix socket: `/var/run/docker.sock` (inside WSL)
  - TCP: `0.0.0.0:2375` (inside WSL, for host access)
- Host connection:
  - Preferred (when the host can route to the WSL guest directly): `DOCKER_HOST=tcp://<wsl-ip>:2375`
  - Fallback: a CrateBay-managed local relay on loopback, e.g. `DOCKER_HOST=tcp://127.0.0.1:<relay-port>`

At startup, CrateBay waits briefly for the direct WSL guest endpoint to answer before falling back to a CrateBay-managed local relay on loopback. This avoids Windows localhost-forwarding cases where health probes succeed but Linux image pulls still get treated like Windows-platform requests.
When you use `cratebay runtime env` on Windows, CrateBay now prints PowerShell, CMD, and Bash snippets that set both `DOCKER_HOST` and `CRATEBAY_DOCKER_PLATFORM`, so subsequent `cratebay docker ... --pull` commands keep requesting Linux images from the bundled WSL runtime.

Runtime assets are bundled into the desktop app as `runtime-wsl/<arch>/rootfs.tar`; on first use CrateBay imports the distro via `wsl.exe --import`.

Release builds generate that `rootfs.tar` locally from Alpine packages during packaging, including bundled OpenRC service definitions for `containerd` and `docker`, then embed it into the Windows installer so end users do not hit a first-run runtime download.

Local Windows `tauri dev` / `tauri build` now performs the same asset preparation automatically when the repo still contains placeholder WSL assets.

CrateBay waits until the Docker API is actually reachable before reporting the Windows runtime as ready, and it auto-recovers a stale partial WSL import directory before re-importing the bundled distro.
When Windows must fall back from `127.0.0.1` to the guest address, CrateBay now prefers the host-reachable WSL NAT IP and skips bridge-only addresses such as Docker's `172.17.0.1`.
If a `wsl.exe` probe stalls during startup, CrateBay now fails that probe with a bounded timeout instead of hanging the whole `cratebay runtime start` flow indefinitely.
Windows now asks the bundled Alpine WSL distro to start Docker through OpenRC service scripts first, so the runtime follows the distro's native service lifecycle when that path is healthy.
If the Docker API still does not come up promptly, CrateBay stops any partial startup and retries once by spawning a detached `wsl.exe` foreground `dockerd` process with a compatibility-safe profile, then validates readiness by pinging `/_ping` inside the guest instead of relying on log text alone.
For deeper diagnosis, `CRATEBAY_RUNTIME_PROGRESS=1` prints the Windows WSL startup phases and probe command boundaries to stderr.

## Runtime images

CrateBay treats the runtime VM like an OS image:

- `cratebay-runtime-aarch64`
- `cratebay-runtime-x86_64`

These are bundled into the desktop app (no first-use download).

Today, the default runtime is a **minimal initramfs-first** Linux (LinuxKit/Alpine-style)
focused on boot speed and small footprint. Debian 12 remains available as a normal
VM image for general-purpose Linux VMs.

## Size & startup notes

Shipping an install-and-use runtime means the desktop app must include a Linux kernel + userspace assets. To keep downloads small and startup fast:

- CrateBay ships per-architecture desktop bundles (so you only download the runtime assets you need).
- Runtime VM disks are sparse files (they grow on demand; the “size on disk” stays small until you actually pull images).
- Linux bundles ship only the matching QEMU helper and its runtime libraries, not a multi-arch toolchain.
- On macOS/APFS, CrateBay prefers copy-on-write cloning when installing the bundled runtime assets and when creating VM disks, which makes first-run setup much faster.

## Useful knobs

- `CRATEBAY_DOCKER_SOCKET_PATH`: override host socket path
- `CRATEBAY_DOCKER_PROXY_PORT`: override guest proxy port (host + guest must match)
- `CRATEBAY_DOCKER_VSOCK_PORT`: legacy name for proxy port
- `CRATEBAY_RUNTIME_OS_IMAGE_ID`: override which OS image id to use
- `CRATEBAY_RUNTIME_ASSETS_DIR`: override the bundled runtime assets location
- `CRATEBAY_RUNTIME_QEMU_PATH`: override the Linux QEMU helper path
- `CRATEBAY_LINUX_DOCKER_PORT`: override the Linux runtime host TCP port
- `CRATEBAY_LINUX_RUNTIME_CMDLINE`: override the Linux runtime guest kernel cmdline
- `CRATEBAY_RUNTIME_HTTP_PROXY`: override the runtime image-pull proxy (macOS also falls back to the system proxy from `scutil --proxy` when present)
- `CRATEBAY_RUNTIME_SOCKET_FORWARD`: override the macOS runtime socket bridge (`tcp` or `vsock`)
- `CRATEBAY_VZ_RUNNER_PATH`: override the macOS VM runner binary path
- `CRATEBAY_WSL_DOCKER_PORT`: override the WSL dockerd TCP port (Windows only)
- `CRATEBAY_WSL_ROOTFS_TAR`: override the WSL rootfs tar path (Windows only)
