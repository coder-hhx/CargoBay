//! LLM proxy and provider management Tauri commands.

use std::time::Instant;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::events::llm_stream_event;
use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::models::{
    AuditAction, ChatMessage, LlmModelInfo, LlmOptions, LlmProvider,
    LlmProviderCreateRequest, LlmProviderUpdateRequest, ProviderTestResult,
};
use cratebay_core::{audit, llm_proxy, storage, validation};
use cratebay_core::MutexExt;

/// List configured LLM providers.
#[tauri::command]
pub async fn llm_provider_list(
    state: State<'_, AppState>,
) -> Result<Vec<LlmProvider>, AppError> {
    let db = state.db.lock_or_recover()?;
    storage::list_providers(&db)
}

/// Create a new LLM provider.
#[tauri::command]
pub async fn llm_provider_create(
    state: State<'_, AppState>,
    request: LlmProviderCreateRequest,
) -> Result<LlmProvider, AppError> {
    validation::validate_name(&request.name, 100)?;
    validation::validate_url(&request.api_base)?;

    let id = uuid::Uuid::new_v4().to_string();
    let db = state.db.lock_or_recover()?;

    // Create provider record
    storage::create_provider(&db, &id, &request.name, &request.api_base, &request.api_format)?;

    // Store encrypted API key
    // For now, store the key as plaintext bytes (real encryption to be added with keyring)
    let hint = storage::compute_key_hint(&request.api_key);
    storage::save_api_key(
        &db,
        &id,
        request.api_key.as_bytes(),
        &[0u8; 12], // placeholder nonce
        &hint,
    )?;

    audit::log_action(&db, &AuditAction::ProviderCreate, &id, Some(&request.name), "user")?;

    storage::get_provider(&db, &id)
}

/// Update an existing LLM provider.
#[tauri::command]
pub async fn llm_provider_update(
    state: State<'_, AppState>,
    id: String,
    request: LlmProviderUpdateRequest,
) -> Result<LlmProvider, AppError> {
    if let Some(ref name) = request.name {
        validation::validate_name(name, 100)?;
    }
    if let Some(ref url) = request.api_base {
        validation::validate_url(url)?;
    }

    let db = state.db.lock_or_recover()?;

    storage::update_provider(
        &db,
        &id,
        request.name.as_deref(),
        request.api_base.as_deref(),
        request.api_format.as_ref(),
        request.enabled,
    )?;

    // Update API key if provided
    if let Some(ref api_key) = request.api_key {
        let hint = storage::compute_key_hint(api_key);
        storage::save_api_key(&db, &id, api_key.as_bytes(), &[0u8; 12], &hint)?;
    }

    audit::log_action(&db, &AuditAction::ProviderUpdate, &id, None, "user")?;

    storage::get_provider(&db, &id)
}

/// Delete an LLM provider and associated data.
#[tauri::command]
pub async fn llm_provider_delete(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    let db = state.db.lock_or_recover()?;
    storage::delete_provider(&db, &id)?;
    audit::log_action(&db, &AuditAction::ProviderDelete, &id, None, "user")?;
    Ok(())
}

/// List models for a specific provider (from local DB).
#[tauri::command]
pub async fn llm_models_list(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<LlmModelInfo>, AppError> {
    let db = state.db.lock_or_recover()?;
    storage::list_models(&db, &provider_id)
}

/// Toggle a model's enabled state.
#[tauri::command]
pub async fn llm_models_toggle(
    state: State<'_, AppState>,
    provider_id: String,
    model_id: String,
    enabled: bool,
) -> Result<(), AppError> {
    let db = state.db.lock_or_recover()?;
    storage::toggle_model(&db, &provider_id, &model_id, enabled)?;
    audit::log_action(
        &db,
        &AuditAction::ModelToggle,
        &format!("{}:{}", provider_id, model_id),
        Some(if enabled { "enabled" } else { "disabled" }),
        "user",
    )?;
    Ok(())
}

/// Stream a chat completion request through the Rust backend.
///
/// API keys never leave the backend. Tokens are emitted as Tauri Events
/// on the channel `llm:stream:{channel_id}`.
#[tauri::command]
pub async fn llm_proxy_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    channel_id: String,
    provider_id: String,
    model_id: String,
    messages: Vec<ChatMessage>,
    options: Option<LlmOptions>,
) -> Result<(), AppError> {
    // Load provider and API key
    let (provider, api_key) = {
        let db = state.db.lock_or_recover()?;
        let provider = storage::get_provider(&db, &provider_id)?;
        let (encrypted_key, _nonce) = storage::load_api_key(&db, &provider_id)?;
        // For now, API key is stored as plaintext bytes (real decryption with keyring later)
        let api_key = String::from_utf8(encrypted_key)
            .map_err(|_| AppError::LlmProxy("Invalid API key encoding".to_string()))?;
        (provider, api_key)
    };

    // Register cancellation token for this session
    let cancel_token = CancellationToken::new();
    {
        let mut tokens = state.llm_cancel_tokens.lock_or_recover()?;
        tokens.insert(channel_id.clone(), cancel_token.clone());
    }

    let event_name = llm_stream_event(&channel_id);
    let (tx, mut rx) = mpsc::channel(256);

    // Spawn a task to forward events from the channel to Tauri Events
    let app_handle = app.clone();
    let event_name_clone = event_name.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if app_handle.emit(&event_name_clone, &event).is_err() {
                break;
            }
        }
    });

    // Build the reqwest client
    let client = reqwest::Client::new();

    // Stream the chat completion with cancellation support
    let channel_id_cleanup = channel_id.clone();
    let cancel_tokens_ref = state.llm_cancel_tokens.clone();

    let result = tokio::select! {
        res = llm_proxy::stream_chat(
            &client,
            &provider,
            &api_key,
            &model_id,
            messages,
            options,
            tx,
        ) => res.map(|_usage| ()),
        _ = cancel_token.cancelled() => {
            Err(AppError::LlmProxy("Stream cancelled by user".to_string()))
        }
    };

    // Clean up cancellation token
    {
        if let Ok(mut tokens) = cancel_tokens_ref.lock() {
            tokens.remove(&channel_id_cleanup);
        }
    }

    result
}

/// Cancel an active LLM streaming session.
#[tauri::command]
pub async fn llm_proxy_cancel(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<(), AppError> {
    let token = {
        let tokens = state.llm_cancel_tokens.lock_or_recover()?;
        tokens.get(&channel_id).cloned()
    };

    match token {
        Some(cancel_token) => {
            cancel_token.cancel();
            Ok(())
        }
        None => {
            // Session may have already completed; not an error
            Ok(())
        }
    }
}

/// Fetch available models from a provider's remote API and store them locally.
#[tauri::command]
pub async fn llm_models_fetch(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<LlmModelInfo>, AppError> {
    // Load provider and API key
    let (provider, api_key) = {
        let db = state.db.lock_or_recover()?;
        let provider = storage::get_provider(&db, &provider_id)?;
        let (encrypted_key, _nonce) = storage::load_api_key(&db, &provider_id)?;
        let api_key = String::from_utf8(encrypted_key)
            .map_err(|_| AppError::LlmProxy("Invalid API key encoding".to_string()))?;
        (provider, api_key)
    };

    // Fetch models from remote
    let client = reqwest::Client::new();
    let remote_models = llm_proxy::fetch_models(&client, &provider, &api_key).await?;

    // Convert to storage format: (id, name, supports_reasoning)
    let models_for_storage: Vec<(String, String, bool)> = remote_models
        .iter()
        .map(|m| (m.id.clone(), m.name.clone(), false))
        .collect();

    // Save to database
    {
        let db = state.db.lock_or_recover()?;
        storage::save_models(&db, &provider_id, &models_for_storage)?;
    }

    // Return the stored models (includes is_enabled state)
    let db = state.db.lock_or_recover()?;
    storage::list_models(&db, &provider_id)
}

/// Test connectivity to an LLM provider.
#[tauri::command]
pub async fn llm_provider_test(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<ProviderTestResult, AppError> {
    // Load provider and API key
    let (provider, api_key) = {
        let db = state.db.lock_or_recover()?;
        let provider = storage::get_provider(&db, &provider_id)?;
        let (encrypted_key, _nonce) = storage::load_api_key(&db, &provider_id)?;
        let api_key = String::from_utf8(encrypted_key)
            .map_err(|_| AppError::LlmProxy("Invalid API key encoding".to_string()))?;
        (provider, api_key)
    };

    let client = reqwest::Client::new();
    let start = Instant::now();

    // Try fetching models as a connectivity test
    match llm_proxy::fetch_models(&client, &provider, &api_key).await {
        Ok(models) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            let model_name = models
                .first()
                .map(|m| m.id.clone())
                .unwrap_or_else(|| "none".to_string());
            Ok(ProviderTestResult {
                success: true,
                latency_ms,
                model: model_name,
                error: None,
            })
        }
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(ProviderTestResult {
                success: false,
                latency_ms,
                model: String::new(),
                error: Some(e.to_string()),
            })
        }
    }
}
