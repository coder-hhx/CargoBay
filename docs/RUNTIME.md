# CrateBay Runtime (Built-in Docker Engine)

CrateBay Runtime is CrateBay’s built-in Docker-compatible engine path. On macOS it runs as a lightweight Linux VM; on Windows it runs as a bundled WSL2 distro. CrateBay GUI + CLI can use it without requiring users to install Docker Desktop, Colima, `docker`, or `docker compose`.

## macOS architecture (Virtualization.framework)

- Host VM runner: `cratebay-vz` (spawned by `cratebay-core`)
- Host Docker socket: `~/.cratebay/run/docker.sock`
- Transport: **TCP forwarding over the guest NAT IP**
  - Host side creates the Unix socket and, for each connection, opens a TCP connection to the guest (NAT IP) on port `6237`.
  - Guest side runs `cratebay-guest-agent`, listening on TCP `0.0.0.0:6237`, proxying traffic to the guest Docker socket (`/var/run/docker.sock`).

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

## Windows architecture (WSL2)

On Windows, CrateBay Runtime is implemented as a bundled **WSL2 distro** that runs `dockerd`.

- WSL distro name: `cratebay-runtime` (configurable via `CRATEBAY_RUNTIME_VM_NAME`)
- Docker Engine: `dockerd` inside WSL
  - Unix socket: `/var/run/docker.sock` (inside WSL)
  - TCP: `0.0.0.0:2375` (inside WSL, for host access)
- Host connection:
  - Preferred (when localhost forwarding is available): `DOCKER_HOST=tcp://127.0.0.1:2375`
  - Fallback: `DOCKER_HOST=tcp://<wsl-ip>:2375`

Runtime assets are bundled into the desktop app as `runtime-wsl/<arch>/rootfs.tar`; on first use CrateBay imports the distro via `wsl.exe --import`.

Release builds generate that `rootfs.tar` locally from Alpine packages during packaging, then embed it into the Windows installer so end users do not hit a first-run runtime download.

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
- On macOS/APFS, CrateBay prefers copy-on-write cloning when installing the bundled runtime assets and when creating VM disks, which makes first-run setup much faster.

## Useful knobs

- `CRATEBAY_DOCKER_SOCKET_PATH`: override host socket path
- `CRATEBAY_DOCKER_PROXY_PORT`: override guest proxy port (host + guest must match)
- `CRATEBAY_DOCKER_VSOCK_PORT`: legacy name for proxy port
- `CRATEBAY_RUNTIME_OS_IMAGE_ID`: override which OS image id to use
- `CRATEBAY_RUNTIME_ASSETS_DIR`: override the bundled runtime assets location
- `CRATEBAY_RUNTIME_HTTP_PROXY`: override the runtime image-pull proxy (macOS also falls back to the system proxy from `scutil --proxy` when present)
- `CRATEBAY_VZ_RUNNER_PATH`: override the macOS VM runner binary path
- `CRATEBAY_WSL_DOCKER_PORT`: override the WSL dockerd TCP port (Windows only)
- `CRATEBAY_WSL_ROOTFS_TAR`: override the WSL rootfs tar path (Windows only)
