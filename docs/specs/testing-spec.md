# Testing Strategy & Specification

> Version: 1.1.0 | Last Updated: 2026-03-20 | Author: frontend-architect

---

## Table of Contents

1. [Testing Pyramid](#1-testing-pyramid)
2. [Rust Testing](#2-rust-testing)
3. [Frontend Testing](#3-frontend-testing)
4. [Agent Testing Strategy](#4-agent-testing-strategy)
5. [CI/CD Pipeline Design](#5-cicd-pipeline-design)
6. [Performance Benchmarks](#6-performance-benchmarks)
7. [Security Testing](#7-security-testing)

---

## 1. Testing Pyramid

CrateBay follows a testing pyramid with emphasis on unit tests and integration tests, supplemented by targeted E2E tests.

```
                    ╱╲
                   ╱  ╲
                  ╱ E2E╲         ~10% — Critical user flows
                 ╱──────╲
                ╱        ╲
               ╱Integration╲     ~30% — Cross-layer, Tauri commands
              ╱──────────────╲
             ╱                ╲
            ╱    Unit Tests    ╲  ~60% — Functions, components, stores
           ╱────────────────────╲
```

### Test Distribution Targets

| Layer | Coverage Target | Technology | Focus |
|-------|----------------|------------|-------|
| Unit (Rust) | 80%+ | cargo test | Core logic, validation, parsing |
| Unit (Frontend) | 75%+ | Vitest | Stores, hooks, utilities, components |
| Integration | 60%+ | cargo test + Vitest | Tauri commands, store ↔ backend |
| E2E | Critical paths | Playwright | Chat flow, container management |
| Performance | Benchmarks pass | Criterion | Binary size, startup, memory |

---

## 2. Rust Testing

### 2.1 Unit Tests

Unit tests live alongside source code in the same file:

```rust
// crates/cratebay-core/src/validation.rs

pub fn validate_container_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::Empty("container name"));
    }
    if name.len() > 64 {
        return Err(ValidationError::TooLong("container name", 64));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(ValidationError::InvalidChars("container name"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_container_names() {
        assert!(validate_container_name("my-container").is_ok());
        assert!(validate_container_name("node_dev_01").is_ok());
        assert!(validate_container_name("a").is_ok());
    }

    #[test]
    fn test_empty_name_rejected() {
        assert!(matches!(
            validate_container_name(""),
            Err(ValidationError::Empty(_))
        ));
    }

    #[test]
    fn test_long_name_rejected() {
        let long_name = "a".repeat(65);
        assert!(matches!(
            validate_container_name(&long_name),
            Err(ValidationError::TooLong(_, 64))
        ));
    }

    #[test]
    fn test_special_chars_rejected() {
        assert!(validate_container_name("my container").is_err());
        assert!(validate_container_name("../escape").is_err());
        assert!(validate_container_name("name;drop").is_err());
    }
}
```

### 2.2 Integration Tests

Integration tests live in `tests/` directories at the crate level. They test cross-module and external interactions.

```
crates/
├── cratebay-core/
│   ├── src/
│   └── tests/
│       ├── docker_integration.rs    # Tests with real Docker (requires Docker)
│       ├── storage_integration.rs   # SQLite tests with temp databases
│       └── mcp_client_test.rs       # MCP client with mock server
├── cratebay-gui/
│   └── src-tauri/
│       └── tests/
│           └── commands_test.rs     # Tauri command integration tests
└── cratebay-mcp/
    └── tests/
        └── mcp_server_test.rs       # MCP protocol compliance tests
```

**Docker integration tests** require Docker and are gated:

```rust
// tests/docker_integration.rs
#[cfg(feature = "integration-tests")]
mod docker_tests {
    use cratebay_core::docker::DockerClient;

    #[tokio::test]
    async fn test_container_lifecycle() {
        let client = DockerClient::connect().await
            .expect("Docker must be running for integration tests");

        // Create
        let container = client.create_sandbox("node-dev", None).await.unwrap();
        assert_eq!(container.status, "created");

        // Start
        client.start_container(&container.id).await.unwrap();
        let info = client.inspect(&container.id).await.unwrap();
        assert_eq!(info.status, "running");

        // Execute
        let output = client.exec(&container.id, "echo hello").await.unwrap();
        assert_eq!(output.trim(), "hello");

        // Stop and delete
        client.stop_container(&container.id).await.unwrap();
        client.delete_container(&container.id).await.unwrap();
    }
}
```

**Run integration tests:**

```bash
# Unit tests only (fast, no Docker needed)
cargo test --workspace

# Include integration tests (requires Docker)
cargo test --workspace --features integration-tests
```

### 2.3 Benchmark Tests (Criterion)

Performance-critical code is benchmarked with Criterion:

```rust
// crates/cratebay-core/benches/storage_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cratebay_core::storage::Storage;

fn bench_message_insert(c: &mut Criterion) {
    let storage = Storage::open_in_memory().unwrap();
    let session_id = storage.create_session("bench").unwrap();

    c.bench_function("insert_message", |b| {
        b.iter(|| {
            storage.save_message(
                black_box(&session_id),
                black_box("user"),
                black_box("Hello, world!"),
            ).unwrap();
        })
    });
}

fn bench_message_query(c: &mut Criterion) {
    let storage = Storage::open_in_memory().unwrap();
    let session_id = storage.create_session("bench").unwrap();

    // Insert 1000 messages
    for i in 0..1000 {
        storage.save_message(&session_id, "user", &format!("Message {i}")).unwrap();
    }

    c.bench_function("query_1000_messages", |b| {
        b.iter(|| {
            storage.get_messages(black_box(&session_id)).unwrap();
        })
    });
}

criterion_group!(benches, bench_message_insert, bench_message_query);
criterion_main!(benches);
```

**Run benchmarks:**

```bash
cargo bench -p cratebay-core
```

### 2.4 Coverage (cargo-llvm-cov)

```bash
# Generate coverage report
cargo llvm-cov --workspace --html

# Check coverage threshold (CI gate)
cargo llvm-cov --workspace --fail-under-lines 70
```

Coverage reports are generated in `target/llvm-cov/html/`.

---

## 3. Frontend Testing

### 3.1 Vitest Unit Tests

Unit tests for stores, hooks, and utilities:

```typescript
// stores/__tests__/chatStore.test.ts
import { describe, it, expect, beforeEach } from "vitest";
import { useChatStore } from "../chatStore";

describe("chatStore", () => {
  beforeEach(() => {
    useChatStore.setState(useChatStore.getInitialState());
  });

  it("creates a new session", () => {
    const session = useChatStore.getState().createSession();
    expect(session.id).toBeDefined();
    expect(session.title).toBe("New Chat");
    expect(useChatStore.getState().sessions).toHaveLength(1);
  });

  it("adds messages to a session", () => {
    const session = useChatStore.getState().createSession();
    useChatStore.getState().addMessage(session.id, {
      id: "msg-1",
      sessionId: session.id,
      role: "user",
      content: "Hello",
      timestamp: new Date().toISOString(),
      status: "complete",
    });

    const messages = useChatStore.getState().messages[session.id];
    expect(messages).toHaveLength(1);
    expect(messages[0].content).toBe("Hello");
  });

  it("appends stream chunks", () => {
    const session = useChatStore.getState().createSession();
    useChatStore.getState().addMessage(session.id, {
      id: "msg-1",
      sessionId: session.id,
      role: "assistant",
      content: "",
      timestamp: new Date().toISOString(),
      status: "streaming",
    });

    useChatStore.getState().appendStreamChunk(session.id, "msg-1", "Hello ");
    useChatStore.getState().appendStreamChunk(session.id, "msg-1", "world");

    const messages = useChatStore.getState().messages[session.id];
    expect(messages[0].content).toBe("Hello world");
  });
});
```

### 3.2 Component Tests

Component tests render React components in isolation with Vitest + React Testing Library:

```typescript
// components/chat/__tests__/ToolCallCard.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ToolCallCard } from "../ToolCallCard";

describe("ToolCallCard", () => {
  it("shows running state with spinner", () => {
    render(
      <ToolCallCard
        toolCall={{
          id: "tc-1",
          toolName: "container_create",
          toolLabel: "Create Container",
          parameters: { templateId: "node-dev" },
          status: "running",
          startedAt: new Date().toISOString(),
        }}
      />
    );

    expect(screen.getByText("Create Container")).toBeInTheDocument();
    expect(screen.getByText("Executing...")).toBeInTheDocument();
    expect(screen.getByRole("progressbar")).toBeInTheDocument();
  });

  it("shows success state with result", () => {
    render(
      <ToolCallCard
        toolCall={{
          id: "tc-1",
          toolName: "container_create",
          toolLabel: "Create Container",
          parameters: { templateId: "node-dev" },
          status: "success",
          result: { id: "abc123", name: "node-01" },
          startedAt: "2026-03-20T10:00:00Z",
          completedAt: "2026-03-20T10:00:02Z",
        }}
      />
    );

    expect(screen.getByText("Create Container")).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("shows error state with message", () => {
    render(
      <ToolCallCard
        toolCall={{
          id: "tc-1",
          toolName: "container_create",
          toolLabel: "Create Container",
          parameters: { templateId: "node-dev" },
          status: "error",
          error: "Docker socket not found",
          startedAt: "2026-03-20T10:00:00Z",
          completedAt: "2026-03-20T10:00:01Z",
        }}
      />
    );

    expect(screen.getByText("Docker socket not found")).toBeInTheDocument();
  });
});
```

### 3.3 E2E Tests (Playwright)

E2E tests cover critical user flows in the actual Tauri application:

```typescript
// tests/e2e/chat-flow.spec.ts
import { test, expect } from "@playwright/test";

test.describe("Chat Flow", () => {
  test("user can send a message and receive a response", async ({ page }) => {
    // Navigate to chat page (default)
    await page.goto("/");
    await expect(page.locator("[data-testid=chat-input]")).toBeVisible();

    // Type and send a message
    await page.fill("[data-testid=chat-input]", "List my containers");
    await page.keyboard.press("Enter");

    // Wait for agent response
    await expect(page.locator("[data-testid=message-bubble]").last()).toContainText(
      /container/i,
      { timeout: 30000 }
    );
  });

  test("destructive operation shows confirmation dialog", async ({ page }) => {
    await page.goto("/");

    // Send a delete command
    await page.fill("[data-testid=chat-input]", "Delete container node-01");
    await page.keyboard.press("Enter");

    // Confirm dialog should appear
    await expect(page.locator("[data-testid=confirm-dialog]")).toBeVisible({
      timeout: 30000,
    });
    await expect(page.locator("[data-testid=confirm-dialog]")).toContainText(
      "permanently deleted"
    );
  });
});

test.describe("Container Management", () => {
  test("user can view container list", async ({ page }) => {
    await page.goto("/");

    // Navigate to containers page
    await page.click("[data-testid=nav-containers]");

    // Container list should load
    await expect(page.locator("[data-testid=container-list]")).toBeVisible();
  });
});
```

**Playwright configuration:**

```typescript
// playwright.config.ts
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 60000,
  retries: 1,
  use: {
    baseURL: "http://localhost:1420", // Tauri dev server
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  webServer: {
    command: "pnpm tauri dev",
    port: 1420,
    reuseExistingServer: true,
    timeout: 120000,
  },
});
```

### 3.4 Coverage

```bash
# Vitest coverage
pnpm test --coverage

# Coverage thresholds in vitest.config.ts
export default defineConfig({
  test: {
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov"],
      thresholds: {
        lines: 70,
        functions: 70,
        branches: 60,
        statements: 70,
      },
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/components/ui/**",  // shadcn auto-generated
        "src/**/*.test.{ts,tsx}",
        "src/types/**",
      ],
    },
  },
});
```

---

## 4. Agent Testing Strategy

Testing the AI agent pipeline requires specialized approaches since it involves LLM interactions, tool execution, and streaming.

### 4.1 Mock LLM (Mock pi-ai streamFn)

For deterministic testing, replace the real LLM with a mock `streamFn`:

```typescript
// tests/mocks/mockStreamFn.ts
import type { StreamFn, StreamMessage } from "@mariozechner/pi-agent-core";

interface MockResponse {
  content: string;
  toolCalls?: Array<{
    name: string;
    arguments: Record<string, unknown>;
  }>;
}

export function createMockStreamFn(
  responses: MockResponse[]
): StreamFn {
  let responseIndex = 0;

  return async function* mockStreamFn(
    messages: StreamMessage[]
  ): AsyncGenerator<{ delta: string }> {
    const response = responses[responseIndex % responses.length];
    responseIndex++;

    // Simulate streaming by yielding character by character
    if (response.content) {
      for (const char of response.content) {
        yield { delta: char };
        await new Promise((r) => setTimeout(r, 1));
      }
    }

    // Simulate tool calls
    if (response.toolCalls) {
      for (const tc of response.toolCalls) {
        yield {
          delta: JSON.stringify({
            type: "tool_call",
            function: {
              name: tc.name,
              arguments: JSON.stringify(tc.arguments),
            },
          }),
        };
      }
    }
  };
}
```

**Usage in tests:**

```typescript
// tests/agent/agent-flow.test.ts
import { describe, it, expect } from "vitest";
import { Agent } from "@mariozechner/pi-agent-core";
import { createMockStreamFn } from "../mocks/mockStreamFn";
import { allTools } from "@/tools";

describe("Agent Flow", () => {
  it("agent calls container_list when asked about containers", async () => {
    const mockStream = createMockStreamFn([
      {
        content: "",
        toolCalls: [{ name: "container_list", arguments: { status: "all" } }],
      },
      {
        content: "Here are your containers: ...",
      },
    ]);

    const agent = new Agent({
      streamFn: mockStream,
      tools: allTools,
      systemPrompt: "You are a test assistant.",
    });

    const result = await agent.run("Show me my containers");
    expect(result.toolCalls).toContainEqual(
      expect.objectContaining({ name: "container_list" })
    );
  });
});
```

### 4.2 Tool Execution Tests (Mock Tauri invoke)

Mock Tauri's `invoke` to test tool execution without a real backend:

```typescript
// tests/mocks/mockTauri.ts
import { vi } from "vitest";

const mockResponses: Record<string, unknown> = {
  container_list: [
    { id: "abc123", shortId: "abc123", name: "node-01", status: "running" },
    { id: "def456", shortId: "def456", name: "py-dev", status: "stopped" },
  ],
  container_create: {
    id: "ghi789",
    shortId: "ghi789",
    name: "new-container",
    status: "running",
  },
};

export function setupMockTauri() {
  vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn((command: string, args?: Record<string, unknown>) => {
      if (command in mockResponses) {
        return Promise.resolve(mockResponses[command]);
      }
      return Promise.reject(new Error(`Unknown command: ${command}`));
    }),
  }));
}
```

**Tool execution tests:**

```typescript
// tools/__tests__/containerTools.test.ts
import { describe, it, expect, beforeAll } from "vitest";
import { setupMockTauri } from "../../tests/mocks/mockTauri";
import { containerListTool } from "../containerTools";

beforeAll(() => {
  setupMockTauri();
});

describe("containerListTool", () => {
  it("returns formatted container list", async () => {
    const result = await containerListTool.execute(
      { status: "all" },
      { onUpdate: vi.fn() }
    );

    expect(result).toMatchObject({
      count: 2,
      containers: expect.arrayContaining([
        expect.objectContaining({ name: "node-01", status: "running" }),
      ]),
    });
  });

  it("reports progress via onUpdate", async () => {
    const onUpdate = vi.fn();
    await containerListTool.execute({ status: "all" }, { onUpdate });

    expect(onUpdate).toHaveBeenCalledWith(
      expect.objectContaining({
        type: "status",
        message: expect.stringContaining("Fetching"),
      })
    );
  });
});
```

### 4.3 Golden File Tests

Golden file tests verify that tool outputs remain stable across changes:

```typescript
// tests/golden/container-list.test.ts
import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { join } from "path";

describe("Golden File Tests", () => {
  it("container_list output matches golden file", async () => {
    const result = await containerListTool.execute({ status: "all" }, {});

    const goldenPath = join(__dirname, "golden-files", "container-list.json");
    const golden = JSON.parse(readFileSync(goldenPath, "utf-8"));

    expect(result).toMatchObject(golden);
  });
});
```

Golden files are stored in `tests/golden/golden-files/` and updated explicitly:

```bash
# Update golden files
UPDATE_GOLDEN=1 pnpm test tests/golden/
```

### 4.4 Canary Tests (Non-blocking Real LLM)

Canary tests use a real LLM to detect regressions in agent behavior. They run in CI but are **non-blocking** (failures are reported but don't fail the build).

```typescript
// tests/canary/real-llm.test.ts
import { describe, it, expect } from "vitest";

describe.skipIf(!process.env.LLM_API_KEY)("Canary: Real LLM", () => {
  it("agent responds coherently to basic container question", async () => {
    const agent = createRealAgent(); // uses actual LLM API

    const result = await agent.run("What containers can I create?");

    // Loose assertions — we just check the response is reasonable
    expect(result.content).toBeTruthy();
    expect(result.content.length).toBeGreaterThan(50);
    expect(result.content.toLowerCase()).toMatch(/template|container|node|python|rust/);
  }, 30000);

  it("agent uses container_list tool when asked", async () => {
    const agent = createRealAgent();

    const result = await agent.run("Show me running containers");

    expect(result.toolCalls).toContainEqual(
      expect.objectContaining({
        name: expect.stringMatching(/container_list/),
      })
    );
  }, 30000);
});
```

---

## 5. CI/CD Pipeline Design

### 5.1 ci.yml — Main CI Pipeline

Triggered on: push to `rewrite/v2`, pull requests to `rewrite/v2` and `master`.

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [rewrite/v2]
  pull_request:
    branches: [rewrite/v2, master]

jobs:
  # Stage 1: Quick checks (parallel)
  check:
    strategy:
      matrix:
        os: [macos-latest, ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check --workspace

  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo clippy --workspace -- -D warnings

  frontend-lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - run: pnpm lint
      - run: pnpm type-check

  # Stage 2: Tests (after checks pass)
  rust-test:
    needs: [check, fmt, clippy]
    strategy:
      matrix:
        os: [macos-latest, ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace

  frontend-test:
    needs: [frontend-lint]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - run: pnpm test --coverage

  # Stage 3: Size and performance checks
  size-check:
    needs: [rust-test]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release -p cratebay-gui
      - name: Check binary size
        run: |
          SIZE=$(stat -c%s target/release/cratebay-gui 2>/dev/null || stat -f%z target/release/cratebay-gui)
          MAX_SIZE=$((20 * 1024 * 1024))  # 20MB
          if [ "$SIZE" -gt "$MAX_SIZE" ]; then
            echo "Binary size ${SIZE} exceeds budget of ${MAX_SIZE}"
            exit 1
          fi

  perf-bench:
    needs: [rust-test]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo bench -p cratebay-core -- --output-format bencher | tee bench-output.txt
      - name: Archive benchmark results
        uses: actions/upload-artifact@v4
        with:
          name: bench-results
          path: bench-output.txt

  # Stage 4: Canary tests (non-blocking)
  canary:
    needs: [rust-test, frontend-test]
    runs-on: ubuntu-latest
    continue-on-error: true  # Non-blocking
    if: github.event_name == 'push'  # Only on push, not PR
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - run: pnpm test tests/canary/
        env:
          LLM_API_KEY: ${{ secrets.CANARY_LLM_API_KEY }}
```

### 5.2 Pipeline Flow

```
Push / PR
    │
    ├── check (macOS, Linux, Windows)  ──┐
    ├── fmt                              ├── rust-test (3 OS) ──┐
    ├── clippy                        ───┘                      ├── size-check
    │                                                           ├── perf-bench
    └── frontend-lint ──── frontend-test ───────────────────────┤
                                                                └── canary (non-blocking)
```

### 5.3 CI Performance Optimizations

- **Cargo cache**: Use `actions/cache` for `~/.cargo/registry` and `target/`
- **pnpm cache**: Built-in `actions/setup-node` cache
- **Parallel jobs**: Check stages run in parallel across OS matrix
- **Conditional canary**: Only runs on push (not PR) to save API costs

---

## 6. Performance Benchmarks

### 6.1 Budget

| Metric | Budget | Measured By |
|--------|--------|-------------|
| Binary size (release) | <20MB | `stat` in CI |
| App startup time | <3s | Criterion bench + manual test |
| Idle RAM usage | <200MB | `ps` monitoring |
| Chat response latency (first token) | <500ms* | E2E test timer |
| Container list API | <200ms | Criterion bench |
| SQLite query (1000 messages) | <50ms | Criterion bench |

*Excluding LLM network latency — measures time from invoke to first Tauri Event.

### 6.2 Benchmark Script

```bash
#!/bin/bash
# scripts/bench-perf.sh

set -e
echo "=== CrateBay Performance Validation ==="

# 1. Binary size
echo "Checking binary size..."
cargo build --release -p cratebay-gui 2>/dev/null
BINARY="target/release/cratebay-gui"
if [ -f "$BINARY" ]; then
    SIZE=$(stat -f%z "$BINARY" 2>/dev/null || stat -c%s "$BINARY")
    MAX=$((20 * 1024 * 1024))
    if [ "$SIZE" -gt "$MAX" ]; then
        echo "FAIL: Binary size $(($SIZE / 1024 / 1024))MB exceeds 20MB budget"
        exit 1
    fi
    echo "PASS: Binary size $(($SIZE / 1024 / 1024))MB"
fi

# 2. Run Criterion benchmarks
echo "Running benchmarks..."
cargo bench -p cratebay-core 2>&1 | tee /tmp/bench-output.txt

# 3. Check frontend bundle size
echo "Checking frontend bundle..."
cd crates/cratebay-gui
pnpm build 2>/dev/null
BUNDLE_SIZE=$(du -sk dist/ | cut -f1)
MAX_BUNDLE=2048  # 2MB
if [ "$BUNDLE_SIZE" -gt "$MAX_BUNDLE" ]; then
    echo "FAIL: Frontend bundle ${BUNDLE_SIZE}KB exceeds 2MB budget"
    exit 1
fi
echo "PASS: Frontend bundle ${BUNDLE_SIZE}KB"

echo "=== All performance checks passed ==="
```

### 6.3 Monitoring in Development

During development, use the Tauri MCP Server's IPC monitoring tools to track performance:

- `tauri_ipc_start_capture` → capture all invoke calls with timing
- Analyze results for slow commands
- Set up alerts for commands exceeding 200ms

---

## 7. Security Testing

### 7.1 Sandbox Escape Attempts

Test that container operations are properly sandboxed:

```rust
#[cfg(test)]
mod sandbox_security_tests {
    #[tokio::test]
    async fn test_exec_cannot_access_host_filesystem() {
        let client = DockerClient::connect().await.unwrap();
        let container = client.create_sandbox("ubuntu-base", None).await.unwrap();
        client.start_container(&container.id).await.unwrap();

        // Attempt to read host files (should see container filesystem, not host)
        let result = client.exec(&container.id, "cat /etc/hostname").await.unwrap();
        assert_ne!(result.trim(), get_host_hostname());

        client.delete_container(&container.id).await.unwrap();
    }

    #[tokio::test]
    async fn test_exec_cannot_mount_host_volumes() {
        let client = DockerClient::connect().await.unwrap();

        // Create request with volume mount attempt (should be rejected)
        let result = client.create_sandbox_with_options(
            "ubuntu-base",
            CreateOptions {
                volumes: vec!["/:/host:rw".to_string()], // Attempt host mount
                ..Default::default()
            },
        ).await;

        assert!(result.is_err(), "Host volume mounts must be rejected");
    }
}
```

### 7.2 Path Traversal Tests

Test the MCP server's path validation:

```rust
#[cfg(test)]
mod path_traversal_tests {
    use super::validate_path;
    use std::path::Path;

    #[test]
    fn test_basic_traversal_rejected() {
        let root = Path::new("/workspace");
        assert!(validate_path("../etc/passwd", root).is_err());
        assert!(validate_path("../../root/.ssh/id_rsa", root).is_err());
        assert!(validate_path("foo/../../etc/shadow", root).is_err());
    }

    #[test]
    fn test_encoded_traversal_rejected() {
        let root = Path::new("/workspace");
        assert!(validate_path("..%2Fetc%2Fpasswd", root).is_err());
        assert!(validate_path("%2e%2e/etc/passwd", root).is_err());
    }

    #[test]
    fn test_symlink_traversal_rejected() {
        let root = Path::new("/workspace");
        // After canonicalization, symlinks that escape root are caught
        // This test requires a temp directory with actual symlinks
    }

    #[test]
    fn test_valid_paths_accepted() {
        let root = Path::new("/workspace");
        assert!(validate_path("src/main.rs", root).is_ok());
        assert!(validate_path("deep/nested/path/file.txt", root).is_ok());
        assert!(validate_path("file-with-dashes.md", root).is_ok());
    }
}
```

### 7.3 API Key Leakage Prevention

Test that API keys never reach the frontend:

```typescript
// tests/security/api-key-leakage.test.ts
import { describe, it, expect } from "vitest";
import { setupMockTauri } from "../mocks/mockTauri";

describe("API Key Security", () => {
  it("settingsStore never exposes API key values", () => {
    const state = useSettingsStore.getState();

    // Providers should have hasApiKey flag, not the actual key
    for (const provider of state.providers) {
      expect(provider).not.toHaveProperty("apiKey");
      expect(provider).toHaveProperty("hasApiKey");
    }
  });

  it("saveApiKey sends key to backend and never stores locally", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const spy = vi.spyOn({ invoke }, "invoke");

    await useSettingsStore.getState().saveApiKey("provider-1", "sk-test-key");

    // Key was sent to backend
    expect(spy).toHaveBeenCalledWith(
      "api_key_save",
      expect.objectContaining({ providerId: "provider-1" })
    );

    // Key is not stored in Zustand state
    const state = useSettingsStore.getState();
    expect(JSON.stringify(state)).not.toContain("sk-test-key");
  });

  it("LLM stream tokens do not contain API keys", async () => {
    // Subscribe to all Tauri events during a stream
    const events: unknown[] = [];
    const unlisten = await listen("llm:token:*", (e) => events.push(e));

    // Trigger a streaming response
    await invoke("llm_proxy_stream", {
      sessionId: "test",
      messages: [{ role: "user", content: "Hello" }],
    });

    // No event payload should contain API key patterns
    const allPayloads = JSON.stringify(events);
    expect(allPayloads).not.toMatch(/sk-[a-zA-Z0-9]{20,}/);
    expect(allPayloads).not.toMatch(/key-[a-zA-Z0-9]{20,}/);

    unlisten();
  });
});
```

### 7.4 Security Test Checklist

| Category | Test | Priority |
|----------|------|----------|
| Sandbox | Container cannot access host filesystem | P0 |
| Sandbox | Container cannot mount host volumes | P0 |
| Sandbox | Container resource limits enforced (CPU/memory) | P1 |
| Path | Path traversal via `../` rejected | P0 |
| Path | URL-encoded traversal rejected | P0 |
| Path | Symlink escape from workspace root rejected | P1 |
| API Key | Keys never in frontend state or events | P0 |
| API Key | Keys encrypted at rest in SQLite | P0 |
| API Key | Keys not logged in audit entries | P0 |
| MCP | Destructive MCP tools require confirmation | P1 |
| MCP | MCP server process isolation (no shared state) | P1 |
| Input | SQL injection in storage layer prevented | P0 |
| Input | XSS in rendered markdown prevented | P1 |
