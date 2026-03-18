# CrateBay Runtime（内置 Docker Engine）

CrateBay Runtime 是 CrateBay 内置的 Docker 兼容运行时路径：在 macOS 上是轻量 Linux VM，在 Linux 上是随应用打包的 QEMU guest，在 Windows 上是随应用打包的 WSL2 发行版。CrateBay 的 GUI + CLI 可直接使用它，不要求用户再安装 Docker Desktop、Colima、`docker` 或 `docker compose`。

## macOS 架构（Virtualization.framework）

- Host VM runner：`cratebay-vz`（由 `cratebay-core` 拉起）
- Host Docker socket：`~/.cratebay/run/docker.sock`
- 传输：**通过 guest NAT IP 的 TCP 转发**
  - Host 侧创建 Unix socket；每次有连接时，Host 会连接到 guest（NAT IP）的 TCP `6237` 并做转发。
  - Guest 侧运行 `cratebay-guest-agent`，监听 TCP `0.0.0.0:6237`，把流量转发到 guest 内的 Docker socket（`/var/run/docker.sock`）。
- 默认传输选择：
  - Intel macOS（`x86_64`）默认走 TCP 转发，因为 Apple Virtualization 的 virtio-vsock 路径在该平台上稳定性不足。
  - Apple Silicon 默认继续走开销更低的 vsock。
  - 如需调试，可通过 `CRATEBAY_RUNTIME_SOCKET_FORWARD=tcp|vsock` 覆盖默认值。

### macOS 签名说明（较新 macOS 版本必需）

在较新的 macOS 版本上，Virtualization.framework 要求 VM runner 进程具备以下 entitlement：

- `com.apple.security.virtualization`
- `com.apple.security.hypervisor`

本地开发可使用 ad-hoc 签名（`scripts/install-local-macos-app.sh` 已自动处理）。

### Guest 侧要求（运行时镜像）

运行时 OS 镜像需要包含并在开机时启动：

- Docker Engine（`dockerd`），通过 **Unix socket** `/var/run/docker.sock` 对外提供服务
- `cratebay-guest-agent`，监听 **TCP** `0.0.0.0:6237`

要让 `docker` 兼容客户端连接到 CrateBay Runtime，可设置：

```bash
export DOCKER_HOST=unix://$HOME/.cratebay/run/docker.sock
```

## Linux 架构（内置 QEMU / KVM）

在 Linux 上，CrateBay Runtime 以“随应用打包的 **QEMU runner**”实现，启动的仍然是同一套最小化 CrateBay runtime guest 镜像。

- Host helper：`runtime-linux/<arch>/qemu-system-*`
- Guest 镜像：`cratebay-runtime-aarch64` / `cratebay-runtime-x86_64`
- Host 侧连接：
  - 默认：`DOCKER_HOST=tcp://127.0.0.1:2475`
  - 可通过 `CRATEBAY_LINUX_DOCKER_PORT=<port>` 覆盖
- 网络：
  - Guest 对外走 QEMU user-mode NAT
  - Host TCP `127.0.0.1:<host-port>` 转发到 guest TCP `6237`
- 加速：
  - 有 `/dev/kvm` 时自动使用 KVM
  - 没有 KVM 时自动回退到 QEMU TCG

Linux release 会把 helper、所需共享库和 QEMU data files 一起打进 `runtime-linux/<arch>/`，因此终端用户无需再额外安装 `qemu-system-*`。

本地 Linux `tauri build` 现在也会在打包前自动准备同一套 helper，因此自构建安装包仍然保持“安装即用”。

## Windows 架构（WSL2）

在 Windows 上，CrateBay Runtime 以“随应用打包的 **WSL2 发行版**”实现，内部运行 `dockerd`。

- WSL distro 名称：`cratebay-runtime`（可通过 `CRATEBAY_RUNTIME_VM_NAME` 覆盖）
- Docker Engine：WSL 内的 `dockerd`
  - Unix socket：`/var/run/docker.sock`（WSL 内）
  - TCP：`0.0.0.0:2375`（WSL 内，用于宿主访问）
- Host 侧连接方式：
  - 优先（宿主可直接路由到 WSL guest 时）：`DOCKER_HOST=tcp://<wsl-ip>:2375`
  - 兜底：`DOCKER_HOST=tcp://127.0.0.1:2375`

运行时资产随桌面应用打包在 `runtime-wsl/<arch>/rootfs.tar`；首次使用时，CrateBay 会通过 `wsl.exe --import` 导入该 distro。

Windows release 构建会在打包阶段基于 Alpine 包本地生成这个 `rootfs.tar`，其中也会内置 `containerd` / `docker` 的 OpenRC service 定义，再把它嵌入安装包，因此终端用户不会在首次运行时再触发运行时下载。

本地 Windows `tauri dev` / `tauri build` 现在也会在 repo 仍是 placeholder 资产时自动完成同样的准备流程。

CrateBay 只有在 Docker API 实际可达后，才会把 Windows runtime 标记为 ready；如果上一次失败导入留下了脏的 WSL 安装目录，它也会先自动清理再重新导入内置 distro。
当 Windows 需要从 `127.0.0.1` 回退到 guest IP 时，CrateBay 现在会优先选择宿主可达的 WSL NAT 地址，并跳过 Docker `172.17.0.1` 这类仅桥接可见的地址。
如果启动过程中某个 `wsl.exe` 探测卡住，CrateBay 现在会在限定超时后直接报错，而不是让整个 `cratebay runtime start` 一直挂住。
Windows 现在会优先让内置 Alpine WSL distro 通过 OpenRC service 脚本启动 Docker，在这条路径健康时尽量走发行版原生的服务生命周期。
如果 Docker API 在合理时间内仍未就绪，CrateBay 会先清理掉半启动状态，再通过一个分离的 `wsl.exe` 前台 `dockerd` 进程用兼容性更高的参数重试一次，并改为在 guest 内直接请求 `/_ping` 判断 readiness，而不是只依赖日志文本。
如需进一步定位，可设置 `CRATEBAY_RUNTIME_PROGRESS=1`，让 Windows WSL 启动阶段与探测命令边界输出到 stderr。

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
- Linux 安装包只携带匹配当前架构的 QEMU helper 与运行时依赖，不带多架构工具链。
- 在 macOS/APFS 上，CrateBay 会优先使用 copy-on-write clone 来安装内置 runtime 资产、并加速 VM 磁盘初始化，从而显著降低首次启动的等待时间。

## 常用配置项

- `CRATEBAY_DOCKER_SOCKET_PATH`：覆盖 host socket 路径
- `CRATEBAY_DOCKER_PROXY_PORT`：覆盖 guest proxy 端口（host + guest 必须一致）
- `CRATEBAY_DOCKER_VSOCK_PORT`：proxy 端口的历史名称（兼容）
- `CRATEBAY_RUNTIME_OS_IMAGE_ID`：覆盖使用哪个 OS image id
- `CRATEBAY_RUNTIME_ASSETS_DIR`：覆盖内置 runtime 资产目录
- `CRATEBAY_RUNTIME_QEMU_PATH`：覆盖 Linux QEMU helper 路径
- `CRATEBAY_LINUX_DOCKER_PORT`：覆盖 Linux runtime 的 host TCP 端口
- `CRATEBAY_LINUX_RUNTIME_CMDLINE`：覆盖 Linux runtime guest 的 kernel cmdline
- `CRATEBAY_RUNTIME_HTTP_PROXY`：覆盖 runtime 拉取镜像时使用的代理（macOS 在未显式设置时也会回退读取 `scutil --proxy` 的系统代理）
- `CRATEBAY_RUNTIME_SOCKET_FORWARD`：覆盖 macOS runtime socket 桥接方式（`tcp` 或 `vsock`）
- `CRATEBAY_VZ_RUNNER_PATH`：覆盖 macOS VM runner 二进制路径
- `CRATEBAY_WSL_DOCKER_PORT`：覆盖 WSL 内 dockerd 的 TCP 端口（仅 Windows）
- `CRATEBAY_WSL_ROOTFS_TAR`：覆盖 WSL rootfs tar 的路径（仅 Windows）
