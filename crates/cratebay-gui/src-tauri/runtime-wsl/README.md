# CrateBay WSL Runtime Assets (Bundled)

This directory is bundled into the Windows desktop app as a resource.

CrateBay uses these assets to `wsl.exe --import` a lightweight WSL2 distro
that runs `dockerd` and exposes a Docker-compatible API to the host.

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
- In the repo, these are placeholders. Release packaging now generates the real `rootfs.tar` locally from Alpine packages before bundling the Windows installer.
