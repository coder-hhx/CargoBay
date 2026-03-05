# CrateBay v1.0 Release Smoke Checklist

> Status as of 2026-03-05: pre-GA (`coming soon`).  
> Use this checklist before GA announcement.

## 1. AI Core Scenario Gate

- Source of truth: `CrateBay-internal/notes/2026-03-05_ai_core_scenarios_draft.md`
- Automated gate:
  - `cargo test -p cratebay-gui ai_tests::assistant_core_scenarios_success_rate`
- Acceptance:
  - Success rate `>= 95%`
  - Destructive actions must always require explicit confirmation

## 2. Cross-platform Installer Smoke

### macOS

- Build artifacts:
  - `cargo build --release -p cratebay-cli -p cratebay-daemon`
  - `cd crates/cratebay-gui && npm ci && npm run build && npm run tauri build`
- Clean-machine validation:
  - Fresh install works
  - App starts and tray icon is visible
  - Containers page, VMs page, K8s page load without fatal errors
  - Uninstall + reinstall works

### Linux

- Build artifacts:
  - `cargo build --release -p cratebay-cli -p cratebay-daemon`
  - `cd crates/cratebay-gui && npm ci && npm run build && npm run tauri build`
- Clean-machine validation:
  - Fresh install works
  - App starts, no missing runtime dependency popup
  - Container and K8s read actions succeed
  - Uninstall + reinstall works

### Windows

- Build artifacts:
  - `bash scripts/build-release-windows.sh`
- Clean-machine validation:
  - MSI/NSIS installer works
  - App starts and can open Settings/Assistant pages
  - VM backend reports status (Hyper-V environment)
  - Uninstall + reinstall works

## 3. Upgrade Path Validation

- Upgrade matrix:
  - `v0.4.x -> v1.0.0-rc`
  - `v1.0.0-rc(N-1) -> v1.0.0-rc(N)`
- Verify:
  - Existing config is preserved
  - Existing VM/container metadata is readable
  - AI settings migrate without data loss
  - No plaintext API keys in config/log/crash artifacts

## 4. Final Documentation & Website Guard

- Wording guard:
  - Keep external wording as `coming soon` / `即将发布`
  - Avoid `released` / `已发布` before GA
- Required checks:
  - `./scripts/release-readiness.sh`
  - `npm run check:i18n`
  - Manual website spot-check (EN + ZH)
