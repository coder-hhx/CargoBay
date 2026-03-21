# CrateBay Platform Testing Gate
# This spec is automatically loaded by the pre-commit hook (.githooks/pre-commit Layer 6)
# and by AI Agents via AGENTS.md Rule 6.
#
# Purpose: Ensure every commit that modifies CLI or GUI code passes all three
# platform tests (macOS, Linux, Windows) before being accepted.

name: "Platform Test Gate"
version: "1.0.0"
trigger: "pre-commit"
enforcement: "blocking"   # commit is rejected if any platform fails

# Only trigger when CLI or GUI code is modified
conditions:
  path_matches:
    - "crates/cratebay-cli/**"
    - "crates/cratebay-gui/**"
    - "crates/cratebay-core/**"
  branch_exclude:
    - "master"
    - "main"

# Execution pipeline
actions:
  # Step 1: Quick local validation (must pass before spawning platform testers)
  - step: "quick-local-check"
    type: "shell"
    run: |
      cargo check --workspace --locked
      cargo test -p cratebay-cli --lib
      cargo test -p cratebay-core --lib
    timeout: 300          # 5 minutes
    continue_on_error: false

  # Step 2: Spawn three platform testers in parallel
  - step: "platform-test-macos"
    type: "agent-prompt"
    agent: "platform-tester-macos"
    prompt: "docs/prompts/platform-test-pre-commit.md"
    platform: "macos"
    timeout: 1800         # 30 minutes
    continue_on_error: false

  - step: "platform-test-linux"
    type: "agent-prompt"
    agent: "platform-tester-linux"
    prompt: "docs/prompts/platform-test-pre-commit.md"
    platform: "linux"
    timeout: 1800
    continue_on_error: false

  - step: "platform-test-windows"
    type: "agent-prompt"
    agent: "platform-tester-windows"
    prompt: "docs/prompts/platform-test-pre-commit.md"
    platform: "windows"
    timeout: 1800
    continue_on_error: false

# All three platforms must pass
pass_criteria:
  quick-local-check: "success"
  platform-test-macos: "all_tests_passed"
  platform-test-linux: "all_tests_passed"
  platform-test-windows: "all_tests_passed"

fail_action: "reject_commit"
fail_message: |
  ❌ Platform Testing Gate FAILED.
  One or more platforms did not pass all required tests.
  Review the test reports above and fix all failures before committing.
  
  To bypass (EMERGENCY ONLY): git commit --no-verify
  Note: --no-verify bypasses are logged and flagged in CI.

# Test matrix reference
test_matrix:
  cli_commands:
    - "cratebay list"
    - "cratebay info"
  gui_commands:
    # Container (9)
    - "container_templates"
    - "container_list"
    - "container_create"
    - "container_start"
    - "container_stop"
    - "container_delete"
    - "container_exec"
    - "container_exec_stream"
    - "container_logs"
    # LLM (9)
    - "llm_proxy_stream"
    - "llm_proxy_cancel"
    - "llm_provider_list"
    - "llm_provider_create"
    - "llm_provider_update"
    - "llm_provider_delete"
    - "llm_provider_test"
    - "llm_models_fetch"
    - "llm_models_list"
    - "llm_models_toggle"
    # Storage (9)
    - "settings_get"
    - "settings_update"
    - "api_key_save"
    - "api_key_delete"
    - "conversation_list"
    - "conversation_get_messages"
    - "conversation_create"
    - "conversation_delete"
    - "conversation_save_message"
    - "conversation_update_title"
    # MCP (8)
    - "mcp_server_list"
    - "mcp_server_add"
    - "mcp_server_remove"
    - "mcp_server_start"
    - "mcp_server_stop"
    - "mcp_client_call_tool"
    - "mcp_client_list_tools"
    - "mcp_export_client_config"
    # System (3)
    - "system_info"
    - "docker_status"
    - "runtime_status"
  platform_specific:
    macos:
      - "vz_runtime_start"
      - "vz_runtime_status"
      - "dmg_install_verify"
      - "macos_permissions"
      - "menu_bar_integration"
    linux:
      - "kvm_runtime_start"
      - "kvm_runtime_status"
      - "appimage_execute"
      - "deb_install_verify"
      - "docker_socket_connect"
    windows:
      - "wsl2_runtime_start"
      - "wsl2_runtime_status"
      - "msix_install_verify"
      - "uac_elevation"
      - "taskbar_integration"
