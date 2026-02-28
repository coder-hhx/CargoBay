# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-28

### Added

- Tauri + Rust + React GUI application for container management.
- Docker container lifecycle management (list, start, stop, remove).
- Auto-detection of Docker socket paths (Colima, OrbStack, default `/var/run/docker.sock`).
- CLI tool with VM commands (`list`, `start`, `stop`, `status`) and Docker commands (`ps`, `start`, `stop`, `rm`).
- Dark and Light theme support with CSS custom properties.
- Multi-language support (English, 中文, 日本語, 한국어).
- Responsive layout with sidebar collapse on small windows.
- Custom CargoBay logo and branding.
- VM engine abstraction with `Hypervisor` trait (macOS Virtualization.framework, Linux KVM).
- gRPC service definitions for VM management.
- Daemon scaffolding for background services.
- Rust workspace with 4 crates: `libreorb-core`, `libreorb-cli`, `libreorb-daemon`, `libreorb-gui`.
- Cross-platform design with conditional compilation (`#[cfg(target_os)]`).
- Bollard crate for Docker API communication.

[0.1.0]: https://github.com/coder-hhx/CargoBay/releases/tag/v0.1.0
