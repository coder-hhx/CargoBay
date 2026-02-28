<p align="center">
  <img src="assets/logo.png" alt="CargoBay" width="128" />
</p>

<h1 align="center">CargoBay</h1>

<p align="center">
  <strong>Free, open-source alternative to OrbStack.</strong><br>
  Lightweight Linux VMs, Docker containers, and Kubernetes — all in one app.
</p>

<p align="center">
  <a href="https://github.com/coder-hhx/CargoBay/releases">Download</a> ·
  <a href="https://github.com/coder-hhx/CargoBay/issues">Issues</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a> ·
  <a href="docs/TUTORIAL.md">Tutorial</a> ·
  <a href="CHANGELOG.md">Changelog</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-Apache%202.0-blue" />
  <img src="https://img.shields.io/badge/rust-1.75+-orange" />
  <img src="https://img.shields.io/badge/platform-macOS%20|%20Linux%20|%20Windows-lightgrey" />
</p>

---

## Why CargoBay?

OrbStack is great, but it's **closed-source and macOS-only**. Docker Desktop is **heavy and requires paid subscriptions**. Podman Desktop and Rancher Desktop use **Electron (300-500MB RAM)**. We believe developers deserve something better:

- **Name meaning**: *CargoBay* = `cargo` (containers, and a wink to Rust `cargo`) + `bay` (a home port for your VMs and dev environments)
- **100% free & open source** — Apache 2.0, no license fees, no telemetry
- **Rust + Tauri native** — not Electron, idles at <200MB RAM
- **VM + Containers unified** — one tool for everything
- **Cross-platform** — macOS, Linux, and Windows

## Comparison

| | CargoBay | OrbStack | Docker Desktop | Podman Desktop | Colima |
|---|:---:|:---:|:---:|:---:|:---:|
| **Open source** | ✅ | ❌ | Partial | ✅ | ✅ |
| **Free for commercial use** | ✅ | ❌ | ❌ (>250 employees) | ✅ | ✅ |
| **GUI** | Tauri (native) | Swift (native) | Electron | Electron | None |
| **Idle RAM** | <200 MB | <1 GB | 3-6 GB | 300-500 MB | ~400 MB |
| **macOS** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Windows** | Planned | ❌ | ✅ | ✅ | ❌ |
| **Linux** | Planned | ❌ | ✅ | ✅ | ✅ |
| **Docker management** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Linux VMs** | In Progress | ✅ | ❌ | ❌ | Indirect |
| **Kubernetes** | Planned | ✅ | ✅ | ✅ | ✅ (K3s) |
| **Auto port forwarding** | Planned | ✅ | ✅ | ❌ | ✅ |
| **VirtioFS file sharing** | In Progress | ✅ | ✅ | ❌ | ✅ |
| **Tech stack** | Rust | Swift | Go + Electron | Electron + TS | Go |

## Features

| Feature | macOS | Linux | Windows | Status |
|---------|-------|-------|---------|--------|
| Docker container management | ✅ | ✅ | ✅ | Working |
| Dashboard & GUI | ✅ | ✅ | ✅ | Working |
| Lightweight Linux VMs | ✅ Virtualization.framework | ✅ KVM | ✅ Hyper-V | In Progress |
| Rosetta x86_64 translation | ✅ Apple Silicon | — | — | In Progress |
| VirtioFS file sharing | ✅ | ✅ virtiofsd | ✅ Plan 9/SMB | In Progress |
| CLI (VM + Docker + Mount) | ✅ | ✅ | ✅ | Working |
| Dark/Light theme + i18n | ✅ | ✅ | ✅ | Working |
| Kubernetes (K3s) | 📋 | 📋 | 📋 | Planned |

## Tech Stack

- **Core**: Rust (cross-platform workspace)
- **GUI**: Tauri v2 + React (TypeScript)
- **VM Engine**: Virtualization.framework (macOS) / KVM (Linux) / Hyper-V (Windows)
- **File Sharing**: VirtioFS (macOS/Linux) / Plan 9 (Windows)
- **x86 Emulation**: Rosetta 2 (macOS Apple Silicon)
- **Containers**: Docker API via Bollard
- **CLI**: Rust (clap)
- **IPC**: gRPC (tonic + prost)

## Quick Start

> CargoBay is in early development. Not ready for production use.

```bash
# Build from source
git clone https://github.com/coder-hhx/CargoBay.git
cd CargoBay
cargo build --release

# CLI usage
cargobay status                              # Show platform info
cargobay docker ps                           # List containers
cargobay vm create myvm --cpus 4 --memory 4096 --rosetta  # Create VM with Rosetta
cargobay mount add --vm myvm --tag code --host-path ~/code --guest-path /mnt/code
```

See [Tutorial](docs/TUTORIAL.md) for detailed instructions.

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full system design.

```
┌──────────────────────────────────────────────┐
│  GUI (Tauri + React)    CLI (Rust/clap)      │
├──────────────────────────────────────────────┤
│              gRPC (tonic)                     │
├──────────────────────────────────────────────┤
│            Daemon (Rust)                      │
├────────────┬────────────┬────────────────────┤
│   macOS    │   Linux    │     Windows        │
│   Vz.fw    │   KVM      │     Hyper-V        │
│  +Rosetta  │ +virtiofsd │    +Plan 9/SMB     │
│  +VirtioFS │            │                    │
└────────────┴────────────┴────────────────────┘
```

## Contributing

We welcome contributions! Please open an issue or submit a pull request.

## License

Apache License 2.0 — free for personal and commercial use.
