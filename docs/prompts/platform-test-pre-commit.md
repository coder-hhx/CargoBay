# Pre-Commit Platform Testing Prompt

> **Version**: 1.0.0 | **Trigger**: `.codebuddy/hooks/.pre-commit.spec` | **AGENTS.md Rule 6**
>
> This prompt is executed by platform-specific tester agents before every commit
> that modifies `crates/cratebay-cli/`, `crates/cratebay-gui/`, or `crates/cratebay-core/`.

---

## Your Role

You are `platform-tester-{PLATFORM}` (where PLATFORM is one of: macos, linux, windows).
Your job is to verify that ALL CrateBay CLI and GUI functionality works correctly on your platform.

---

## Mandatory Steps

### Step 1: Environment Verification

Verify your test environment:

| Check | Expected |
|-------|----------|
| Platform matches your role | macOS / Linux / Windows |
| Rust toolchain installed | `rustc --version` succeeds |
| Cargo workspace compiles | `cargo check --workspace` succeeds |
| Node.js / pnpm available | `pnpm --version` succeeds |
| Frontend builds | `cd crates/cratebay-gui && pnpm build` succeeds |

### Step 2: Compile Verification

```bash
# Must pass with zero errors
cargo build --workspace
cargo build --release -p cratebay-cli
```

### Step 3: CLI Command Tests (5 test cases — ALL must pass)

| ID | Command | Test | Expected Result |
|----|---------|------|----------------|
| CLI-001 | `cratebay list` | No containers running | Empty list or "No containers" message |
| CLI-002 | `cratebay list` | With containers (if Docker available) | Correct table/JSON output |
| CLI-003 | `cratebay list --help` | Help output | Shows usage, options, aliases |
| CLI-004 | `cratebay info` | System information | Shows OS, arch, memory, disk, runtime status |
| CLI-005 | `cratebay --version` | Version check | Outputs `cratebay 2.0.0-alpha.1` |

### Step 4: Backend Unit Tests

```bash
# All tests must pass
cargo test -p cratebay-core -- --test-threads=1
cargo test -p cratebay-cli
cargo test -p cratebay-gui
cargo test -p cratebay-mcp
```

### Step 5: GUI Tauri Command Tests (30 commands)

Test each Tauri command's basic functionality. For commands that require external services
(Docker, LLM API), verify graceful error handling when the service is unavailable.

#### Container Commands (9)
| Command | Test | Expected |
|---------|------|----------|
| `container_templates` | List templates | Returns template array (may be empty) |
| `container_list` | List containers | Returns container array |
| `container_create` | Create with valid template | Returns container ID or graceful error if Docker unavailable |
| `container_start` | Start existing container | Success or "Docker not available" |
| `container_stop` | Stop running container | Success or graceful error |
| `container_delete` | Delete stopped container | Success or graceful error |
| `container_exec` | Execute command | Returns output or error |
| `container_exec_stream` | Stream exec output | Stream events or error |
| `container_logs` | Get container logs | Returns logs or error |

#### LLM Commands (9)
| Command | Test | Expected |
|---------|------|----------|
| `llm_provider_list` | List providers | Returns provider array (may be empty) |
| `llm_provider_create` | Create provider | Returns provider ID |
| `llm_provider_update` | Update provider | Success |
| `llm_provider_delete` | Delete provider | Success |
| `llm_provider_test` | Test connection | Success or "API key invalid" |
| `llm_models_fetch` | Fetch models | Returns model list or error |
| `llm_models_list` | List cached models | Returns cached model array |
| `llm_models_toggle` | Toggle model | Success |
| `llm_proxy_stream` | Stream completion | Token events or error |
| `llm_proxy_cancel` | Cancel stream | Cancellation acknowledged |

#### Storage Commands (9)
| Command | Test | Expected |
|---------|------|----------|
| `settings_get` | Get settings | Returns settings object |
| `settings_update` | Update setting | Success |
| `api_key_save` | Save API key | Success (encrypted) |
| `api_key_delete` | Delete API key | Success |
| `conversation_list` | List conversations | Returns array |
| `conversation_create` | Create conversation | Returns conversation ID |
| `conversation_get_messages` | Get messages | Returns message array |
| `conversation_save_message` | Save message | Success |
| `conversation_delete` | Delete conversation | Success |
| `conversation_update_title` | Update title | Success |

#### MCP Commands (8)
| Command | Test | Expected |
|---------|------|----------|
| `mcp_server_list` | List MCP servers | Returns server config array |
| `mcp_server_add` | Add server | Success |
| `mcp_server_remove` | Remove server | Success |
| `mcp_server_start` | Start server | Success or connection error |
| `mcp_server_stop` | Stop server | Success |
| `mcp_client_list_tools` | List tools | Returns tool array |
| `mcp_client_call_tool` | Call tool | Returns result or error |
| `mcp_export_client_config` | Export config | Returns JSON config |

#### System Commands (3)
| Command | Test | Expected |
|---------|------|----------|
| `system_info` | Get system info | Returns OS, arch, memory, disk |
| `docker_status` | Get Docker status | Returns connected/disconnected |
| `runtime_status` | Get runtime status | Returns runtime state |

### Step 6: Platform-Specific Tests

#### macOS Only (platform-tester-macos)
| ID | Test | Expected |
|----|------|----------|
| MAC-001 | VZ.framework runtime initialization | Runtime state reported correctly |
| MAC-002 | DMG bundle contains icon.icns | icon.icns present in Resources/ |
| MAC-003 | App codesign verification | `codesign --verify` succeeds (or unsigned warning) |
| MAC-004 | Notarization readiness | Bundle structure valid |
| MAC-005 | File system permission dialogs | Graceful handling without crash |
| MAC-006 | Menu bar presence | App appears in menu bar |
| MAC-007 | Dock icon | Correct icon displayed |
| MAC-008 | Window management | Resize, minimize, fullscreen work |
| MAC-009 | macOS notification | Notification delivery |
| MAC-010 | Universal binary check | Architecture matches host |

#### Linux Only (platform-tester-linux)
| ID | Test | Expected |
|----|------|----------|
| LNX-001 | KVM/QEMU runtime initialization | Runtime state reported correctly |
| LNX-002 | Docker socket connection | Connect to /var/run/docker.sock |
| LNX-003 | AppImage execution | AppImage launches correctly |
| LNX-004 | DEB package install | dpkg -i succeeds |
| LNX-005 | Desktop file integration | .desktop file created |
| LNX-006 | Icon theme integration | Icon visible in launcher |
| LNX-007 | X11/Wayland compatibility | Window renders correctly |
| LNX-008 | System tray (if supported) | Tray icon displayed |
| LNX-009 | File permissions | App files have correct permissions |
| LNX-010 | SELinux/AppArmor compat | No security denials |

#### Windows Only (platform-tester-windows)
| ID | Test | Expected |
|----|------|----------|
| WIN-001 | WSL2 runtime initialization | Runtime state reported correctly |
| WIN-002 | WSL2 process communication | IPC succeeds |
| WIN-003 | MSIX installer | Installation completes |
| WIN-004 | Start menu entry | App appears in Start menu |
| WIN-005 | Taskbar integration | Taskbar icon displayed |
| WIN-006 | Windows notification | Toast notification delivery |
| WIN-007 | UAC elevation (if needed) | Graceful handling |
| WIN-008 | High DPI rendering | UI scales correctly |
| WIN-009 | Windows Defender compat | No false positive blocks |
| WIN-010 | File path handling | Long paths and spaces handled |

### Step 7: Frontend Tests (if applicable)

```bash
cd crates/cratebay-gui
pnpm run lint
pnpm run build
pnpm run test:unit
# E2E tests only if display server available
pnpm run test:e2e || echo "E2E skipped (no display)"
```

---

## Report Format (MANDATORY)

After completing all tests, output your report in this exact format:

```
═══════════════════════════════════════════════════
  PLATFORM TEST REPORT — {PLATFORM}
═══════════════════════════════════════════════════
  Platform:     {macOS|Linux|Windows}
  Agent:        platform-tester-{platform}
  Timestamp:    {ISO 8601}
  Commit:       {git short hash}
═══════════════════════════════════════════════════

  COMPILATION
    cargo check --workspace    : {PASS|FAIL}
    cargo build --release      : {PASS|FAIL}

  CLI TESTS (5)
    Passed: {N}/5
    Failed: {list of failed IDs}

  BACKEND UNIT TESTS
    cratebay-core              : {N passed}/{N total}
    cratebay-cli               : {N passed}/{N total}
    cratebay-gui               : {N passed}/{N total}
    cratebay-mcp               : {N passed}/{N total}

  GUI TAURI COMMANDS (30)
    Container (9)              : {N}/9 passed
    LLM (9)                    : {N}/9 passed
    Storage (9)                : {N}/9 passed
    MCP (8)                    : {N}/8 passed
    System (3)                 : {N}/3 passed

  PLATFORM-SPECIFIC ({PLATFORM}, 10)
    Passed: {N}/10
    Failed: {list of failed IDs}

  FRONTEND
    Lint                       : {PASS|FAIL}
    Build                      : {PASS|FAIL}
    Unit Tests                 : {N passed}/{N total}
    E2E Tests                  : {N passed}/{N total} or SKIPPED

═══════════════════════════════════════════════════
  OVERALL: {PASS|FAIL}
═══════════════════════════════════════════════════
```

---

## Pass Criteria

**PASS** requires ALL of the following:
- Compilation: zero errors
- CLI tests: 5/5 passed
- Backend unit tests: zero failures
- GUI Tauri commands: 30/30 passed (graceful error handling counts as pass)
- Platform-specific: 8/10 minimum (2 allowed to be SKIPPED if hardware unavailable)
- Frontend: lint + build + unit tests all pass

**FAIL** if any of the above criteria is not met. Report the exact failures.

---

## Automation Notes

- This prompt is referenced by `.codebuddy/hooks/.pre-commit.spec`
- AGENTS.md Rule 6 mandates this testing for every commit
- Three platform testers run in parallel to minimize total time
- Results are aggregated by the pre-commit hook — ALL three must PASS
