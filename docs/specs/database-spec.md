# Database Specification

> Version: 1.1.0 | Last Updated: 2026-03-20 | Author: architect

---

## Table of Contents

1. [Storage Location](#1-storage-location)
2. [Schema Design](#2-schema-design)
3. [Migration Strategy](#3-migration-strategy)
4. [Encryption](#4-encryption)
5. [Data Lifecycle](#5-data-lifecycle)
6. [Backup & Export](#6-backup--export)

---

## 1. Storage Location

### 1.1 Database Path

```
~/.cratebay/cratebay.db
```

| Platform | Resolved Path |
|----------|---------------|
| macOS | `/Users/<user>/.cratebay/cratebay.db` |
| Linux | `/home/<user>/.cratebay/cratebay.db` |
| Windows | `C:\Users\<user>\.cratebay\cratebay.db` |

### 1.2 Directory Structure

```
~/.cratebay/
├── cratebay.db           # Main SQLite database
├── cratebay.db-wal       # WAL journal (auto-created by SQLite)
├── cratebay.db-shm       # Shared memory (auto-created by SQLite)
├── runtime/              # Built-in runtime files
├── logs/                 # Application logs
└── backups/              # Database backups
    └── cratebay-2026-03-20.db
```

### 1.3 SQLite Configuration

```rust
pub fn init(path: &Path) -> Result<Connection, AppError> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;

    // Performance pragmas
    conn.execute_batch("
        PRAGMA journal_mode = WAL;          -- Write-Ahead Logging for concurrency
        PRAGMA synchronous = NORMAL;        -- Balance durability and performance
        PRAGMA foreign_keys = ON;           -- Enforce FK constraints
        PRAGMA busy_timeout = 5000;         -- 5s timeout on lock contention
        PRAGMA cache_size = -2000;          -- 2MB page cache
        PRAGMA temp_store = MEMORY;         -- In-memory temp tables
    ")?;

    run_migrations(&conn)?;
    Ok(conn)
}
```

---

## 2. Schema Design

### 2.1 Entity-Relationship Overview

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  ai_providers│     │   api_keys   │     │   settings   │
│              │←───→│  (encrypted) │     │  (key-value) │
└──────────────┘     └──────────────┘     └──────────────┘
       │
       │ 1:N
       ▼
┌──────────────┐
│   ai_models  │
│ (per-provider│
│  model list) │
└──────────────┘

┌──────────────┐     ┌──────────────┐
│conversations │←───→│   messages   │
│              │  1:N │              │
└──────────────┘     └──────────────┘

┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  container   │     │  mcp_servers │     │  audit_log   │
│  _templates  │     │              │     │              │
└──────────────┘     └──────────────┘     └──────────────┘

┌──────────────┐
│  _migrations │  (internal: tracks schema version)
└──────────────┘
```

### 2.2 Table: `ai_providers`

Stores LLM provider configurations (without API keys).

```sql
CREATE TABLE IF NOT EXISTS ai_providers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,                  -- Display name: "OpenAI", "Anthropic", custom name
    api_base    TEXT NOT NULL,                  -- Base URL: "https://api.openai.com", "https://api.anthropic.com"
    api_format  TEXT NOT NULL DEFAULT 'openai_completions',
                                               -- API format: 'anthropic', 'openai_responses', 'openai_completions'
    enabled     INTEGER NOT NULL DEFAULT 1,    -- 0 = disabled, 1 = enabled
    notes       TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    CHECK (api_format IN ('anthropic', 'openai_responses', 'openai_completions'))
);
```

**`api_format` field values:**

| Value | Description | Reasoning Effort |
|-------|-------------|------------------|
| `anthropic` | Anthropic Messages API format | Not supported |
| `openai_responses` | OpenAI Responses API format | Supported |
| `openai_completions` | OpenAI Chat Completions format | Not supported |

**Note:** The `provider_type`, `default_model`, and `available_models` columns from v1.0.0 have been removed. Provider-specific model lists are now managed in the `ai_models` table. The `api_format` column replaces `provider_type` as the discriminator for request format.

### 2.2b Table: `ai_models`

Stores models discovered from each provider via the `/v1/models` endpoint. Users can enable/disable individual models.

```sql
CREATE TABLE IF NOT EXISTS ai_models (
    id              TEXT NOT NULL,              -- Model ID from API (e.g., "gpt-4o", "claude-3-5-sonnet")
    provider_id     TEXT NOT NULL REFERENCES ai_providers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL DEFAULT '',   -- Display name (defaults to id if not provided)
    is_enabled      INTEGER NOT NULL DEFAULT 0, -- 0 = disabled, 1 = enabled (user toggle)
    supports_reasoning INTEGER NOT NULL DEFAULT 0,
                                               -- 1 = model supports reasoning effort (OpenAI Responses API)
    fetched_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (id, provider_id)
);

CREATE INDEX IF NOT EXISTS idx_ai_models_provider
    ON ai_models(provider_id);

CREATE INDEX IF NOT EXISTS idx_ai_models_enabled
    ON ai_models(provider_id, is_enabled);
```

**Usage flow:**
1. User adds a provider with `name`, `api_base`, `api_key`, `api_format`
2. Frontend calls `llm_models_fetch` → backend GETs `{api_base}/v1/models`
3. Discovered models are inserted into `ai_models` with `is_enabled = 0`
4. User toggles specific models via `llm_models_toggle`
5. Only enabled models (`is_enabled = 1`) appear in the chat model selector
```

### 2.3 Table: `api_keys`

Stores encrypted API keys. The plaintext key is never stored.

```sql
CREATE TABLE IF NOT EXISTS api_keys (
    provider_id   TEXT PRIMARY KEY REFERENCES ai_providers(id) ON DELETE CASCADE,
    encrypted_key BLOB NOT NULL,       -- AES-256-GCM encrypted API key
    nonce         BLOB NOT NULL,       -- 12-byte nonce for AES-GCM
    key_hint      TEXT NOT NULL DEFAULT '', -- Last 4 characters for display: "...sk-1234"
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
```

### 2.4 Table: `conversations`

Chat conversation metadata.

```sql
CREATE TABLE IF NOT EXISTS conversations (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL DEFAULT 'New Conversation',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    archived    INTEGER NOT NULL DEFAULT 0,   -- 0 = active, 1 = archived
    metadata    TEXT NOT NULL DEFAULT '{}'    -- JSON blob for extensible metadata
);

CREATE INDEX IF NOT EXISTS idx_conversations_updated
    ON conversations(updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversations_archived
    ON conversations(archived, updated_at DESC);
```

### 2.5 Table: `messages`

Individual messages within conversations.

```sql
CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,          -- "system", "user", "assistant", "tool"
    content         TEXT NOT NULL DEFAULT '',
    tool_calls      TEXT,                   -- JSON array of tool call objects (nullable)
    tool_call_id    TEXT,                   -- For tool response messages (nullable)
    model           TEXT,                   -- LLM model used for assistant messages
    provider_id     TEXT,                   -- Provider used for this message
    usage           TEXT,                   -- JSON: {"prompt_tokens":..., "completion_tokens":...}
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    sort_order      INTEGER NOT NULL DEFAULT 0  -- For ordering within conversation
);

CREATE INDEX IF NOT EXISTS idx_messages_conversation
    ON messages(conversation_id, sort_order);
```

**Tool Calls JSON Format:**

```json
[
    {
        "id": "call_abc123",
        "type": "function",
        "function": {
            "name": "container_create",
            "arguments": "{\"name\":\"my-app\",\"image\":\"node:20\"}"
        }
    }
]
```

### 2.6 Table: `container_templates`

Predefined container configurations (sandbox templates).

```sql
CREATE TABLE IF NOT EXISTS container_templates (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,              -- "Node.js Development"
    description TEXT NOT NULL DEFAULT '',
    image       TEXT NOT NULL,              -- "node:20-alpine"
    command     TEXT,                        -- Override CMD
    env         TEXT NOT NULL DEFAULT '[]', -- JSON array: ["NODE_ENV=development"]
    ports       TEXT NOT NULL DEFAULT '[]', -- JSON array of port mappings
    volumes     TEXT NOT NULL DEFAULT '[]', -- JSON array of volume mounts
    cpu_cores   INTEGER NOT NULL DEFAULT 2,
    memory_mb   INTEGER NOT NULL DEFAULT 1024,
    working_dir TEXT,
    labels      TEXT NOT NULL DEFAULT '{}', -- JSON object of labels
    enabled     INTEGER NOT NULL DEFAULT 1,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Default templates
INSERT OR IGNORE INTO container_templates (id, name, description, image, cpu_cores, memory_mb, env)
VALUES
    ('node-dev', 'Node.js Development', 'Node.js 20 with common dev tools',
     'node:20-alpine', 2, 1024, '["NODE_ENV=development"]'),
    ('python-dev', 'Python Development', 'Python 3.12 with pip and common tools',
     'python:3.12-slim', 2, 1024, '[]'),
    ('rust-dev', 'Rust Development', 'Rust stable with cargo',
     'rust:1-slim', 2, 2048, '[]'),
    ('ubuntu', 'Ubuntu Shell', 'Ubuntu 24.04 general-purpose shell',
     'ubuntu:24.04', 1, 512, '[]');
```

### 2.7 Table: `mcp_servers`

MCP server configurations.

```sql
CREATE TABLE IF NOT EXISTS mcp_servers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    command     TEXT NOT NULL,              -- Executable path
    args        TEXT NOT NULL DEFAULT '[]', -- JSON array of arguments
    env         TEXT NOT NULL DEFAULT '[]', -- JSON array: ["KEY=VALUE"]
    working_dir TEXT NOT NULL DEFAULT '',
    enabled     INTEGER NOT NULL DEFAULT 1,
    notes       TEXT NOT NULL DEFAULT '',
    auto_start  INTEGER NOT NULL DEFAULT 0, -- Auto-start on app launch
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Default MCP servers (from .mcp.json)
INSERT OR IGNORE INTO mcp_servers (id, name, command, args, notes)
VALUES
    ('shadcn', 'shadcn/ui', 'npx', '["shadcn@latest","mcp"]',
     'Browse and install shadcn/ui components');
```

### 2.8 Table: `audit_log`

Operation audit trail.

```sql
CREATE TABLE IF NOT EXISTS audit_log (
    id          TEXT PRIMARY KEY,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    action      TEXT NOT NULL,             -- "container.create", "api_key.save", etc.
    target      TEXT NOT NULL DEFAULT '',  -- Resource identifier (container ID, provider ID)
    details     TEXT,                       -- JSON or text details
    user        TEXT NOT NULL DEFAULT 'user',
    ip_address  TEXT,                       -- Reserved for future remote access
    session_id  TEXT                        -- Reserved for future session tracking
);

CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp
    ON audit_log(timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_log_action
    ON audit_log(action, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_log_target
    ON audit_log(target, timestamp DESC);
```

**Audit Action Names:**

| Action | Description |
|--------|-------------|
| `container.create` | Container created |
| `container.start` | Container started |
| `container.stop` | Container stopped |
| `container.delete` | Container deleted |
| `container.exec` | Command executed in container |
| `api_key.save` | API key stored (encrypted) |
| `api_key.delete` | API key removed |
| `provider.create` | LLM provider created |
| `provider.update` | LLM provider updated |
| `provider.delete` | LLM provider removed |
| `model.toggle` | Model enabled/disabled |
| `mcp_server.start` | MCP server started |
| `mcp_server.stop` | MCP server stopped |
| `settings.update` | Application setting changed |
| `conversation.create` | New conversation started |
| `conversation.delete` | Conversation deleted |

### 2.9 Table: `settings`

Key-value application settings.

```sql
CREATE TABLE IF NOT EXISTS settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Default settings
INSERT OR IGNORE INTO settings (key, value)
VALUES
    ('theme', 'system'),
    ('language', 'en'),
    ('runtime.auto_start', 'true'),
    ('runtime.cpu_cores', '2'),
    ('runtime.memory_mb', '2048');
```

### 2.10 Table: `_migrations`

Internal migration tracking (not exposed via API).

```sql
CREATE TABLE IF NOT EXISTS _migrations (
    version     INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    applied_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
```

---

## 3. Migration Strategy

### 3.1 Version-Based SQL Migrations

Each migration is a numbered SQL script applied in sequence:

```rust
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: include_str!("../migrations/001_initial_schema.sql"),
    },
    Migration {
        version: 2,
        name: "add_audit_log",
        sql: include_str!("../migrations/002_add_audit_log.sql"),
    },
    // Future migrations added here
];
```

### 3.2 Migration Runner

```rust
pub fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    // Ensure migrations table exists
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );
    ")?;

    let current_version: u32 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM _migrations", [], |row| row.get(0))
        .unwrap_or(0);

    for migration in MIGRATIONS {
        if migration.version > current_version {
            tracing::info!(
                "Applying migration v{}: {}",
                migration.version,
                migration.name
            );

            // Run migration in a transaction
            conn.execute_batch("BEGIN;")?;
            match conn.execute_batch(migration.sql) {
                Ok(_) => {
                    conn.execute(
                        "INSERT INTO _migrations (version, name) VALUES (?1, ?2)",
                        params![migration.version, migration.name],
                    )?;
                    conn.execute_batch("COMMIT;")?;
                    tracing::info!("Migration v{} applied successfully", migration.version);
                }
                Err(e) => {
                    conn.execute_batch("ROLLBACK;")?;
                    return Err(AppError::Database(e));
                }
            }
        }
    }

    Ok(())
}
```

### 3.3 Migration File: `001_initial_schema.sql`

```sql
-- Migration 001: Initial schema
-- Creates all core tables for CrateBay

-- LLM Providers
CREATE TABLE IF NOT EXISTS ai_providers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    api_base    TEXT NOT NULL,
    api_format  TEXT NOT NULL DEFAULT 'openai_completions',
    enabled     INTEGER NOT NULL DEFAULT 1,
    notes       TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    CHECK (api_format IN ('anthropic', 'openai_responses', 'openai_completions'))
);

-- AI Models (per-provider model list)
CREATE TABLE IF NOT EXISTS ai_models (
    id              TEXT NOT NULL,
    provider_id     TEXT NOT NULL REFERENCES ai_providers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL DEFAULT '',
    is_enabled      INTEGER NOT NULL DEFAULT 0,
    supports_reasoning INTEGER NOT NULL DEFAULT 0,
    fetched_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (id, provider_id)
);

CREATE INDEX IF NOT EXISTS idx_ai_models_provider
    ON ai_models(provider_id);

-- Encrypted API Keys
CREATE TABLE IF NOT EXISTS api_keys (
    provider_id   TEXT PRIMARY KEY REFERENCES ai_providers(id) ON DELETE CASCADE,
    encrypted_key BLOB NOT NULL,
    nonce         BLOB NOT NULL,
    key_hint      TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Conversations
CREATE TABLE IF NOT EXISTS conversations (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL DEFAULT 'New Conversation',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    archived    INTEGER NOT NULL DEFAULT 0,
    metadata    TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_conversations_updated
    ON conversations(updated_at DESC);

-- Messages
CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,
    content         TEXT NOT NULL DEFAULT '',
    tool_calls      TEXT,
    tool_call_id    TEXT,
    model           TEXT,
    provider_id     TEXT,
    usage           TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    sort_order      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_messages_conversation
    ON messages(conversation_id, sort_order);

-- Container Templates
CREATE TABLE IF NOT EXISTS container_templates (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    image       TEXT NOT NULL,
    command     TEXT,
    env         TEXT NOT NULL DEFAULT '[]',
    ports       TEXT NOT NULL DEFAULT '[]',
    volumes     TEXT NOT NULL DEFAULT '[]',
    cpu_cores   INTEGER NOT NULL DEFAULT 2,
    memory_mb   INTEGER NOT NULL DEFAULT 1024,
    working_dir TEXT,
    labels      TEXT NOT NULL DEFAULT '{}',
    enabled     INTEGER NOT NULL DEFAULT 1,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- MCP Servers
CREATE TABLE IF NOT EXISTS mcp_servers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    command     TEXT NOT NULL,
    args        TEXT NOT NULL DEFAULT '[]',
    env         TEXT NOT NULL DEFAULT '[]',
    working_dir TEXT NOT NULL DEFAULT '',
    enabled     INTEGER NOT NULL DEFAULT 1,
    notes       TEXT NOT NULL DEFAULT '',
    auto_start  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Settings
CREATE TABLE IF NOT EXISTS settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Audit Log
CREATE TABLE IF NOT EXISTS audit_log (
    id          TEXT PRIMARY KEY,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    action      TEXT NOT NULL,
    target      TEXT NOT NULL DEFAULT '',
    details     TEXT,
    user        TEXT NOT NULL DEFAULT 'user',
    ip_address  TEXT,
    session_id  TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp
    ON audit_log(timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_log_action
    ON audit_log(action, timestamp DESC);

-- Seed default data
-- Note: No default providers are pre-seeded in v1.1.0.
-- Users configure their own providers via Settings > LLM Providers.

INSERT OR IGNORE INTO container_templates (id, name, description, image, cpu_cores, memory_mb, env)
VALUES
    ('node-dev', 'Node.js Development', 'Node.js 20 with common dev tools',
     'node:20-alpine', 2, 1024, '["NODE_ENV=development"]'),
    ('python-dev', 'Python Development', 'Python 3.12 with pip',
     'python:3.12-slim', 2, 1024, '[]'),
    ('rust-dev', 'Rust Development', 'Rust stable with cargo',
     'rust:1-slim', 2, 2048, '[]'),
    ('ubuntu', 'Ubuntu Shell', 'Ubuntu 24.04 general-purpose',
     'ubuntu:24.04', 1, 512, '[]');

INSERT OR IGNORE INTO mcp_servers (id, name, command, args, notes)
VALUES
    ('shadcn', 'shadcn/ui', 'npx', '["shadcn@latest","mcp"]',
     'Browse and install shadcn/ui components');

INSERT OR IGNORE INTO settings (key, value)
VALUES
    ('theme', 'system'),
    ('language', 'en'),
    ('runtime.auto_start', 'true'),
    ('runtime.cpu_cores', '2'),
    ('runtime.memory_mb', '2048');
```

### 3.4 Migration Guidelines

| Rule | Description |
|------|-------------|
| **Append-only** | Never modify existing migrations. Always create a new one. |
| **Backward compatible** | New columns must have defaults. Never drop columns in minor versions. |
| **Transactional** | Each migration runs in a transaction. Failure rolls back that migration. |
| **Idempotent** | Use `IF NOT EXISTS` and `INSERT OR IGNORE` for safety. |
| **Versioned** | Each migration has a unique, monotonically increasing version number. |

---

## 4. Encryption

### 4.1 API Key Encryption Scheme

```
┌───────────┐     ┌──────────────┐     ┌──────────────┐
│ Plaintext │     │ System       │     │ SQLite       │
│ API Key   │     │ Keyring      │     │ api_keys     │
│ (input)   │     │              │     │ table        │
└─────┬─────┘     └──────┬───────┘     └──────┬───────┘
      │                  │                     │
      │   1. Get or create           3. Store  │
      │      master key              encrypted │
      │                  │            key+nonce │
      │           ┌──────▼───────┐             │
      └──────────→│ AES-256-GCM │─────────────┘
                  │ Encrypt      │
                  └──────────────┘
```

### 4.2 Key Derivation

The encryption key is derived from the system keyring:

```rust
use keyring::Entry;

const KEYRING_SERVICE: &str = "com.cratebay.app";
const KEYRING_KEY: &str = "master-encryption-key";

/// Get or create the master encryption key from the system keyring
fn get_master_key() -> Result<[u8; 32], AppError> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_KEY)
        .map_err(|e| AppError::Runtime(format!("Keyring access failed: {}", e)))?;

    match entry.get_password() {
        Ok(key_hex) => {
            // Key exists, decode it
            let key = hex::decode(&key_hex)
                .map_err(|_| AppError::Runtime("Invalid master key in keyring".into()))?;
            let mut result = [0u8; 32];
            result.copy_from_slice(&key);
            Ok(result)
        }
        Err(_) => {
            // First time: generate a new key
            let mut key = [0u8; 32];
            getrandom::getrandom(&mut key)
                .map_err(|e| AppError::Runtime(format!("Random generation failed: {}", e)))?;
            entry
                .set_password(&hex::encode(key))
                .map_err(|e| AppError::Runtime(format!("Keyring store failed: {}", e)))?;
            Ok(key)
        }
    }
}
```

### 4.3 Encrypt / Decrypt Functions

```rust
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::Aead;

/// Encrypt an API key
pub fn encrypt_api_key(plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), AppError> {
    let master_key = get_master_key()?;
    let cipher = Aes256Gcm::new_from_slice(&master_key)
        .map_err(|e| AppError::Runtime(format!("Cipher init failed: {}", e)))?;

    // Generate random 12-byte nonce
    let mut nonce_bytes = [0u8; 12];
    getrandom::getrandom(&mut nonce_bytes)
        .map_err(|e| AppError::Runtime(format!("Nonce generation failed: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| AppError::Runtime(format!("Encryption failed: {}", e)))?;

    Ok((ciphertext, nonce_bytes.to_vec()))
}

/// Decrypt an API key
pub fn decrypt_api_key(encrypted: &[u8], nonce: &[u8]) -> Result<String, AppError> {
    let master_key = get_master_key()?;
    let cipher = Aes256Gcm::new_from_slice(&master_key)
        .map_err(|e| AppError::Runtime(format!("Cipher init failed: {}", e)))?;

    let nonce = Nonce::from_slice(nonce);
    let plaintext = cipher
        .decrypt(nonce, encrypted)
        .map_err(|e| AppError::Runtime(format!("Decryption failed: {}", e)))?;

    String::from_utf8(plaintext)
        .map_err(|e| AppError::Runtime(format!("Invalid UTF-8 after decryption: {}", e)))
}
```

### 4.4 System Keyring Integration

| Platform | Keyring Backend |
|----------|----------------|
| macOS | macOS Keychain (`security` framework) |
| Linux | Secret Service API (GNOME Keyring / KDE Wallet) |
| Windows | Windows Credential Manager |

**Fallback**: If no system keyring is available (e.g., headless Linux), fall back to a file-based key stored at `~/.cratebay/.master_key` with `600` permissions. This is less secure but ensures the application works in all environments.

### 4.5 Key Hint Display

For UI display, the `key_hint` field stores the last 4 characters of the API key:

```rust
fn compute_key_hint(api_key: &str) -> String {
    if api_key.len() >= 4 {
        format!("...{}", &api_key[api_key.len() - 4..])
    } else {
        "****".to_string()
    }
}
```

The frontend shows: `API Key: ...sk-1234` — enough for the user to identify the key without exposing it.

---

## 5. Data Lifecycle

### 5.1 Conversation Cleanup

Old conversations can be cleaned up based on user preference:

```rust
/// Delete conversations older than the specified number of days
pub fn cleanup_old_conversations(
    conn: &Connection,
    older_than_days: u32,
) -> Result<u32, AppError> {
    let cutoff = Utc::now() - chrono::Duration::days(older_than_days as i64);
    let deleted = conn.execute(
        "DELETE FROM conversations
         WHERE updated_at < ?1
         AND archived = 0",
        params![cutoff.to_rfc3339()],
    )?;
    // Messages are cascade-deleted via FK
    Ok(deleted as u32)
}
```

**Cleanup policies** (configurable via settings):

| Setting | Default | Description |
|---------|---------|-------------|
| `conversation.auto_cleanup` | `false` | Enable automatic cleanup |
| `conversation.retention_days` | `90` | Days to keep conversations |
| `conversation.max_count` | `1000` | Maximum number of conversations |

### 5.2 Audit Log Rotation

```rust
/// Rotate audit logs older than the specified number of days
pub fn rotate_audit_log(
    conn: &Connection,
    older_than_days: u32,
) -> Result<u32, AppError> {
    let cutoff = Utc::now() - chrono::Duration::days(older_than_days as i64);
    let deleted = conn.execute(
        "DELETE FROM audit_log WHERE timestamp < ?1",
        params![cutoff.to_rfc3339()],
    )?;
    Ok(deleted as u32)
}
```

**Rotation policy:**

| Setting | Default | Description |
|---------|---------|-------------|
| `audit.retention_days` | `30` | Days to keep audit logs |
| `audit.max_entries` | `10000` | Maximum audit log entries |

### 5.3 Database Size Management

```rust
/// Get database size and statistics
pub fn db_stats(conn: &Connection) -> Result<DbStats, AppError> {
    let page_count: u64 = conn.query_row("PRAGMA page_count;", [], |r| r.get(0))?;
    let page_size: u64 = conn.query_row("PRAGMA page_size;", [], |r| r.get(0))?;
    let freelist_count: u64 = conn.query_row("PRAGMA freelist_count;", [], |r| r.get(0))?;

    Ok(DbStats {
        total_size_bytes: page_count * page_size,
        free_space_bytes: freelist_count * page_size,
        conversation_count: count_table(conn, "conversations")?,
        message_count: count_table(conn, "messages")?,
        audit_log_count: count_table(conn, "audit_log")?,
    })
}

/// Reclaim unused space
pub fn vacuum(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("VACUUM;")?;
    Ok(())
}
```

---

## 6. Backup & Export

### 6.1 Database Backup

```rust
/// Create a backup of the database
pub fn backup(conn: &Connection, backup_dir: &Path) -> Result<PathBuf, AppError> {
    std::fs::create_dir_all(backup_dir)?;

    let timestamp = Utc::now().format("%Y-%m-%d_%H%M%S");
    let backup_path = backup_dir.join(format!("cratebay-{}.db", timestamp));

    // Use SQLite Online Backup API
    let mut dst = Connection::open(&backup_path)?;
    let backup = rusqlite::backup::Backup::new(conn, &mut dst)?;
    backup.run_to_completion(100, std::time::Duration::from_millis(250), None)?;

    tracing::info!("Database backed up to {}", backup_path.display());
    Ok(backup_path)
}
```

### 6.2 Conversation Export

```rust
/// Export a conversation to JSON
pub fn export_conversation(
    conn: &Connection,
    conversation_id: &str,
) -> Result<serde_json::Value, AppError> {
    let conversation = get_conversation(conn, conversation_id)?;

    Ok(serde_json::json!({
        "version": "1.0",
        "exported_at": Utc::now().to_rfc3339(),
        "conversation": {
            "id": conversation.id,
            "title": conversation.title,
            "created_at": conversation.created_at,
            "messages": conversation.messages.iter().map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                    "tool_calls": m.tool_calls,
                    "created_at": m.created_at,
                })
            }).collect::<Vec<_>>(),
        }
    }))
}
```

### 6.3 Settings Export/Import

```rust
/// Export all settings to JSON
pub fn export_settings(conn: &Connection) -> Result<serde_json::Value, AppError> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let settings: HashMap<String, String> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({
        "version": "1.0",
        "exported_at": Utc::now().to_rfc3339(),
        "settings": settings,
    }))
}

/// Import settings from JSON (merge, don't replace)
pub fn import_settings(
    conn: &Connection,
    data: serde_json::Value,
) -> Result<u32, AppError> {
    let settings = data["settings"]
        .as_object()
        .ok_or_else(|| AppError::Validation("Invalid settings format".into()))?;

    let mut count = 0u32;
    for (key, value) in settings {
        if let Some(v) = value.as_str() {
            set_setting(conn, key, v)?;
            count += 1;
        }
    }
    Ok(count)
}
```

### 6.4 Backup Schedule

| Trigger | Action |
|---------|--------|
| App startup | Check if last backup > 7 days, prompt user |
| Before migration | Automatic backup before applying migrations |
| Manual | User-triggered via Settings > Backup |
| Export | User can export specific conversations or all data |

### 6.5 Backup Retention

```rust
/// Clean up old backups, keeping only the most recent N
pub fn cleanup_backups(backup_dir: &Path, keep_count: usize) -> Result<(), AppError> {
    let mut backups: Vec<_> = std::fs::read_dir(backup_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "db"))
        .collect();

    backups.sort_by_key(|e| std::cmp::Reverse(e.metadata().ok()?.modified().ok()?));

    for backup in backups.into_iter().skip(keep_count) {
        std::fs::remove_file(backup.path())?;
    }

    Ok(())
}
```

Default: keep the 5 most recent backups.

---

## Appendix A: SQLite Performance Notes

| Aspect | Configuration | Rationale |
|--------|--------------|-----------|
| Journal mode | WAL | Allows concurrent reads during writes |
| Synchronous | NORMAL | Good balance of durability and speed |
| Foreign keys | ON | Data integrity enforcement |
| Busy timeout | 5000 ms | Graceful handling of lock contention |
| Cache size | 2 MB | Reasonable memory usage for desktop app |
| Connection pool | Single connection | SQLite WAL mode handles concurrency well with one writer |

## Appendix B: Data Types Convention

| Concept | SQLite Type | Format |
|---------|-------------|--------|
| Primary key | TEXT | UUID v4 string |
| Timestamp | TEXT | RFC3339 (`2026-03-20T12:00:00Z`) |
| Boolean | INTEGER | 0 = false, 1 = true |
| JSON data | TEXT | Valid JSON string |
| Encrypted data | BLOB | Raw bytes |
| Enum | TEXT | Lowercase string value |
