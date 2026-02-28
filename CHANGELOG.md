# Changelog

> **English** · [中文](CHANGELOG.zh.md)

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- CLI: image search (`cargobay image search`) and tag listing (`cargobay image tags`).
- CLI: image import/push (`cargobay image load` / `cargobay image push`) and container snapshot packaging (`cargobay image pack-container`).
- CLI: run containers with optional CPU/memory limits (`cargobay docker run --cpus/--memory`) and optional pull (`--pull`).
- CLI: print container login command (`cargobay docker login-cmd`).
- GUI: Images page (search Docker Hub/Quay, list tags for registry references, run container with CPU/memory, import/push images).
- GUI: VM page (preview VM lifecycle UI, VirtioFS mount tracking, login command generator).
- GUI: show container login commands and package a container as an image (docker commit).

## [0.1.0] - 2026-02-28

### Added

- Tauri + Rust + React GUI application for container management.
- Docker container lifecycle management (list, start, stop, remove).
- Auto-detection of Docker socket paths (Colima, OrbStack, default `/var/run/docker.sock`).
- CLI tool with VM commands (`list`, `start`, `stop`, `status`) and Docker commands (`ps`, `start`, `stop`, `rm`).
- Dark and Light theme support with CSS custom properties.
- Multi-language support (English, 中文).
- Responsive layout with sidebar collapse on small windows.
- Custom CargoBay logo and branding.
- VM engine abstraction with `Hypervisor` trait (macOS Virtualization.framework, Linux KVM).
- gRPC service definitions for VM management.
- Daemon scaffolding for background services.
- Rust workspace with 4 crates: `cargobay-core`, `cargobay-cli`, `cargobay-daemon`, `cargobay-gui`.
- Cross-platform design with conditional compilation (`#[cfg(target_os)]`).
- Bollard crate for Docker API communication.

[0.1.0]: https://github.com/coder-hhx/CargoBay/releases/tag/v0.1.0
