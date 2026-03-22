//! SQLite storage layer.
//!
//! Database stored at `~/.cratebay/cratebay.db` with WAL mode.
//! Includes migration system, settings CRUD, conversations/messages CRUD,
//! and provider/model management.

use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::models::{
    ApiFormat, ChatMessage, ConversationDetail, ConversationSummary, LlmModelInfo, LlmProvider,
    McpServerConfig, McpServerStatus,
};

// ─── Migration System ───────────────────────────────────────────────

/// A numbered database migration.
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// All migrations, applied in order.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_schema",
    sql: include_str!("../migrations/001_initial_schema.sql"),
}];

/// Get the default database path: `~/.cratebay/cratebay.db`
pub fn default_db_path() -> Result<PathBuf, AppError> {
    let home = home_dir()?;
    let db_dir = home.join(".cratebay");
    std::fs::create_dir_all(&db_dir)?;
    Ok(db_dir.join("cratebay.db"))
}

/// Open a database connection with recommended PRAGMA settings.
pub fn open(path: &Path) -> Result<Connection, AppError> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;

    // Apply performance and safety PRAGMAs
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA cache_size = -2000;
         PRAGMA temp_store = MEMORY;",
    )?;

    Ok(conn)
}

/// Run all pending database migrations.
pub fn migrate(conn: &Connection) -> Result<(), AppError> {
    // Create migrations tracking table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version     INTEGER PRIMARY KEY,
            name        TEXT NOT NULL,
            applied_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );",
    )?;

    let current_version: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |row| row.get(0),
        )
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
                Ok(()) => {
                    conn.execute(
                        "INSERT INTO _migrations (version, name) VALUES (?1, ?2)",
                        params![migration.version, migration.name],
                    )?;
                    conn.execute_batch("COMMIT;")?;
                    tracing::info!("Migration v{} applied successfully", migration.version);
                }
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK;");
                    return Err(AppError::Database(e));
                }
            }
        }
    }

    Ok(())
}

/// Open and migrate database in one step.
pub fn init(path: &Path) -> Result<Connection, AppError> {
    let conn = open(path)?;
    migrate(&conn)?;
    Ok(conn)
}

// ─── Settings CRUD ──────────────────────────────────────────────────

/// Get a setting value by key.
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
    let result = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(result)
}

/// Set a setting value (insert or update).
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO settings (key, value, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
         ON CONFLICT(key) DO UPDATE SET
             value = excluded.value,
             updated_at = excluded.updated_at",
        params![key, value],
    )?;
    Ok(())
}

/// Get all settings as key-value pairs.
pub fn get_all_settings(conn: &Connection) -> Result<Vec<(String, String)>, AppError> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ─── Provider CRUD ──────────────────────────────────────────────────

/// Create a new LLM provider.
pub fn create_provider(
    conn: &Connection,
    id: &str,
    name: &str,
    api_base: &str,
    api_format: &ApiFormat,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO ai_providers (id, name, api_base, api_format)
         VALUES (?1, ?2, ?3, ?4)",
        params![id, name, api_base, api_format.as_str()],
    )?;
    Ok(())
}

/// Update an existing LLM provider.
pub fn update_provider(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    api_base: Option<&str>,
    api_format: Option<&ApiFormat>,
    enabled: Option<bool>,
) -> Result<(), AppError> {
    // Build dynamic UPDATE
    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(n) = name {
        sets.push("name = ?");
        values.push(Box::new(n.to_string()));
    }
    if let Some(b) = api_base {
        sets.push("api_base = ?");
        values.push(Box::new(b.to_string()));
    }
    if let Some(f) = api_format {
        sets.push("api_format = ?");
        values.push(Box::new(f.as_str().to_string()));
    }
    if let Some(e) = enabled {
        sets.push("enabled = ?");
        values.push(Box::new(e as i32));
    }

    if sets.is_empty() {
        return Ok(());
    }

    sets.push("updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')");

    let sql = format!("UPDATE ai_providers SET {} WHERE id = ?", sets.join(", "));
    values.push(Box::new(id.to_string()));

    let params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let updated = conn.execute(&sql, params.as_slice())?;

    if updated == 0 {
        return Err(AppError::NotFound {
            entity: "provider".to_string(),
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Delete a provider (cascade deletes api_keys and ai_models via FK).
pub fn delete_provider(conn: &Connection, id: &str) -> Result<(), AppError> {
    let deleted = conn.execute("DELETE FROM ai_providers WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(AppError::NotFound {
            entity: "provider".to_string(),
            id: id.to_string(),
        });
    }
    Ok(())
}

/// List all LLM providers with API key presence info.
pub fn list_providers(conn: &Connection) -> Result<Vec<LlmProvider>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.api_base, p.api_format, p.enabled, p.notes,
                p.created_at, p.updated_at,
                CASE WHEN k.provider_id IS NOT NULL THEN 1 ELSE 0 END as has_key
         FROM ai_providers p
         LEFT JOIN api_keys k ON p.id = k.provider_id
         ORDER BY p.created_at",
    )?;

    let rows = stmt.query_map([], |row| {
        let format_str: String = row.get(3)?;
        Ok(LlmProvider {
            id: row.get(0)?,
            name: row.get(1)?,
            api_base: row.get(2)?,
            api_format: ApiFormat::from_str(&format_str).unwrap_or(ApiFormat::OpenAiCompletions),
            enabled: row.get::<_, i32>(4)? != 0,
            notes: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            has_api_key: row.get::<_, i32>(8)? != 0,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Get a single provider by ID.
pub fn get_provider(conn: &Connection, id: &str) -> Result<LlmProvider, AppError> {
    let result = conn
        .query_row(
            "SELECT p.id, p.name, p.api_base, p.api_format, p.enabled, p.notes,
                    p.created_at, p.updated_at,
                    CASE WHEN k.provider_id IS NOT NULL THEN 1 ELSE 0 END as has_key
             FROM ai_providers p
             LEFT JOIN api_keys k ON p.id = k.provider_id
             WHERE p.id = ?1",
            params![id],
            |row| {
                let format_str: String = row.get(3)?;
                Ok(LlmProvider {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    api_base: row.get(2)?,
                    api_format: ApiFormat::from_str(&format_str)
                        .unwrap_or(ApiFormat::OpenAiCompletions),
                    enabled: row.get::<_, i32>(4)? != 0,
                    notes: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    has_api_key: row.get::<_, i32>(8)? != 0,
                })
            },
        )
        .optional()?;

    result.ok_or_else(|| AppError::NotFound {
        entity: "provider".to_string(),
        id: id.to_string(),
    })
}

// ─── API Key Operations (encrypted storage) ─────────────────────────

/// Save an encrypted API key for a provider.
/// The caller is responsible for encryption — this stores raw bytes.
pub fn save_api_key(
    conn: &Connection,
    provider_id: &str,
    encrypted_key: &[u8],
    nonce: &[u8],
    key_hint: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO api_keys (provider_id, encrypted_key, nonce, key_hint)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(provider_id) DO UPDATE SET
             encrypted_key = excluded.encrypted_key,
             nonce = excluded.nonce,
             key_hint = excluded.key_hint,
             updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        params![provider_id, encrypted_key, nonce, key_hint],
    )?;
    Ok(())
}

/// Load the encrypted API key and nonce for a provider.
pub fn load_api_key(conn: &Connection, provider_id: &str) -> Result<(Vec<u8>, Vec<u8>), AppError> {
    let result = conn
        .query_row(
            "SELECT encrypted_key, nonce FROM api_keys WHERE provider_id = ?1",
            params![provider_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    result.ok_or_else(|| AppError::NotFound {
        entity: "api_key".to_string(),
        id: provider_id.to_string(),
    })
}

/// Delete an API key for a provider.
pub fn delete_api_key(conn: &Connection, provider_id: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM api_keys WHERE provider_id = ?1",
        params![provider_id],
    )?;
    Ok(())
}

/// Get the key hint for display (e.g., "...sk-1234").
pub fn get_api_key_hint(conn: &Connection, provider_id: &str) -> Result<Option<String>, AppError> {
    let result = conn
        .query_row(
            "SELECT key_hint FROM api_keys WHERE provider_id = ?1",
            params![provider_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(result)
}

// ─── AI Model Operations ────────────────────────────────────────────

/// Save models for a provider (replace all existing models for that provider).
pub fn save_models(
    conn: &Connection,
    provider_id: &str,
    models: &[(String, String, bool)], // (id, name, supports_reasoning)
) -> Result<(), AppError> {
    // Delete existing models for this provider that are not in the new list
    let model_ids: Vec<&str> = models.iter().map(|(id, _, _)| id.as_str()).collect();

    if model_ids.is_empty() {
        conn.execute(
            "DELETE FROM ai_models WHERE provider_id = ?1",
            params![provider_id],
        )?;
        return Ok(());
    }

    // Upsert each model, preserving is_enabled for existing ones
    for (id, name, supports_reasoning) in models {
        conn.execute(
            "INSERT INTO ai_models (id, provider_id, name, supports_reasoning, fetched_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
             ON CONFLICT(id, provider_id) DO UPDATE SET
                 name = excluded.name,
                 supports_reasoning = excluded.supports_reasoning,
                 fetched_at = excluded.fetched_at",
            params![id, provider_id, name, *supports_reasoning as i32],
        )?;
    }

    // Remove models that are no longer in the list
    let placeholders: Vec<String> = (0..model_ids.len())
        .map(|i| format!("?{}", i + 2))
        .collect();
    let sql = format!(
        "DELETE FROM ai_models WHERE provider_id = ?1 AND id NOT IN ({})",
        placeholders.join(", ")
    );

    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(provider_id.to_string())];
    for id in &model_ids {
        params_vec.push(Box::new(id.to_string()));
    }
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|v| v.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())?;

    Ok(())
}

/// List models for a specific provider.
pub fn list_models(conn: &Connection, provider_id: &str) -> Result<Vec<LlmModelInfo>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, provider_id, name, is_enabled, supports_reasoning
         FROM ai_models
         WHERE provider_id = ?1
         ORDER BY name",
    )?;

    let rows = stmt.query_map(params![provider_id], |row| {
        Ok(LlmModelInfo {
            id: row.get(0)?,
            provider_id: row.get(1)?,
            name: row.get(2)?,
            is_enabled: row.get::<_, i32>(3)? != 0,
            supports_reasoning: row.get::<_, i32>(4)? != 0,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Toggle a model's enabled state.
pub fn toggle_model(
    conn: &Connection,
    provider_id: &str,
    model_id: &str,
    enabled: bool,
) -> Result<(), AppError> {
    let updated = conn.execute(
        "UPDATE ai_models SET is_enabled = ?1 WHERE id = ?2 AND provider_id = ?3",
        params![enabled as i32, model_id, provider_id],
    )?;

    if updated == 0 {
        return Err(AppError::NotFound {
            entity: "model".to_string(),
            id: format!("{}:{}", provider_id, model_id),
        });
    }
    Ok(())
}

// ─── Conversation CRUD ──────────────────────────────────────────────

/// Create a new conversation.
pub fn create_conversation(conn: &Connection, id: &str, title: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO conversations (id, title) VALUES (?1, ?2)",
        params![id, title],
    )?;
    Ok(())
}

/// List conversations with summary info.
pub fn list_conversations(
    conn: &Connection,
    limit: u32,
    offset: u32,
) -> Result<Vec<ConversationSummary>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.title, c.created_at, c.updated_at,
                COUNT(m.id) as message_count,
                (SELECT content FROM messages
                 WHERE conversation_id = c.id
                 ORDER BY sort_order DESC LIMIT 1) as last_preview
         FROM conversations c
         LEFT JOIN messages m ON m.conversation_id = c.id
         WHERE c.archived = 0
         GROUP BY c.id
         ORDER BY c.updated_at DESC
         LIMIT ?1 OFFSET ?2",
    )?;

    let rows = stmt.query_map(params![limit, offset], |row| {
        let preview: Option<String> = row.get(5)?;
        Ok(ConversationSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
            message_count: row.get(4)?,
            last_message_preview: preview.map(|p| {
                if p.len() > 100 {
                    format!("{}...", &p[..100])
                } else {
                    p
                }
            }),
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Get a conversation with all its messages.
pub fn get_conversation(conn: &Connection, id: &str) -> Result<ConversationDetail, AppError> {
    let (title, created_at, updated_at) = conn
        .query_row(
            "SELECT title, created_at, updated_at FROM conversations WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound {
            entity: "conversation".to_string(),
            id: id.to_string(),
        })?;

    let mut stmt = conn.prepare(
        "SELECT id, role, content, tool_calls, tool_call_id
         FROM messages
         WHERE conversation_id = ?1
         ORDER BY sort_order, created_at",
    )?;

    let rows = stmt.query_map(params![id], |row| {
        let tool_calls_json: Option<String> = row.get(3)?;
        let tool_calls = tool_calls_json.and_then(|j| serde_json::from_str(&j).ok());

        Ok(ChatMessage {
            role: row.get(1)?,
            content: row.get(2)?,
            tool_calls,
            tool_call_id: row.get(4)?,
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }

    Ok(ConversationDetail {
        id: id.to_string(),
        title,
        created_at,
        updated_at,
        messages,
    })
}

/// Save a message to a conversation.
pub fn save_message(
    conn: &Connection,
    id: &str,
    conversation_id: &str,
    role: &str,
    content: &str,
    tool_calls: Option<&str>,
    tool_call_id: Option<&str>,
    model: Option<&str>,
    provider_id: Option<&str>,
    usage: Option<&str>,
    sort_order: i32,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, tool_calls,
                               tool_call_id, model, provider_id, usage, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            conversation_id,
            role,
            content,
            tool_calls,
            tool_call_id,
            model,
            provider_id,
            usage,
            sort_order,
        ],
    )?;

    // Update conversation updated_at
    conn.execute(
        "UPDATE conversations SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE id = ?1",
        params![conversation_id],
    )?;

    Ok(())
}

/// Delete a conversation (messages are cascade-deleted via FK).
pub fn delete_conversation(conn: &Connection, id: &str) -> Result<(), AppError> {
    let deleted = conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(AppError::NotFound {
            entity: "conversation".to_string(),
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Update a conversation's title.
pub fn update_conversation_title(conn: &Connection, id: &str, title: &str) -> Result<(), AppError> {
    let updated = conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE id = ?2",
        params![title, id],
    )?;
    if updated == 0 {
        return Err(AppError::NotFound {
            entity: "conversation".to_string(),
            id: id.to_string(),
        });
    }
    Ok(())
}

// ─── MCP Server Configuration CRUD ──────────────────────────────────

/// Add a new MCP server configuration.
pub fn add_mcp_server(
    conn: &Connection,
    id: &str,
    config: &McpServerConfig,
) -> Result<McpServerStatus, AppError> {
    let args_json = serde_json::to_string(&config.args.clone().unwrap_or_default())
        .unwrap_or_else(|_| "[]".to_string());
    let env_json = serde_json::to_string(&config.env.clone().unwrap_or_default())
        .unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO mcp_servers (id, name, command, args, env, working_dir, enabled, notes, auto_start)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            config.name,
            config.command,
            args_json,
            env_json,
            config.working_dir.as_deref().unwrap_or(""),
            config.enabled.unwrap_or(true) as i32,
            config.notes.as_deref().unwrap_or(""),
            config.auto_start.unwrap_or(false) as i32,
        ],
    )?;

    get_mcp_server(conn, id)
}

/// Remove an MCP server configuration.
pub fn remove_mcp_server(conn: &Connection, id: &str) -> Result<(), AppError> {
    let deleted = conn.execute("DELETE FROM mcp_servers WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(AppError::NotFound {
            entity: "mcp_server".to_string(),
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Get a single MCP server by ID.
pub fn get_mcp_server(conn: &Connection, id: &str) -> Result<McpServerStatus, AppError> {
    let result = conn
        .query_row(
            "SELECT id, name, command, args, env, enabled FROM mcp_servers WHERE id = ?1",
            params![id],
            |row| {
                let args_json: String = row.get(3)?;
                let env_json: String = row.get(4)?;
                let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();
                let env: Vec<String> = serde_json::from_str(&env_json).unwrap_or_default();

                Ok(McpServerStatus {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    command: row.get(2)?,
                    args,
                    env,
                    enabled: row.get::<_, i32>(5)? != 0,
                    running: false,
                    pid: None,
                    last_started_at: None,
                    last_exit_code: None,
                    tools: Vec::new(),
                })
            },
        )
        .optional()?;

    result.ok_or_else(|| AppError::NotFound {
        entity: "mcp_server".to_string(),
        id: id.to_string(),
    })
}

/// List all MCP servers from config.
pub fn list_mcp_servers(conn: &Connection) -> Result<Vec<McpServerStatus>, AppError> {
    let mut stmt = conn
        .prepare("SELECT id, name, command, args, env, enabled FROM mcp_servers ORDER BY name")?;

    let rows = stmt.query_map([], |row| {
        let args_json: String = row.get(3)?;
        let env_json: String = row.get(4)?;
        let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();
        let env: Vec<String> = serde_json::from_str(&env_json).unwrap_or_default();

        Ok(McpServerStatus {
            id: row.get(0)?,
            name: row.get(1)?,
            command: row.get(2)?,
            args,
            env,
            enabled: row.get::<_, i32>(5)? != 0,
            running: false,
            pid: None,
            last_started_at: None,
            last_exit_code: None,
            tools: Vec::new(),
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ─── Container Template Operations ──────────────────────────────────

/// List all container templates.
pub fn list_templates(conn: &Connection) -> Result<Vec<serde_json::Value>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, image, command, env, ports, volumes,
                cpu_cores, memory_mb, working_dir, labels, enabled, sort_order
         FROM container_templates
         WHERE enabled = 1
         ORDER BY sort_order, name",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "image": row.get::<_, String>(3)?,
            "command": row.get::<_, Option<String>>(4)?,
            "env": row.get::<_, String>(5)?,
            "ports": row.get::<_, String>(6)?,
            "volumes": row.get::<_, String>(7)?,
            "cpu_cores": row.get::<_, i32>(8)?,
            "memory_mb": row.get::<_, i64>(9)?,
            "working_dir": row.get::<_, Option<String>>(10)?,
            "labels": row.get::<_, String>(11)?,
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ─── Audit Log Operations ───────────────────────────────────────────

/// Query audit logs with optional filtering.
pub fn list_audit_logs(
    conn: &Connection,
    action: Option<&str>,
    target: Option<&str>,
    limit: u32,
) -> Result<Vec<serde_json::Value>, AppError> {
    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match (action, target) {
        (Some(a), Some(t)) => (
            "SELECT id, timestamp, action, target, details, user
                 FROM audit_log WHERE action = ?1 AND target = ?2
                 ORDER BY timestamp DESC LIMIT ?3"
                .to_string(),
            vec![
                Box::new(a.to_string()),
                Box::new(t.to_string()),
                Box::new(limit),
            ],
        ),
        (Some(a), None) => (
            "SELECT id, timestamp, action, target, details, user
                 FROM audit_log WHERE action = ?1
                 ORDER BY timestamp DESC LIMIT ?2"
                .to_string(),
            vec![Box::new(a.to_string()), Box::new(limit)],
        ),
        (None, Some(t)) => (
            "SELECT id, timestamp, action, target, details, user
                 FROM audit_log WHERE target = ?1
                 ORDER BY timestamp DESC LIMIT ?2"
                .to_string(),
            vec![Box::new(t.to_string()), Box::new(limit)],
        ),
        (None, None) => (
            "SELECT id, timestamp, action, target, details, user
                 FROM audit_log ORDER BY timestamp DESC LIMIT ?1"
                .to_string(),
            vec![Box::new(limit)],
        ),
    };

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|v| v.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "timestamp": row.get::<_, String>(1)?,
            "action": row.get::<_, String>(2)?,
            "target": row.get::<_, String>(3)?,
            "details": row.get::<_, Option<String>>(4)?,
            "user": row.get::<_, String>(5)?,
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ─── Path Utility Functions ──────────────────────────────────────────
//
// Platform-aware directory helpers used by the runtime, image management,
// and logging subsystems.  These mirror the helpers from the v1 `store`
// module and are the canonical way to locate CrateBay data on disk.

/// CrateBay configuration directory.
///
/// Override with `CRATEBAY_CONFIG_DIR`.  Platform defaults:
/// - macOS:   `~/Library/Application Support/com.cratebay.app`
/// - Linux:   `$XDG_CONFIG_HOME/cratebay` or `~/.config/cratebay`
/// - Windows: `%APPDATA%\cratebay`
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CRATEBAY_CONFIG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("com.cratebay.app");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("cratebay");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join("cratebay");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("cratebay");
        }
    }

    std::env::temp_dir().join("cratebay")
}

/// CrateBay persistent data directory.
///
/// Override with `CRATEBAY_DATA_DIR`.  Platform defaults:
/// - Linux: `$XDG_DATA_HOME/cratebay` or `~/.local/share/cratebay`
/// - macOS / Windows: same as [`config_dir()`]
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CRATEBAY_DATA_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("cratebay");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("cratebay");
        }
    }

    // macOS / Windows default: same as config_dir.
    config_dir()
}

/// CrateBay log directory.
///
/// Override with `CRATEBAY_LOG_DIR`.
pub fn log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CRATEBAY_LOG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "linux")]
    {
        return data_dir();
    }

    #[cfg(not(target_os = "linux"))]
    {
        config_dir()
    }
}

/// Console log path for a runtime VM.
pub fn vm_console_log_path(vm_id: &str) -> PathBuf {
    data_dir().join("vms").join(vm_id).join("console.log")
}

/// Write `bytes` atomically: writes to a temporary file then renames.
///
/// Creates parent directories if necessary.  Safe for concurrent use
/// from multiple processes.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;

    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp");
    let unique = format!(
        "{}.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        file_name
    );
    let tmp_path = dir.join(format!(".{}.tmp", unique));

    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }

    match std::fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Windows may fail rename if destination exists.
            if path.exists() {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&tmp_path, path).map_err(|_| e)?;
                return Ok(());
            }
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────

fn home_dir() -> Result<PathBuf, AppError> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| AppError::Runtime("Cannot determine home directory".into()))
}

/// Compute a key hint from a plaintext API key (last 4 chars).
pub fn compute_key_hint(api_key: &str) -> String {
    if api_key.len() >= 4 {
        format!("...{}", &api_key[api_key.len() - 4..])
    } else {
        "****".to_string()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )
        .unwrap();
        migrate(&conn).unwrap();
        conn
    }

    #[test]
    fn test_migrations_apply_cleanly() {
        let conn = setup_db();
        // Verify _migrations table has our migration
        let version: u32 = conn
            .query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_migrations_idempotent() {
        let conn = setup_db();
        // Running migrate again should be a no-op
        migrate(&conn).unwrap();
        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_settings_crud() {
        let conn = setup_db();

        // Default settings should exist
        let theme = get_setting(&conn, "theme").unwrap();
        assert_eq!(theme, Some("system".to_string()));

        // Update setting
        set_setting(&conn, "theme", "dark").unwrap();
        let theme = get_setting(&conn, "theme").unwrap();
        assert_eq!(theme, Some("dark".to_string()));

        // New setting
        set_setting(&conn, "custom.key", "custom_value").unwrap();
        let value = get_setting(&conn, "custom.key").unwrap();
        assert_eq!(value, Some("custom_value".to_string()));

        // Non-existent setting
        let missing = get_setting(&conn, "nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_provider_crud() {
        let conn = setup_db();

        // Create provider
        create_provider(
            &conn,
            "openai-1",
            "OpenAI",
            "https://api.openai.com",
            &ApiFormat::OpenAiCompletions,
        )
        .unwrap();

        // List providers
        let providers = list_providers(&conn).unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "OpenAI");
        assert!(!providers[0].has_api_key);

        // Update provider
        update_provider(&conn, "openai-1", Some("OpenAI Updated"), None, None, None).unwrap();
        let provider = get_provider(&conn, "openai-1").unwrap();
        assert_eq!(provider.name, "OpenAI Updated");

        // Delete provider
        delete_provider(&conn, "openai-1").unwrap();
        let providers = list_providers(&conn).unwrap();
        assert!(providers.is_empty());
    }

    #[test]
    fn test_model_operations() {
        let conn = setup_db();

        create_provider(
            &conn,
            "test-provider",
            "Test",
            "https://api.test.com",
            &ApiFormat::OpenAiCompletions,
        )
        .unwrap();

        // Save models
        let models = vec![
            ("gpt-4o".to_string(), "GPT-4o".to_string(), false),
            ("gpt-4o-mini".to_string(), "GPT-4o Mini".to_string(), false),
        ];
        save_models(&conn, "test-provider", &models).unwrap();

        // List models
        let listed = list_models(&conn, "test-provider").unwrap();
        assert_eq!(listed.len(), 2);
        assert!(!listed[0].is_enabled);

        // Toggle model
        toggle_model(&conn, "test-provider", "gpt-4o", true).unwrap();
        let listed = list_models(&conn, "test-provider").unwrap();
        let gpt4o = listed.iter().find(|m| m.id == "gpt-4o").unwrap();
        assert!(gpt4o.is_enabled);
    }

    #[test]
    fn test_conversation_crud() {
        let conn = setup_db();

        // Create conversation
        create_conversation(&conn, "conv-1", "Test Conversation").unwrap();

        // Save messages
        save_message(
            &conn, "msg-1", "conv-1", "user", "Hello!", None, None, None, None, None, 0,
        )
        .unwrap();
        save_message(
            &conn,
            "msg-2",
            "conv-1",
            "assistant",
            "Hi there!",
            None,
            None,
            Some("gpt-4o"),
            Some("openai"),
            None,
            1,
        )
        .unwrap();

        // List conversations
        let convs = list_conversations(&conn, 50, 0).unwrap();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].message_count, 2);

        // Get conversation detail
        let detail = get_conversation(&conn, "conv-1").unwrap();
        assert_eq!(detail.messages.len(), 2);
        assert_eq!(detail.messages[0].role, "user");
        assert_eq!(detail.messages[1].role, "assistant");

        // Delete conversation
        delete_conversation(&conn, "conv-1").unwrap();
        let convs = list_conversations(&conn, 50, 0).unwrap();
        assert!(convs.is_empty());
    }

    #[test]
    fn test_default_templates_seeded() {
        let conn = setup_db();
        let templates = list_templates(&conn).unwrap();
        assert_eq!(templates.len(), 4);
    }

    #[test]
    fn test_compute_key_hint() {
        assert_eq!(compute_key_hint("sk-abcdef1234"), "...1234");
        assert_eq!(compute_key_hint("ab"), "****");
    }

    #[test]
    fn test_cascade_delete_provider_removes_models() {
        let conn = setup_db();

        create_provider(
            &conn,
            "cascade-test",
            "Cascade",
            "https://api.test.com",
            &ApiFormat::Anthropic,
        )
        .unwrap();

        let models = vec![("model-1".to_string(), "Model 1".to_string(), false)];
        save_models(&conn, "cascade-test", &models).unwrap();

        // Verify model exists
        let listed = list_models(&conn, "cascade-test").unwrap();
        assert_eq!(listed.len(), 1);

        // Delete provider — should cascade delete models
        delete_provider(&conn, "cascade-test").unwrap();
        let listed = list_models(&conn, "cascade-test").unwrap();
        assert!(listed.is_empty());
    }

    #[test]
    fn test_api_key_crud() {
        let conn = setup_db();

        create_provider(
            &conn,
            "key-test",
            "KeyTest",
            "https://api.test.com",
            &ApiFormat::OpenAiCompletions,
        )
        .unwrap();

        // Save API key (mock encrypted data)
        let encrypted = vec![1u8, 2, 3, 4, 5];
        let nonce = vec![10u8, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21];
        save_api_key(&conn, "key-test", &encrypted, &nonce, "...1234").unwrap();

        // Load API key
        let (loaded_enc, loaded_nonce) = load_api_key(&conn, "key-test").unwrap();
        assert_eq!(loaded_enc, encrypted);
        assert_eq!(loaded_nonce, nonce);

        // Get hint
        let hint = get_api_key_hint(&conn, "key-test").unwrap();
        assert_eq!(hint, Some("...1234".to_string()));

        // Delete API key
        delete_api_key(&conn, "key-test").unwrap();
        assert!(load_api_key(&conn, "key-test").is_err());
    }

    // ── SQL injection prevention tests (testing-spec.md §7.4) ──

    #[test]
    fn test_sql_injection_in_conversation_title() {
        let conn = setup_db();
        // Attempt SQL injection via conversation title
        let malicious_title = "'; DROP TABLE conversations; --";
        create_conversation(&conn, "inject-1", malicious_title).unwrap();

        // Verify the conversation exists with the literal title (not executed as SQL)
        let detail = get_conversation(&conn, "inject-1").unwrap();
        assert_eq!(detail.title, malicious_title);

        // Verify tables still exist
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            .unwrap();
        assert!(count >= 1, "conversations table must still exist");
    }

    #[test]
    fn test_sql_injection_in_message_content() {
        let conn = setup_db();
        create_conversation(&conn, "inject-msg-conv", "Test").unwrap();

        let malicious_content = "Hello'; DELETE FROM messages WHERE '1'='1";
        save_message(
            &conn,
            "inject-msg-1",
            "inject-msg-conv",
            "user",
            malicious_content,
            None,
            None,
            None,
            None,
            None,
            0,
        )
        .unwrap();

        // Verify the message was stored literally
        let detail = get_conversation(&conn, "inject-msg-conv").unwrap();
        assert_eq!(detail.messages.len(), 1);
        assert_eq!(detail.messages[0].content, malicious_content);
    }

    #[test]
    fn test_sql_injection_in_provider_name() {
        let conn = setup_db();
        let malicious_name = "Evil'; DROP TABLE providers; --";
        create_provider(
            &conn,
            "inject-provider",
            malicious_name,
            "https://api.evil.com",
            &ApiFormat::OpenAiCompletions,
        )
        .unwrap();

        let provider = get_provider(&conn, "inject-provider").unwrap();
        assert_eq!(provider.name, malicious_name);

        // Tables still intact
        let providers = list_providers(&conn).unwrap();
        assert!(!providers.is_empty());
    }

    #[test]
    fn test_sql_injection_in_setting_key() {
        let conn = setup_db();
        let malicious_key = "key'; DROP TABLE settings; --";
        set_setting(&conn, malicious_key, "value").unwrap();

        let value = get_setting(&conn, malicious_key).unwrap();
        assert_eq!(value, Some("value".to_string()));

        // Settings table still intact
        let theme = get_setting(&conn, "theme").unwrap();
        assert!(theme.is_some());
    }

    // ── API Key leakage prevention tests (testing-spec.md §7.3) ──

    #[test]
    fn test_api_key_not_in_provider_listing() {
        let conn = setup_db();

        create_provider(
            &conn,
            "leakage-test",
            "LeakTest",
            "https://api.test.com",
            &ApiFormat::OpenAiCompletions,
        )
        .unwrap();

        // Save an API key
        let encrypted = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let nonce = vec![1u8; 12];
        save_api_key(&conn, "leakage-test", &encrypted, &nonce, "...test").unwrap();

        // List providers — should have has_api_key = true but NO raw key
        let providers = list_providers(&conn).unwrap();
        let provider = providers.iter().find(|p| p.id == "leakage-test").unwrap();
        assert!(provider.has_api_key);

        // Serialize the provider listing to JSON and verify no key material
        let json = serde_json::to_string(&providers).unwrap();
        assert!(
            !json.contains("DEADBEEF"),
            "Encrypted key bytes must not appear in provider listing"
        );
        // The provider struct has no api_key field, only has_api_key boolean
        assert!(
            !json.contains("encrypted_key"),
            "Encrypted key field must not appear in provider listing"
        );
    }

    #[test]
    fn test_api_key_hint_does_not_reveal_full_key() {
        assert_eq!(compute_key_hint("sk-abcdef123456789012345678"), "...5678");
        // Hint is always the last 4 characters
        assert_eq!(compute_key_hint("short"), "...hort");
        // Very short keys get masked
        assert_eq!(compute_key_hint("ab"), "****");
        assert_eq!(compute_key_hint(""), "****");
    }

    // ── Path utility function tests ──

    #[test]
    fn test_config_dir_returns_nonempty() {
        let dir = config_dir();
        assert!(
            !dir.as_os_str().is_empty(),
            "config_dir should not be empty"
        );
    }

    #[test]
    fn test_data_dir_returns_nonempty() {
        let dir = data_dir();
        assert!(!dir.as_os_str().is_empty(), "data_dir should not be empty");
    }

    #[test]
    fn test_log_dir_returns_nonempty() {
        let dir = log_dir();
        assert!(!dir.as_os_str().is_empty(), "log_dir should not be empty");
    }

    #[test]
    fn test_vm_console_log_path_contains_vm_id() {
        let path = vm_console_log_path("test-vm-42");
        let s = path.to_string_lossy();
        assert!(s.contains("test-vm-42"), "path should contain VM id");
        assert!(
            s.ends_with("console.log"),
            "path should end with console.log"
        );
    }

    #[test]
    fn test_write_atomic_creates_file_and_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("test.txt");

        write_atomic(&path, b"hello").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("hello"));
    }

    #[test]
    fn test_write_atomic_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("over.txt");

        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("second"));
    }
}
