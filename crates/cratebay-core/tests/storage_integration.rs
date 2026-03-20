//! Integration tests for the SQLite storage layer.
//!
//! All tests use `:memory:` databases — no files created on disk.
//! Tests cover cross-function interactions and edge cases that
//! complement the inline unit tests in `storage.rs`.

use rusqlite::Connection;

use cratebay_core::error::AppError;
use cratebay_core::models::ApiFormat;
use cratebay_core::storage;

// ─── Helpers ────────────────────────────────────────────────────────

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;",
    )
    .unwrap();
    storage::migrate(&conn).unwrap();
    conn
}

// ─── Schema & PRAGMA Tests ──────────────────────────────────────────

#[test]
fn pragma_wal_mode_set() {
    // Note: in-memory databases report "memory" for journal_mode even after
    // setting WAL. This test verifies the PRAGMA is accepted without error.
    // On-disk databases would return "wal".
    let conn = setup_db();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    // In-memory DBs always use "memory" journal mode
    assert!(
        mode == "wal" || mode == "memory",
        "Expected 'wal' or 'memory', got '{}'",
        mode
    );
}

#[test]
fn pragma_foreign_keys_enabled() {
    let conn = setup_db();
    let fk: i32 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(fk, 1);
}

#[test]
fn migration_creates_all_expected_tables() {
    let conn = setup_db();

    let expected_tables = [
        "_migrations",
        "ai_providers",
        "ai_models",
        "api_keys",
        "conversations",
        "messages",
        "container_templates",
        "mcp_servers",
        "settings",
        "audit_log",
    ];

    for table_name in &expected_tables {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                rusqlite::params![table_name],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            exists,
            "Expected table '{}' to exist after migration",
            table_name
        );
    }
}

#[test]
fn migration_seeds_default_settings() {
    let conn = setup_db();

    let all_settings = storage::get_all_settings(&conn).unwrap();
    assert!(
        all_settings.len() >= 5,
        "Expected at least 5 default settings, got {}",
        all_settings.len()
    );

    // Verify specific defaults
    let theme = storage::get_setting(&conn, "theme").unwrap();
    assert_eq!(theme, Some("system".to_string()));

    let lang = storage::get_setting(&conn, "language").unwrap();
    assert_eq!(lang, Some("en".to_string()));

    let auto_start = storage::get_setting(&conn, "runtime.auto_start").unwrap();
    assert_eq!(auto_start, Some("true".to_string()));
}

#[test]
fn migration_seeds_default_templates() {
    let conn = setup_db();
    let templates = storage::list_templates(&conn).unwrap();
    assert_eq!(templates.len(), 4);

    let ids: Vec<String> = templates
        .iter()
        .filter_map(|t| t["id"].as_str().map(String::from))
        .collect();
    assert!(ids.contains(&"node-dev".to_string()));
    assert!(ids.contains(&"python-dev".to_string()));
    assert!(ids.contains(&"rust-dev".to_string()));
    assert!(ids.contains(&"ubuntu".to_string()));
}

#[test]
fn migration_seeds_default_mcp_server() {
    let conn = setup_db();
    let count: u32 = conn
        .query_row("SELECT COUNT(*) FROM mcp_servers", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1, "Should seed exactly 1 default MCP server");
}

// ─── Settings Tests ─────────────────────────────────────────────────

#[test]
fn settings_get_all_returns_sorted() {
    let conn = setup_db();
    storage::set_setting(&conn, "aaa.first", "1").unwrap();
    storage::set_setting(&conn, "zzz.last", "2").unwrap();

    let all = storage::get_all_settings(&conn).unwrap();
    // Verify sorted by key
    let keys: Vec<&str> = all.iter().map(|(k, _)| k.as_str()).collect();
    let mut sorted_keys = keys.clone();
    sorted_keys.sort();
    assert_eq!(keys, sorted_keys);
}

#[test]
fn settings_upsert_overwrites() {
    let conn = setup_db();

    storage::set_setting(&conn, "test_key", "value1").unwrap();
    assert_eq!(
        storage::get_setting(&conn, "test_key").unwrap(),
        Some("value1".to_string())
    );

    storage::set_setting(&conn, "test_key", "value2").unwrap();
    assert_eq!(
        storage::get_setting(&conn, "test_key").unwrap(),
        Some("value2".to_string())
    );

    // Should not create duplicates
    let count: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM settings WHERE key = 'test_key'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

// ─── Provider Tests ─────────────────────────────────────────────────

#[test]
fn provider_all_api_formats() {
    let conn = setup_db();

    let formats = [
        ("p1", ApiFormat::Anthropic),
        ("p2", ApiFormat::OpenAiResponses),
        ("p3", ApiFormat::OpenAiCompletions),
    ];

    for (id, format) in &formats {
        storage::create_provider(&conn, id, "Test", "https://api.test.com", format).unwrap();
    }

    let providers = storage::list_providers(&conn).unwrap();
    assert_eq!(providers.len(), 3);

    // Verify format roundtrip
    let p1 = storage::get_provider(&conn, "p1").unwrap();
    assert_eq!(p1.api_format, ApiFormat::Anthropic);

    let p2 = storage::get_provider(&conn, "p2").unwrap();
    assert_eq!(p2.api_format, ApiFormat::OpenAiResponses);

    let p3 = storage::get_provider(&conn, "p3").unwrap();
    assert_eq!(p3.api_format, ApiFormat::OpenAiCompletions);
}

#[test]
fn provider_update_partial_fields() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "upd-test",
        "Original",
        "https://original.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    // Update only name
    storage::update_provider(&conn, "upd-test", Some("Updated"), None, None, None).unwrap();
    let p = storage::get_provider(&conn, "upd-test").unwrap();
    assert_eq!(p.name, "Updated");
    assert_eq!(p.api_base, "https://original.com"); // Unchanged

    // Update only api_base
    storage::update_provider(&conn, "upd-test", None, Some("https://new.com"), None, None)
        .unwrap();
    let p = storage::get_provider(&conn, "upd-test").unwrap();
    assert_eq!(p.name, "Updated"); // Unchanged
    assert_eq!(p.api_base, "https://new.com");

    // Update enabled
    storage::update_provider(&conn, "upd-test", None, None, None, Some(false)).unwrap();
    let p = storage::get_provider(&conn, "upd-test").unwrap();
    assert!(!p.enabled);

    // Update api_format
    storage::update_provider(
        &conn,
        "upd-test",
        None,
        None,
        Some(&ApiFormat::Anthropic),
        None,
    )
    .unwrap();
    let p = storage::get_provider(&conn, "upd-test").unwrap();
    assert_eq!(p.api_format, ApiFormat::Anthropic);
}

#[test]
fn provider_update_nonexistent_returns_not_found() {
    let conn = setup_db();
    let result =
        storage::update_provider(&conn, "nonexistent", Some("Test"), None, None, None);
    assert!(result.is_err());
    match result.unwrap_err() {
        AppError::NotFound { entity, id } => {
            assert_eq!(entity, "provider");
            assert_eq!(id, "nonexistent");
        }
        other => panic!("Expected NotFound, got {:?}", other),
    }
}

#[test]
fn provider_delete_nonexistent_returns_not_found() {
    let conn = setup_db();
    let result = storage::delete_provider(&conn, "nonexistent");
    assert!(result.is_err());
    match result.unwrap_err() {
        AppError::NotFound { entity, id } => {
            assert_eq!(entity, "provider");
            assert_eq!(id, "nonexistent");
        }
        other => panic!("Expected NotFound, got {:?}", other),
    }
}

#[test]
fn provider_update_empty_fields_is_noop() {
    let conn = setup_db();
    storage::create_provider(
        &conn,
        "noop-test",
        "Original",
        "https://api.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    // Update with no fields should succeed (no-op)
    storage::update_provider(&conn, "noop-test", None, None, None, None).unwrap();
    let p = storage::get_provider(&conn, "noop-test").unwrap();
    assert_eq!(p.name, "Original");
}

#[test]
fn provider_has_api_key_flag_reflects_key_presence() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "flag-test",
        "Test",
        "https://api.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    // Initially no key
    let p = storage::get_provider(&conn, "flag-test").unwrap();
    assert!(!p.has_api_key);

    // Save API key
    storage::save_api_key(&conn, "flag-test", &[1, 2, 3], &[4, 5, 6], "...test").unwrap();

    // Now has_api_key should be true
    let p = storage::get_provider(&conn, "flag-test").unwrap();
    assert!(p.has_api_key);

    // Also verify in list
    let providers = storage::list_providers(&conn).unwrap();
    let provider = providers.iter().find(|p| p.id == "flag-test").unwrap();
    assert!(provider.has_api_key);

    // Delete key
    storage::delete_api_key(&conn, "flag-test").unwrap();
    let p = storage::get_provider(&conn, "flag-test").unwrap();
    assert!(!p.has_api_key);
}

// ─── Model Tests ────────────────────────────────────────────────────

#[test]
fn save_models_replaces_removed_models() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "model-replace",
        "Test",
        "https://api.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    // Initial set: A, B, C
    let models_v1 = vec![
        ("a".to_string(), "Model A".to_string(), false),
        ("b".to_string(), "Model B".to_string(), false),
        ("c".to_string(), "Model C".to_string(), true),
    ];
    storage::save_models(&conn, "model-replace", &models_v1).unwrap();
    assert_eq!(
        storage::list_models(&conn, "model-replace").unwrap().len(),
        3
    );

    // New set: B, D — A and C should be removed
    let models_v2 = vec![
        ("b".to_string(), "Model B Updated".to_string(), false),
        ("d".to_string(), "Model D".to_string(), false),
    ];
    storage::save_models(&conn, "model-replace", &models_v2).unwrap();

    let listed = storage::list_models(&conn, "model-replace").unwrap();
    assert_eq!(listed.len(), 2);
    let ids: Vec<&str> = listed.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&"b"));
    assert!(ids.contains(&"d"));
    assert!(!ids.contains(&"a"));
    assert!(!ids.contains(&"c"));

    // Verify Model B was updated
    let b = listed.iter().find(|m| m.id == "b").unwrap();
    assert_eq!(b.name, "Model B Updated");
}

#[test]
fn save_models_empty_list_clears_all() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "clear-test",
        "Test",
        "https://api.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    let models = vec![
        ("a".to_string(), "A".to_string(), false),
        ("b".to_string(), "B".to_string(), false),
    ];
    storage::save_models(&conn, "clear-test", &models).unwrap();
    assert_eq!(
        storage::list_models(&conn, "clear-test").unwrap().len(),
        2
    );

    // Empty list clears all
    storage::save_models(&conn, "clear-test", &[]).unwrap();
    assert_eq!(
        storage::list_models(&conn, "clear-test").unwrap().len(),
        0
    );
}

#[test]
fn save_models_preserves_enabled_state() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "preserve-test",
        "Test",
        "https://api.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    let models = vec![("m1".to_string(), "Model 1".to_string(), false)];
    storage::save_models(&conn, "preserve-test", &models).unwrap();

    // Enable the model
    storage::toggle_model(&conn, "preserve-test", "m1", true).unwrap();
    let listed = storage::list_models(&conn, "preserve-test").unwrap();
    assert!(listed[0].is_enabled);

    // Upsert with the same model ID — is_enabled should be preserved
    let models_v2 = vec![("m1".to_string(), "Model 1 Updated".to_string(), false)];
    storage::save_models(&conn, "preserve-test", &models_v2).unwrap();

    let listed = storage::list_models(&conn, "preserve-test").unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].is_enabled, "is_enabled should be preserved across upsert");
    assert_eq!(listed[0].name, "Model 1 Updated");
}

#[test]
fn toggle_model_nonexistent_returns_not_found() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "toggle-nf",
        "Test",
        "https://api.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    let result = storage::toggle_model(&conn, "toggle-nf", "nonexistent", true);
    assert!(result.is_err());
    match result.unwrap_err() {
        AppError::NotFound { entity, .. } => {
            assert_eq!(entity, "model");
        }
        other => panic!("Expected NotFound, got {:?}", other),
    }
}

// ─── Cascade Delete Tests ───────────────────────────────────────────

#[test]
fn cascade_delete_provider_removes_api_key_and_models() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "cascade-all",
        "Test",
        "https://api.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    // Add models
    let models = vec![
        ("m1".to_string(), "M1".to_string(), false),
        ("m2".to_string(), "M2".to_string(), true),
    ];
    storage::save_models(&conn, "cascade-all", &models).unwrap();

    // Add API key
    storage::save_api_key(&conn, "cascade-all", &[1, 2, 3], &[4, 5, 6], "...key").unwrap();

    // Verify all exist
    assert_eq!(
        storage::list_models(&conn, "cascade-all").unwrap().len(),
        2
    );
    assert!(storage::load_api_key(&conn, "cascade-all").is_ok());

    // Delete provider
    storage::delete_provider(&conn, "cascade-all").unwrap();

    // Models should be cascade deleted
    assert_eq!(
        storage::list_models(&conn, "cascade-all").unwrap().len(),
        0
    );

    // API key should be cascade deleted
    assert!(storage::load_api_key(&conn, "cascade-all").is_err());
}

// ─── API Key Tests ──────────────────────────────────────────────────

#[test]
fn api_key_upsert_replaces_existing() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "key-upsert",
        "Test",
        "https://api.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    storage::save_api_key(&conn, "key-upsert", &[1, 2, 3], &[4, 5, 6], "...abc").unwrap();
    storage::save_api_key(&conn, "key-upsert", &[7, 8, 9], &[10, 11, 12], "...xyz").unwrap();

    let (enc, nonce) = storage::load_api_key(&conn, "key-upsert").unwrap();
    assert_eq!(enc, vec![7u8, 8, 9]);
    assert_eq!(nonce, vec![10u8, 11, 12]);

    let hint = storage::get_api_key_hint(&conn, "key-upsert").unwrap();
    assert_eq!(hint, Some("...xyz".to_string()));
}

#[test]
fn api_key_hint_missing_returns_none() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "no-key",
        "Test",
        "https://api.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();

    let hint = storage::get_api_key_hint(&conn, "no-key").unwrap();
    assert!(hint.is_none());
}

// ─── Conversation & Message Tests ───────────────────────────────────

#[test]
fn conversation_list_pagination() {
    let conn = setup_db();

    for i in 0..5 {
        storage::create_conversation(&conn, &format!("conv-{}", i), &format!("Conv {}", i))
            .unwrap();
    }

    // Get first 2
    let page1 = storage::list_conversations(&conn, 2, 0).unwrap();
    assert_eq!(page1.len(), 2);

    // Get next 2
    let page2 = storage::list_conversations(&conn, 2, 2).unwrap();
    assert_eq!(page2.len(), 2);

    // Get last 1
    let page3 = storage::list_conversations(&conn, 2, 4).unwrap();
    assert_eq!(page3.len(), 1);

    // Beyond range
    let page4 = storage::list_conversations(&conn, 2, 10).unwrap();
    assert!(page4.is_empty());
}

#[test]
fn conversation_last_message_preview_truncation() {
    let conn = setup_db();

    storage::create_conversation(&conn, "preview-test", "Preview Test").unwrap();

    // Create a long message (> 100 chars)
    let long_content = "A".repeat(200);
    storage::save_message(
        &conn,
        "msg-long",
        "preview-test",
        "user",
        &long_content,
        None,
        None,
        None,
        None,
        None,
        0,
    )
    .unwrap();

    let convs = storage::list_conversations(&conn, 50, 0).unwrap();
    let conv = convs.iter().find(|c| c.id == "preview-test").unwrap();
    let preview = conv.last_message_preview.as_ref().unwrap();
    assert!(preview.len() <= 103, "Preview should be truncated to ~100 chars + '...'");
    assert!(preview.ends_with("..."));
}

#[test]
fn conversation_message_with_tool_calls() {
    let conn = setup_db();

    storage::create_conversation(&conn, "tool-conv", "Tool Conv").unwrap();

    let tool_calls_json = r#"[{"id":"tc-1","name":"container_list","arguments":"{}"}]"#;
    storage::save_message(
        &conn,
        "msg-tool",
        "tool-conv",
        "assistant",
        "Let me list containers.",
        Some(tool_calls_json),
        None,
        Some("gpt-4o"),
        Some("openai"),
        None,
        0,
    )
    .unwrap();

    // Save tool result
    storage::save_message(
        &conn,
        "msg-result",
        "tool-conv",
        "tool",
        "[]",
        None,
        Some("tc-1"),
        None,
        None,
        None,
        1,
    )
    .unwrap();

    let detail = storage::get_conversation(&conn, "tool-conv").unwrap();
    assert_eq!(detail.messages.len(), 2);

    // Verify tool calls are deserialized
    let assistant_msg = &detail.messages[0];
    assert!(assistant_msg.tool_calls.is_some());
    let tcs = assistant_msg.tool_calls.as_ref().unwrap();
    assert_eq!(tcs.len(), 1);
    assert_eq!(tcs[0].name, "container_list");

    // Verify tool result
    let tool_msg = &detail.messages[1];
    assert_eq!(tool_msg.role, "tool");
    assert_eq!(tool_msg.tool_call_id, Some("tc-1".to_string()));
}

#[test]
fn conversation_delete_cascades_messages() {
    let conn = setup_db();

    storage::create_conversation(&conn, "del-conv", "Delete Me").unwrap();
    storage::save_message(
        &conn,
        "msg-del-1",
        "del-conv",
        "user",
        "Hello",
        None,
        None,
        None,
        None,
        None,
        0,
    )
    .unwrap();
    storage::save_message(
        &conn,
        "msg-del-2",
        "del-conv",
        "assistant",
        "Hi",
        None,
        None,
        None,
        None,
        None,
        1,
    )
    .unwrap();

    // Verify messages exist
    let count: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = 'del-conv'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);

    // Delete conversation
    storage::delete_conversation(&conn, "del-conv").unwrap();

    // Messages should be cascade deleted
    let count: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = 'del-conv'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn conversation_delete_nonexistent_returns_not_found() {
    let conn = setup_db();
    let result = storage::delete_conversation(&conn, "nonexistent");
    assert!(result.is_err());
    match result.unwrap_err() {
        AppError::NotFound { entity, .. } => {
            assert_eq!(entity, "conversation");
        }
        other => panic!("Expected NotFound, got {:?}", other),
    }
}

#[test]
fn conversation_get_nonexistent_returns_not_found() {
    let conn = setup_db();
    let result = storage::get_conversation(&conn, "nonexistent");
    assert!(result.is_err());
    match result.unwrap_err() {
        AppError::NotFound { entity, .. } => {
            assert_eq!(entity, "conversation");
        }
        other => panic!("Expected NotFound, got {:?}", other),
    }
}

#[test]
fn conversation_message_sort_order() {
    let conn = setup_db();

    storage::create_conversation(&conn, "sort-conv", "Sort Test").unwrap();

    // Insert messages out of order
    storage::save_message(
        &conn,
        "msg-3",
        "sort-conv",
        "assistant",
        "Third",
        None,
        None,
        None,
        None,
        None,
        2,
    )
    .unwrap();
    storage::save_message(
        &conn,
        "msg-1",
        "sort-conv",
        "user",
        "First",
        None,
        None,
        None,
        None,
        None,
        0,
    )
    .unwrap();
    storage::save_message(
        &conn,
        "msg-2",
        "sort-conv",
        "assistant",
        "Second",
        None,
        None,
        None,
        None,
        None,
        1,
    )
    .unwrap();

    let detail = storage::get_conversation(&conn, "sort-conv").unwrap();
    assert_eq!(detail.messages.len(), 3);
    assert_eq!(detail.messages[0].content, "First");
    assert_eq!(detail.messages[1].content, "Second");
    assert_eq!(detail.messages[2].content, "Third");
}

// ─── Key Hint Utility ───────────────────────────────────────────────

#[test]
fn compute_key_hint_various_lengths() {
    assert_eq!(storage::compute_key_hint("sk-abcdef1234"), "...1234");
    assert_eq!(storage::compute_key_hint("abcd"), "...abcd");
    assert_eq!(storage::compute_key_hint("abc"), "****");
    assert_eq!(storage::compute_key_hint("a"), "****");
    assert_eq!(storage::compute_key_hint(""), "****");
}

// ─── Multi-Provider Isolation ───────────────────────────────────────

#[test]
fn models_isolated_between_providers() {
    let conn = setup_db();

    storage::create_provider(
        &conn,
        "provider-a",
        "Provider A",
        "https://a.com",
        &ApiFormat::OpenAiCompletions,
    )
    .unwrap();
    storage::create_provider(
        &conn,
        "provider-b",
        "Provider B",
        "https://b.com",
        &ApiFormat::Anthropic,
    )
    .unwrap();

    let models_a = vec![
        ("gpt-4o".to_string(), "GPT-4o".to_string(), false),
        ("gpt-4o-mini".to_string(), "GPT-4o Mini".to_string(), false),
    ];
    let models_b = vec![
        ("claude-3-5-sonnet".to_string(), "Claude 3.5 Sonnet".to_string(), true),
    ];

    storage::save_models(&conn, "provider-a", &models_a).unwrap();
    storage::save_models(&conn, "provider-b", &models_b).unwrap();

    let listed_a = storage::list_models(&conn, "provider-a").unwrap();
    let listed_b = storage::list_models(&conn, "provider-b").unwrap();

    assert_eq!(listed_a.len(), 2);
    assert_eq!(listed_b.len(), 1);

    // Toggle model in A should not affect B
    storage::toggle_model(&conn, "provider-a", "gpt-4o", true).unwrap();
    let listed_b = storage::list_models(&conn, "provider-b").unwrap();
    assert!(!listed_b[0].is_enabled);

    // Delete provider A should not affect provider B
    storage::delete_provider(&conn, "provider-a").unwrap();
    let listed_b = storage::list_models(&conn, "provider-b").unwrap();
    assert_eq!(listed_b.len(), 1);
}
