# CrateBay Linux Runtime Helpers (Bundled)

This directory is bundled into the desktop app as a resource.

CrateBay's Linux runtime uses **KVM/QEMU**. To keep the app **zero-dependency**,
release bundles should ship a self-contained `qemu-system-*` plus its runtime
libraries and `share/qemu` data.

## Expected layout

```
runtime-linux/
  cratebay-runtime-linux-x86_64/
    qemu-system-x86_64
    lib/
      *.so*
    share/
      qemu/
        ...
  cratebay-runtime-linux-aarch64/
    qemu-system-aarch64
    lib/
      *.so*
    share/
      qemu/
        ...
```

## How to generate (maintainers / CI)

Use the helper script (requires a matching-arch Linux host with `qemu-system-*`,
`ldd`, `patchelf`, `python3` available):

```
./scripts/build-runtime-assets-linux.sh x86_64
./scripts/build-runtime-assets-linux.sh aarch64
```

