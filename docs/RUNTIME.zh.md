# CrateBay Runtime（内置 Docker Engine）

CrateBay Runtime 是一台轻量 Linux VM，为 CrateBay（GUI + CLI）提供 Docker 兼容 API，从而避免用户必须安装 Colima / Docker Desktop / OrbStack。

## macOS 架构（Virtualization.framework）

- Host VM runner：`cratebay-vz`（由 `cratebay-core` 拉起）
- Host Docker socket：`~/.cratebay/run/docker.sock`
- 传输：**virtio-vsock**
  - Host 侧创建 Unix socket；每次有连接时，Host 会打开到 guest 的 vsock 连接并做转发。
  - Guest 侧运行 `cratebay-guest-agent`，监听 vsock 端口 `6237`，把流量转发到 guest 内的 Docker socket（`/var/run/docker.sock`）。

### Guest 侧要求（运行时镜像）

运行时 OS 镜像需要包含并在开机时启动：

- Docker Engine（`dockerd`），通过 **Unix socket** `/var/run/docker.sock` 对外提供服务
- `cratebay-guest-agent`，监听 **vsock** 端口 `6237`

要让 `docker` 兼容客户端连接到 CrateBay Runtime，可设置：

```bash
export DOCKER_HOST=unix://$HOME/.cratebay/run/docker.sock
```

## Windows 架构（WSL2）

在 Windows 上，CrateBay Runtime 以“随应用打包的 **WSL2 发行版**”实现，内部运行 `dockerd`。

- WSL distro 名称：`cratebay-runtime`（可通过 `CRATEBAY_RUNTIME_VM_NAME` 覆盖）
- Docker Engine：WSL 内的 `dockerd`
  - Unix socket：`/var/run/docker.sock`（WSL 内）
  - TCP：`0.0.0.0:2375`（WSL 内，用于宿主访问）
- Host 侧连接方式：
  - 优先（支持 localhost 转发时）：`DOCKER_HOST=tcp://127.0.0.1:2375`
  - 兜底：`DOCKER_HOST=tcp://<wsl-ip>:2375`

运行时资产随桌面应用打包在 `runtime-wsl/<arch>/rootfs.tar`；首次使用时，CrateBay 会通过 `wsl.exe --import` 导入该 distro。

## 运行时镜像

CrateBay 将 Runtime VM 视作一种 OS image：

- `cratebay-runtime-aarch64`
- `cratebay-runtime-x86_64`

运行时镜像随桌面应用打包（无需首次使用再下载）。

现阶段默认的 Runtime 走 **initramfs-first 的最小化 Linux**（LinuxKit/Alpine 风格），重点优化启动速度与体积；Debian 12 仍保留为“普通 VM 镜像”，用于更通用的 Linux VM 场景。

## 安装包大小与启动速度说明

要做到“安装即用”，桌面应用必须内置 Linux kernel + userspace 资产，这是体积的硬成本。为了尽量小、尽量快：

- CrateBay 按架构分别打包（避免 universal 包把 runtime 资产翻倍）。
- Runtime VM 的磁盘使用 **sparse file**（按需增长；未使用时“占用空间”很小）。
- 在 macOS/APFS 上，CrateBay 会优先使用 copy-on-write clone 来安装内置 runtime 资产、并加速 VM 磁盘初始化，从而显著降低首次启动的等待时间。

## 常用配置项

- `CRATEBAY_DOCKER_SOCKET_PATH`：覆盖 host socket 路径
- `CRATEBAY_DOCKER_VSOCK_PORT`：覆盖 guest vsock 端口（host + guest 必须一致）
- `CRATEBAY_RUNTIME_OS_IMAGE_ID`：覆盖使用哪个 OS image id
- `CRATEBAY_WSL_DOCKER_PORT`：覆盖 WSL 内 dockerd 的 TCP 端口（仅 Windows）
- `CRATEBAY_WSL_ROOTFS_TAR`：覆盖 WSL rootfs tar 的路径（仅 Windows）
