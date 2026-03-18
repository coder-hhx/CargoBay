# CrateBay WSL Runtime Assets (Bundled)

This directory is bundled into the Windows desktop app as a resource.

CrateBay uses these assets to `wsl.exe --import` a lightweight Alpine-based
WSL2 distro that exposes a Docker-compatible API to the host.

## Expected layout

```
runtime-wsl/
  cratebay-runtime-wsl-x86_64/
    rootfs.tar
  cratebay-runtime-wsl-aarch64/
    rootfs.tar
```

Notes:

- Release builds should include **only the matching host arch** to keep the app smaller.
- In the repo, these start as placeholders. Local `tauri dev` / `tauri build` and release packaging now generate the real `rootfs.tar` locally from Alpine packages, including bundled OpenRC service files for `containerd` and `docker`; at runtime CrateBay can still fall back to a detached direct `dockerd` bootstrap if the OpenRC path does not reach Docker API health.
