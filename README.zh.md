<p align="center">
  <img src="https://raw.githubusercontent.com/coder-hhx/CrateBay/master/assets/logo.png" alt="CrateBay" width="128" />
</p>

<h1 align="center">CrateBay</h1>

<p align="center"><strong>v1.0 已上线</strong></p>

<p align="center">
  面向本地 AI 沙箱、本地模型、MCP Server 与 provider / CLI 桥接的开源桌面控制台。
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <strong>中文</strong>
</p>

## 公开说明

- CrateBay v1.0 已上线（下载见 GitHub Releases）。
- 当前公开 v1 范围聚焦于本地 AI 沙箱、本地模型、MCP Server 与 provider / CLI 桥接。
- 容器能力继续作为这些工作流的底层运行时。
- 在 macOS 与 Windows 上，CrateBay 提供内置的 Docker 兼容运行时（macOS：轻量 VM；Windows：WSL2）；就 CrateBay 自身而言，不再要求用户额外安装 Docker Desktop、Colima、`docker` 或 `docker compose`。
- VM 与 Kubernetes 暂列为 v1 之后的实验/扩展方向，待专用 runner 验证成熟后再升级承诺。
- 对外更新会在合适时机通过公开版本说明与 changelog 发布。
