# CrateBay Runtime Linux Helper (Bundled)

This directory is bundled into the desktop app as a resource on Linux.

CrateBay ships the built-in runtime guest image separately in `runtime-images/`.
`runtime-linux/` contains the host-side helper used to boot that image on Linux:

```text
runtime-linux/
  cratebay-runtime-linux-x86_64/
    qemu-system-x86_64
    lib/
    share/qemu/
  cratebay-runtime-linux-aarch64/
    qemu-system-aarch64
    lib/
    share/qemu/
```

Notes:

- Release builds should include only the matching host architecture.
- The helper is launched by `cratebay-core::runtime::ensure_runtime_linux_running()`.
- The guest image is the same initramfs-first runtime shipped in `runtime-images/`.
