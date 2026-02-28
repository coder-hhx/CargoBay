# CargoBay Usage Tutorial

> CargoBay is a free, open-source alternative to OrbStack. Lightweight Docker container management through a native desktop GUI (Tauri + React) and a Rust-powered CLI.
>
> CargoBay 是 OrbStack 的免费开源替代方案。通过 Tauri + React 原生桌面 GUI 和 Rust 命令行工具，提供轻量级容器与虚拟机管理。

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Installation](#2-installation)
3. [GUI Guide](#3-gui-guide)
4. [CLI Reference](#4-cli-reference)
5. [Docker Socket Detection](#5-docker-socket-detection)
6. [Configuration](#6-configuration)
7. [Roadmap](#7-roadmap)

---

## 1. Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| **Rust** | 1.75+ | Core backend, CLI, Tauri backend |
| **Node.js** | 18+ | GUI frontend (React + Vite) |
| **npm** | 9+ | JavaScript dependencies |
| **Docker** | Any | Container engine |

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Install Node.js

```bash
# macOS
brew install node

# or via nvm
nvm install 18
```

### Docker Runtime

CargoBay works with any Docker-compatible runtime:

- **Colima** (recommended, free) — `brew install colima && colima start`
- **Docker Desktop** — the standard Docker experience
- **OrbStack** — CargoBay auto-detects its socket too

---

## 2. Installation

### Build from Source

```bash
git clone https://github.com/coder-hhx/CargoBay.git
cd CargoBay

# Install frontend dependencies
cd crates/libreorb-gui && npm install && cd ../..

# Build everything
cargo build --release
```

### Run the GUI (Development)

```bash
cd crates/libreorb-gui
npm run tauri dev
```

Hot-reload enabled: `.tsx` changes reload instantly, Rust changes trigger recompile.

### Production Build

```bash
cd crates/libreorb-gui
npm run tauri build
```

Output: `crates/libreorb-gui/src-tauri/target/release/bundle/`

- macOS: `.dmg` and `.app`
- Windows: `.msi` and `.exe`
- Linux: `.deb`, `.rpm`, `.AppImage`

### CLI Only

```bash
cargo build --release --bin cargobay
# Binary at: target/release/cargobay
```

---

## 3. GUI Guide

### Dashboard (仪表盘)

The default landing page. Shows a card-based overview:

| Card | Description |
|------|-------------|
| **Containers** | Total container count, click to jump to container management |
| **Virtual Machines** | VM count (coming soon) |
| **Images** | Docker image count (coming soon) |
| **System** | Docker connection status |

Running containers are previewed below the cards (up to 5).

### Containers (容器管理)

Full container management page:

- **Running containers** — shown with a green status dot and glow effect
- **Stopped containers** — shown with a grey dot

**Actions per container:**

| Action | Description |
|--------|-------------|
| **Start** | Start a stopped container |
| **Stop** | Gracefully stop (10s timeout) |
| **Delete** | Force stop + remove container |

The container list auto-refreshes every 3 seconds. Connection status is shown in the top-right pill.

### Virtual Machines (虚拟机)

Coming soon. Will support:
- Create/start/stop/delete lightweight Linux VMs
- macOS: Virtualization.framework
- Linux: KVM
- Windows: Hyper-V

### Images (镜像)

Coming soon. Will support:
- Pull/list/remove Docker images
- Search Docker Hub

### Settings (设置)

| Setting | Options |
|---------|---------|
| **Theme** | Dark (default) / Light |
| **Language** | English, 中文, 日本語, 한국어 |

Preferences are saved in `localStorage` and persist across sessions.

---

## 4. CLI Reference

### System Status

```bash
cargobay status
```

Output:
```
CargoBay v0.1.0
Platform: macOS aarch64 (Virtualization.framework available)
Rosetta x86_64: available
Docker: connected (~/.colima/default/docker.sock)
```

### Docker Commands

```bash
# List all containers
cargobay docker ps

# Start a container
cargobay docker start <container_id>

# Stop a container
cargobay docker stop <container_id>

# Remove a container (force)
cargobay docker rm <container_id>
```

### VM Commands

```bash
# Create a VM
cargobay vm create myvm --cpus 4 --memory 4096 --disk 20

# Create with Rosetta x86 translation (Apple Silicon)
cargobay vm create myvm --cpus 4 --memory 4096 --rosetta

# Start / Stop / Delete
cargobay vm start myvm
cargobay vm stop myvm
cargobay vm delete myvm

# List all VMs
cargobay vm list
```

### File Sharing (VirtioFS)

```bash
# Mount a host directory into a VM
cargobay mount add \
  --vm myvm \
  --tag code \
  --host-path ~/code \
  --guest-path /mnt/code

# Mount as read-only
cargobay mount add \
  --vm myvm \
  --tag data \
  --host-path ~/data \
  --guest-path /mnt/data \
  --readonly

# List mounts
cargobay mount list --vm myvm

# Remove a mount
cargobay mount remove --vm myvm --tag code
```

---

## 5. Docker Socket Detection

CargoBay auto-detects Docker sockets in this order:

| Priority | Path | Runtime |
|----------|------|---------|
| 1 | `~/.colima/default/docker.sock` | Colima |
| 2 | `~/.orbstack/run/docker.sock` | OrbStack |
| 3 | `/var/run/docker.sock` | Docker Desktop / native |
| 4 | `~/.docker/run/docker.sock` | Docker Desktop (alt) |

**Windows:** Also checks `//./pipe/docker_engine` and `//./pipe/dockerDesktopLinuxEngine`.

### Override

```bash
export DOCKER_HOST=unix:///path/to/custom/docker.sock
cargobay docker ps
```

---

## 6. Configuration

### Environment Variables

| Variable | Description |
|----------|-------------|
| `DOCKER_HOST` | Override Docker socket path |
| `RUST_LOG` | Set log level (`info`, `debug`, `trace`) |

### Data Locations

| Platform | Config | Logs |
|----------|--------|------|
| macOS | `~/Library/Application Support/com.cargobay.app/` | Same |
| Linux | `~/.config/cargobay/` | `~/.local/share/cargobay/` |
| Windows | `%APPDATA%\cargobay\` | Same |

---

## 7. Roadmap

| Version | Focus | Key Features |
|---------|-------|-------------|
| **v0.1** (current) | Foundation | Docker management, GUI, CLI, i18n |
| **v0.2** | Virtual Machines | VM lifecycle, VirtioFS, auto port forwarding |
| **v0.3** | Developer Experience | Container logs/terminal, image management, auto DNS |
| **v0.4** | Cross-platform | Windows (Hyper-V) + Linux (KVM) support |
| **v0.5** | Kubernetes | K3s integration, K8s dashboard |
| **v1.0** | Production Ready | Stability, auto-update, security audit |

---

## License

Apache License 2.0 — free for personal and commercial use.
