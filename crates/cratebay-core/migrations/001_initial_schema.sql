-- Migration 001: Initial schema
-- Creates all core tables for CrateBay v2

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

CREATE INDEX IF NOT EXISTS idx_ai_models_enabled
    ON ai_models(provider_id, is_enabled);

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

CREATE INDEX IF NOT EXISTS idx_conversations_archived
    ON conversations(archived, updated_at DESC);

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

-- Settings (key-value)
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

CREATE INDEX IF NOT EXISTS idx_audit_log_target
    ON audit_log(target, timestamp DESC);

-- Seed default data

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
