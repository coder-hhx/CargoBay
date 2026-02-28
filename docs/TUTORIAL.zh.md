# CargoBay 使用教程（中文）

> [English](TUTORIAL.md) · **中文**
>
> CargoBay 是 OrbStack 的免费开源替代方案。通过 Tauri + React 原生桌面 GUI 与 Rust 命令行工具，提供轻量级容器与虚拟机管理。

---

## 目录

1. [环境准备](#1-环境准备)
2. [安装与构建](#2-安装与构建)
3. [GUI 使用指南](#3-gui-使用指南)
4. [CLI 命令参考](#4-cli-命令参考)
5. [Docker Socket 自动识别](#5-docker-socket-自动识别)
6. [配置与数据目录](#6-配置与数据目录)
7. [路线图](#7-路线图)

---

## 1. 环境准备

| 工具 | 版本 | 用途 |
|------|---------|---------|
| **Rust** | 1.75+ | 后端、CLI、Tauri 后端 |
| **Node.js** | 18+ | GUI 前端（React + Vite） |
| **npm** | 9+ | JavaScript 依赖 |
| **Docker** | 任意 | 容器运行时 |

### 平台兼容性

- **macOS**：兼容 Apple Silicon（M 系列）与 Intel（x86_64）。Rosetta x86_64 仅在 Apple Silicon + macOS 13+ 可用。
- **Windows**：目标兼容 Windows 10 与 Windows 11。VM 后端依赖 Hyper-V（通常需要 Pro/Enterprise/Education + 启用 Hyper-V）。
- **Linux**：VM 后端依赖 KVM（需要 `/dev/kvm` 及权限）。

### 安装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 安装 Node.js

```bash
# macOS
brew install node

# or via nvm
nvm install 18
```

### Docker 运行时

CargoBay 支持任意 Docker 兼容运行时：

- **Colima**（推荐，免费）— `brew install colima && colima start`
- **Docker Desktop** — 常见 Docker 体验
- **OrbStack** — CargoBay 也会自动识别其 socket

---

## 2. 安装与构建

### 从源码构建

```bash
git clone https://github.com/coder-hhx/CargoBay.git
cd CargoBay

# 安装前端依赖
cd crates/cargobay-gui && npm install && cd ../..

# 构建
cargo build --release
```

### 运行 GUI（开发模式）

```bash
cd crates/cargobay-gui
npm run tauri dev
```

支持热更新：`.tsx` 改动会即时刷新；Rust 改动会触发重新编译。

### 生产构建

```bash
cd crates/cargobay-gui
npm run tauri build
```

输出目录：`crates/cargobay-gui/src-tauri/target/release/bundle/`

- macOS：`.dmg` / `.app`
- Windows：`.msi` / `.exe`
- Linux：`.deb` / `.rpm` / `.AppImage`

### 仅构建 CLI

```bash
cargo build --release --bin cargobay
# 二进制：target/release/cargobay
```

---

## 3. GUI 使用指南

### Dashboard（仪表盘）

默认首页，会展示卡片式概览：

| 卡片 | 说明 |
|------|-------------|
| **Containers** | 容器总数，点击进入容器管理 |
| **Virtual Machines** | 虚拟机数量（预览） |
| **Images** | 镜像搜索结果数量（最近一次搜索） |
| **System** | Docker 连接状态 |

运行中的容器会在下方预览（最多 5 个）。

### Containers（容器管理）

完整容器管理页面：

- **运行中** — 绿色状态点
- **已停止** — 灰色状态点

**每个容器支持的操作：**

| 操作 | 说明 |
|--------|-------------|
| **Start** | 启动已停止容器 |
| **Stop** | 优雅停止（10 秒超时） |
| **Delete** | 强制停止并删除容器 |
| **Login command** | 显示该容器的 `docker exec -it ...` 登录命令 |
| **Package as image** | 基于容器生成新镜像（`docker commit`） |

容器列表每 3 秒自动刷新；右上角会显示连接状态。

### Virtual Machines（虚拟机）

v0.1 预览版支持：

- **创建 / 启动 / 停止 / 删除 / 列表**（内存预览版）
- 创建时可设置 **CPU / 内存 / 磁盘**
- **Rosetta 开关**（仅 macOS Apple Silicon；是否可用取决于 macOS 13+）
- **VirtioFS 挂载列表**（UI 中会记录；真实挂载后续接入）
- **登录命令**：生成 `ssh user@host -p <port>`（端口需你手动提供）

> 注意：VM 状态目前仅在应用运行期间保存在内存中，暂未持久化。

### Images（镜像）

v0.1 已支持：

- **镜像搜索**：Docker Hub、Quay
- **标签列表**：对带域名的镜像引用列出 tags（如 `quay.io/org/image`、`ghcr.io/org/image`）
- **基于镜像创建容器**：可选 **CPU 核数 / 内存(MB)**，可选 **创建前拉取**
- **导入自定义镜像**：从本地 `.tar` 归档导入（`docker load -i`）
- **上传镜像到仓库**：`docker push`

> 提示：Docker Hub 的镜像一般使用 `docker run` 风格引用（如 `nginx:latest`）。tags 列表目前需要带域名的引用。

### Settings（设置）

| 设置项 | 选项 |
|---------|---------|
| **Theme** | Dark（默认）/ Light |
| **Language** | English, 中文 |

偏好会保存在 `localStorage` 中并持久化。

---

## 4. CLI 命令参考

### 系统状态

```bash
cargobay status
```

示例输出：
```
CargoBay v0.1.0
Platform: macOS aarch64 (Virtualization.framework available)
Rosetta x86_64: available
Docker: connected (~/.colima/default/docker.sock)
```

### Docker 命令

```bash
# 列出容器
cargobay docker ps

# 运行一个新容器（可选 CPU/内存限制，可选拉取镜像）
cargobay docker run nginx:latest --name web --cpus 2 --memory 512 --pull

# 启动容器
cargobay docker start <container_id>

# 停止容器
cargobay docker stop <container_id>

# 删除容器（强制）
cargobay docker rm <container_id>

# 输出容器登录命令（shell）
cargobay docker login-cmd web
```

### VM 命令

```bash
# 创建 VM（可自定义 CPU 核数与内存）
cargobay vm create myvm --cpus 4 --memory 4096 --disk 20

# Apple Silicon 上启用 Rosetta x86 翻译
cargobay vm create myvm --cpus 4 --memory 4096 --rosetta

# 启动 / 停止 / 删除
cargobay vm start myvm
cargobay vm stop myvm
cargobay vm delete myvm

# 列出全部 VM
cargobay vm list

# 输出 VM 登录命令（SSH，需要你提供端口）
cargobay vm login-cmd myvm --user root --host 127.0.0.1 --port 2222
```

### 镜像命令

```bash
# 搜索镜像（Docker Hub / Quay）
cargobay image search nginx --source all --limit 20

# 列出某个 OCI 镜像仓库的 tags（支持 ghcr.io/quay.io/私有仓库等）
cargobay image tags ghcr.io/owner/image --limit 50

# 导入镜像归档（.tar）
cargobay image load ./image.tar

# 上传镜像到仓库
cargobay image push ghcr.io/owner/image:tag

# 基于已有容器打包镜像
cargobay image pack-container web myorg/web:snapshot
```

### 文件共享（VirtioFS）

```bash
# 把宿主机目录挂载到 VM 内
cargobay mount add \
  --vm myvm \
  --tag code \
  --host-path ~/code \
  --guest-path /mnt/code

# 只读挂载
cargobay mount add \
  --vm myvm \
  --tag data \
  --host-path ~/data \
  --guest-path /mnt/data \
  --readonly

# 查看挂载
cargobay mount list --vm myvm

# 移除挂载
cargobay mount remove --vm myvm --tag code
```

---

## 5. Docker Socket 自动识别

CargoBay 会按以下顺序自动识别 Docker socket：

| 优先级 | 路径 | 运行时 |
|----------|------|---------|
| 1 | `~/.colima/default/docker.sock` | Colima |
| 2 | `~/.orbstack/run/docker.sock` | OrbStack |
| 3 | `/var/run/docker.sock` | Docker Desktop / 原生 |
| 4 | `~/.docker/run/docker.sock` | Docker Desktop（备用） |

**Windows：** 也会尝试 `//./pipe/docker_engine` 与 `//./pipe/dockerDesktopLinuxEngine`。

### 覆盖默认识别顺序

```bash
export DOCKER_HOST=unix:///path/to/custom/docker.sock
cargobay docker ps
```

---

## 6. 配置与数据目录

### 环境变量

| 变量 | 说明 |
|----------|-------------|
| `DOCKER_HOST` | 覆盖 Docker socket 路径 |
| `RUST_LOG` | 日志级别（`info` / `debug` / `trace`） |

### 数据目录

| 平台 | 配置 | 日志 |
|----------|--------|------|
| macOS | `~/Library/Application Support/com.cargobay.app/` | 同上 |
| Linux | `~/.config/cargobay/` | `~/.local/share/cargobay/` |
| Windows | `%APPDATA%\cargobay\` | 同上 |

---

## 7. 路线图

| 版本 | 重点 | 关键功能 |
|---------|-------|-------------|
| **v0.1**（当前） | 基础可用 | Docker 管理、GUI、CLI、i18n（中/英） |
| **v0.2** | 虚拟机 | VM 生命周期、VirtioFS、自动端口转发 |
| **v0.3** | 开发者体验 | 容器日志/终端、镜像管理、自动 DNS |
| **v0.4** | 跨平台 | Windows（Hyper-V）+ Linux（KVM） |
| **v0.5** | Kubernetes | K3s 集成、K8s 仪表盘 |
| **v1.0** | 生产就绪 | 稳定性、自动更新、安全审计 |

---

## License

Apache License 2.0 — 可免费用于个人与商业用途。
