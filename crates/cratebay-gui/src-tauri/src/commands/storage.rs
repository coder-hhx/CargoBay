//! Storage-related Tauri commands (settings, API keys, conversations).

use tauri::State;

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::models::{
    AuditAction, ConversationDetail, ConversationSummary, SaveMessageRequest,
};
use cratebay_core::MutexExt;
use cratebay_core::{audit, storage};

/// Get a setting value by key.
#[tauri::command]
pub async fn settings_get(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, AppError> {
    let db = state.db.lock_or_recover()?;
    storage::get_setting(&db, &key)
}

/// Update a setting value.
#[tauri::command]
pub async fn settings_update(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), AppError> {
    let db = state.db.lock_or_recover()?;
    storage::set_setting(&db, &key, &value)?;
    audit::log_action(
        &db,
        &AuditAction::SettingsUpdate,
        &key,
        Some(&value),
        "user",
    )?;
    Ok(())
}

/// Save an encrypted API key for a provider.
#[tauri::command]
pub async fn api_key_save(
    state: State<'_, AppState>,
    provider_id: String,
    api_key: String,
) -> Result<(), AppError> {
    let db = state.db.lock_or_recover()?;
    let hint = storage::compute_key_hint(&api_key);
    // Store as plaintext bytes for now (real encryption with keyring to be added)
    storage::save_api_key(&db, &provider_id, api_key.as_bytes(), &[0u8; 12], &hint)?;
    audit::log_action(&db, &AuditAction::ApiKeySave, &provider_id, None, "user")?;
    Ok(())
}

/// Delete a stored API key.
#[tauri::command]
pub async fn api_key_delete(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<(), AppError> {
    let db = state.db.lock_or_recover()?;
    storage::delete_api_key(&db, &provider_id)?;
    audit::log_action(&db, &AuditAction::ApiKeyDelete, &provider_id, None, "user")?;
    Ok(())
}

/// List all conversations.
#[tauri::command]
pub async fn conversation_list(
    state: State<'_, AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ConversationSummary>, AppError> {
    let db = state.db.lock_or_recover()?;
    storage::list_conversations(&db, limit.unwrap_or(50), offset.unwrap_or(0))
}

/// Get a conversation with all its messages.
#[tauri::command]
pub async fn conversation_get_messages(
    state: State<'_, AppState>,
    id: String,
) -> Result<ConversationDetail, AppError> {
    let db = state.db.lock_or_recover()?;
    storage::get_conversation(&db, &id)
}

/// Create a new conversation.
#[tauri::command]
pub async fn conversation_create(
    state: State<'_, AppState>,
    title: Option<String>,
) -> Result<ConversationDetail, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let title = title.unwrap_or_else(|| "New Conversation".to_string());

    let db = state.db.lock_or_recover()?;
    storage::create_conversation(&db, &id, &title)?;
    audit::log_action(
        &db,
        &AuditAction::ConversationCreate,
        &id,
        Some(&title),
        "user",
    )?;

    storage::get_conversation(&db, &id)
}

/// Delete a conversation and all its messages.
#[tauri::command]
pub async fn conversation_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let db = state.db.lock_or_recover()?;
    storage::delete_conversation(&db, &id)?;
    audit::log_action(&db, &AuditAction::ConversationDelete, &id, None, "user")?;
    Ok(())
}

/// Save a message to a conversation.
#[tauri::command]
pub async fn conversation_save_message(
    state: State<'_, AppState>,
    session_id: String,
    message: SaveMessageRequest,
) -> Result<(), AppError> {
    let msg_id = uuid::Uuid::new_v4().to_string();

    let tool_calls_json = message
        .tool_calls
        .as_ref()
        .map(|tc| serde_json::to_string(tc).unwrap_or_else(|_| "[]".to_string()));

    let db = state.db.lock_or_recover()?;

    // Get the current max sort_order for this conversation
    let sort_order: i32 = db
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM messages WHERE conversation_id = ?1",
            rusqlite::params![&session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    storage::save_message(
        &db,
        &msg_id,
        &session_id,
        &message.role,
        &message.content,
        tool_calls_json.as_deref(),
        message.tool_call_id.as_deref(),
        message.model.as_deref(),
        message.provider_id.as_deref(),
        None, // usage
        sort_order,
    )?;

    Ok(())
}

/// Update a conversation's title.
#[tauri::command]
pub async fn conversation_update_title(
    state: State<'_, AppState>,
    session_id: String,
    title: String,
) -> Result<(), AppError> {
    let db = state.db.lock_or_recover()?;
    storage::update_conversation_title(&db, &session_id, &title)
}
