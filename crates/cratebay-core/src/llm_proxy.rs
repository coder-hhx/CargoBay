//! LLM request proxy with streaming support.
//!
//! Supports three API formats:
//! - **Anthropic Messages API** (`/v1/messages`)
//! - **OpenAI Responses API** (`/v1/responses`) — with reasoning effort
//! - **OpenAI Chat Completions** (`/v1/chat/completions`)
//!
//! All outgoing requests use dual-header authentication (both
//! `Authorization: Bearer` and `x-api-key`).

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::error::AppError;
use crate::models::{
    ApiFormat, ChatMessage, LlmOptions, LlmProvider, LlmStreamEvent, ModelInfo, ToolDefinition,
    UsageStats,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Stream a chat completion through the LLM provider.
///
/// 1. Build format-specific request body
/// 2. Send with dual-header authentication
/// 3. Parse SSE stream, forwarding events through `tx`
///
/// Returns the final [`UsageStats`] once the stream completes.
pub async fn stream_chat(
    client: &Client,
    provider: &LlmProvider,
    api_key: &str,
    model_id: &str,
    messages: Vec<ChatMessage>,
    options: Option<LlmOptions>,
    tx: mpsc::Sender<LlmStreamEvent>,
) -> Result<UsageStats, AppError> {
    info!(provider = %provider.name, model = %model_id, "Starting LLM stream");

    // 1. Build authentication headers
    let headers = build_auth_headers(api_key, &provider.api_format);

    // 2. Build format-specific request
    let (url, body) = match provider.api_format {
        ApiFormat::Anthropic => build_anthropic_request(provider, model_id, &messages, &options)?,
        ApiFormat::OpenAiResponses => {
            build_openai_responses_request(provider, model_id, &messages, &options)?
        }
        ApiFormat::OpenAiCompletions => {
            build_openai_completions_request(provider, model_id, &messages, &options)?
        }
    };

    // 3. Send request
    let response = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::LlmProxy(format!("Request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".into());
        return Err(AppError::LlmProxy(format!(
            "Provider returned HTTP {}: {}",
            status, error_body
        )));
    }

    // 4. Parse SSE stream
    let usage = parse_sse_stream(response, &provider.api_format, &tx).await?;

    // 5. Send Done event
    let done_event = LlmStreamEvent::Done {
        usage: usage.clone(),
    };
    tx.send(done_event).await.ok();

    info!(
        prompt_tokens = usage.prompt_tokens,
        completion_tokens = usage.completion_tokens,
        "LLM stream completed"
    );
    Ok(usage)
}

/// Fetch available models from a provider's `/v1/models` endpoint.
///
/// Uses dual-header authentication. The response follows the standard
/// OpenAI `/v1/models` response format (`{ data: [{ id, ... }, ...] }`).
pub async fn fetch_models(
    client: &Client,
    provider: &LlmProvider,
    api_key: &str,
) -> Result<Vec<ModelInfo>, AppError> {
    let url = format!("{}/v1/models", provider.api_base.trim_end_matches('/'));
    let headers = build_auth_headers(api_key, &provider.api_format);

    let response = client
        .get(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| AppError::LlmProxy(format!("Failed to fetch models: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".into());
        return Err(AppError::LlmProxy(format!(
            "Models endpoint returned HTTP {}: {}",
            status, error_body
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::LlmProxy(format!("Invalid models response: {}", e)))?;

    let models = body["data"]
        .as_array()
        .ok_or_else(|| AppError::LlmProxy("Invalid models response: missing 'data' array".into()))?
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            Some(ModelInfo {
                name: id.clone(),
                id,
            })
        })
        .collect();

    Ok(models)
}

// ---------------------------------------------------------------------------
// Dual-header authentication
// ---------------------------------------------------------------------------

/// Build authentication headers.
///
/// Both `Authorization: Bearer <key>` and `x-api-key: <key>` are **always**
/// sent, ensuring compatibility with OpenAI-compatible, Anthropic, and
/// third-party providers.
///
/// For Anthropic, the `anthropic-version` header is also included.
fn build_auth_headers(api_key: &str, api_format: &ApiFormat) -> HeaderMap {
    let mut headers = HeaderMap::new();

    // Dual authentication
    if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", api_key)) {
        headers.insert("Authorization", val);
    }
    if let Ok(val) = HeaderValue::from_str(api_key) {
        headers.insert("x-api-key", val);
    }

    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    // Anthropic requires a version header
    if *api_format == ApiFormat::Anthropic {
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    }

    headers
}

// ---------------------------------------------------------------------------
// Request builders — one per API format
// ---------------------------------------------------------------------------

/// Build an Anthropic Messages API request.
///
/// - `system` is a **top-level** parameter (not in the messages array).
/// - Message content uses **content blocks** (`[{type, text}]`).
/// - Tool definitions use `input_schema` instead of `parameters`.
fn build_anthropic_request(
    provider: &LlmProvider,
    model_id: &str,
    messages: &[ChatMessage],
    options: &Option<LlmOptions>,
) -> Result<(String, serde_json::Value), AppError> {
    let url = format!("{}/v1/messages", provider.api_base.trim_end_matches('/'));

    // Extract system message to top-level parameter
    let system_content = messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // Convert non-system messages to Anthropic format
    let anthropic_messages: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role != "system")
        .map(anthropic_message)
        .collect();

    let max_tokens = options.as_ref().and_then(|o| o.max_tokens).unwrap_or(4096);

    let mut body = serde_json::json!({
        "model": model_id,
        "max_tokens": max_tokens,
        "system": system_content,
        "messages": anthropic_messages,
        "stream": true,
    });

    // Add temperature if specified
    if let Some(ref opts) = options {
        if let Some(temp) = opts.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = opts.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
    }

    // Convert tools to Anthropic format (input_schema instead of parameters)
    if let Some(ref opts) = options {
        if let Some(ref tools) = opts.tools {
            let anthropic_tools: Vec<serde_json::Value> =
                tools.iter().map(anthropic_tool).collect();
            body["tools"] = serde_json::json!(anthropic_tools);
        }
    }

    Ok((url, body))
}

/// Build an OpenAI Responses API request.
///
/// - Uses `input` instead of `messages`.
/// - Supports `reasoning.effort` parameter.
fn build_openai_responses_request(
    provider: &LlmProvider,
    model_id: &str,
    messages: &[ChatMessage],
    options: &Option<LlmOptions>,
) -> Result<(String, serde_json::Value), AppError> {
    let url = format!("{}/v1/responses", provider.api_base.trim_end_matches('/'));

    let input: Vec<serde_json::Value> = messages.iter().map(openai_message).collect();

    let mut body = serde_json::json!({
        "model": model_id,
        "input": input,
        "stream": true,
    });

    if let Some(ref opts) = options {
        if let Some(temp) = opts.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = opts.max_tokens {
            body["max_output_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = opts.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }

        // Reasoning effort — ONLY this format supports it
        if let Some(ref effort) = opts.reasoning_effort {
            body["reasoning"] = serde_json::json!({ "effort": effort });
        }

        // Tools
        if let Some(ref tools) = opts.tools {
            let openai_tools: Vec<serde_json::Value> =
                tools.iter().map(openai_responses_tool).collect();
            body["tools"] = serde_json::json!(openai_tools);
        }
    }

    Ok((url, body))
}

/// Build an OpenAI Chat Completions request.
///
/// Standard `messages` array with `{role, content}` objects.
fn build_openai_completions_request(
    provider: &LlmProvider,
    model_id: &str,
    messages: &[ChatMessage],
    options: &Option<LlmOptions>,
) -> Result<(String, serde_json::Value), AppError> {
    let url = format!(
        "{}/v1/chat/completions",
        provider.api_base.trim_end_matches('/')
    );

    let openai_messages: Vec<serde_json::Value> = messages.iter().map(openai_message).collect();

    let mut body = serde_json::json!({
        "model": model_id,
        "messages": openai_messages,
        "stream": true,
    });

    if let Some(ref opts) = options {
        if let Some(temp) = opts.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = opts.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = opts.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }

        // Tools in OpenAI Completions format
        if let Some(ref tools) = opts.tools {
            let completions_tools: Vec<serde_json::Value> =
                tools.iter().map(openai_completions_tool).collect();
            body["tools"] = serde_json::json!(completions_tools);
        }
    }

    Ok((url, body))
}

// ---------------------------------------------------------------------------
// Message format helpers
// ---------------------------------------------------------------------------

/// Convert a `ChatMessage` to Anthropic message format.
///
/// Anthropic uses content blocks: `[{type: "text", text: "..."}]`.
fn anthropic_message(msg: &ChatMessage) -> serde_json::Value {
    // Handle tool result messages
    if msg.role == "tool" {
        return serde_json::json!({
            "role": "user",
            "content": [{
                "type": "tool_result",
                "tool_use_id": msg.tool_call_id.clone().unwrap_or_default(),
                "content": msg.content,
            }],
        });
    }

    let mut m = serde_json::json!({
        "role": msg.role,
        "content": [{
            "type": "text",
            "text": msg.content,
        }],
    });

    // Append tool_use blocks if the assistant made tool calls
    if let Some(ref tool_calls) = msg.tool_calls {
        if let Some(content) = m["content"].as_array_mut() {
            for tc in tool_calls {
                let input: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));
                content.push(serde_json::json!({
                    "type": "tool_use",
                    "id": tc.id,
                    "name": tc.name,
                    "input": input,
                }));
            }
        }
    }

    m
}

/// Convert a `ChatMessage` to standard OpenAI message format.
fn openai_message(msg: &ChatMessage) -> serde_json::Value {
    let mut m = serde_json::json!({
        "role": msg.role,
        "content": msg.content,
    });

    // Tool calls in assistant messages
    if let Some(ref tool_calls) = msg.tool_calls {
        let tc_json: Vec<serde_json::Value> = tool_calls
            .iter()
            .map(|tc| {
                serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments,
                    },
                })
            })
            .collect();
        m["tool_calls"] = serde_json::json!(tc_json);
    }

    // Tool result messages
    if let Some(ref tool_call_id) = msg.tool_call_id {
        m["tool_call_id"] = serde_json::json!(tool_call_id);
    }

    m
}

// ---------------------------------------------------------------------------
// Tool format helpers
// ---------------------------------------------------------------------------

/// Anthropic tool format: `{name, description, input_schema}`.
fn anthropic_tool(tool: &ToolDefinition) -> serde_json::Value {
    serde_json::json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.parameters,
    })
}

/// OpenAI Responses API tool format: `{type, name, description, parameters}`.
fn openai_responses_tool(tool: &ToolDefinition) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "parameters": tool.parameters,
    })
}

/// OpenAI Chat Completions tool format:
/// `{type: "function", function: {name, description, parameters}}`.
fn openai_completions_tool(tool: &ToolDefinition) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        },
    })
}

// ---------------------------------------------------------------------------
// SSE stream parsing
// ---------------------------------------------------------------------------

/// Parse the Server-Sent Events stream from the provider and emit
/// [`LlmStreamEvent`]s through the sender.
async fn parse_sse_stream(
    response: reqwest::Response,
    api_format: &ApiFormat,
    tx: &mpsc::Sender<LlmStreamEvent>,
) -> Result<UsageStats, AppError> {
    let mut usage = UsageStats::default();
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk =
            chunk_result.map_err(|e| AppError::LlmProxy(format!("Stream read error: {}", e)))?;

        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // SSE events are separated by double newlines
        while let Some(pos) = buffer.find("\n\n") {
            let event_text = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            for line in event_text.lines() {
                let line = line.trim();
                if !line.starts_with("data: ") {
                    continue;
                }
                let data = &line[6..];

                // [DONE] marker
                if data == "[DONE]" {
                    return Ok(usage);
                }

                let json: serde_json::Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Skipping unparseable SSE data: {}", e);
                        continue;
                    }
                };

                match api_format {
                    ApiFormat::Anthropic => {
                        parse_anthropic_event(&json, tx, &mut usage).await;
                    }
                    ApiFormat::OpenAiResponses => {
                        parse_openai_responses_event(&json, tx, &mut usage).await;
                    }
                    ApiFormat::OpenAiCompletions => {
                        parse_openai_completions_event(&json, tx, &mut usage).await;
                    }
                }
            }
        }
    }

    Ok(usage)
}

/// Parse an Anthropic SSE event.
///
/// Anthropic uses event types like `content_block_delta`, `message_delta`,
/// and `message_stop`.
async fn parse_anthropic_event(
    json: &serde_json::Value,
    tx: &mpsc::Sender<LlmStreamEvent>,
    usage: &mut UsageStats,
) {
    let event_type = json["type"].as_str().unwrap_or("");

    match event_type {
        "content_block_delta" => {
            let delta = &json["delta"];
            if let Some(text) = delta["text"].as_str() {
                let event = LlmStreamEvent::Token {
                    content: text.to_string(),
                };
                tx.send(event).await.ok();
            }
            // Tool use input delta
            if let Some(partial_json) = delta["partial_json"].as_str() {
                // Accumulate partial JSON for tool calls — for now emit as
                // content for the agent layer to handle.
                let event = LlmStreamEvent::Token {
                    content: partial_json.to_string(),
                };
                tx.send(event).await.ok();
            }
        }
        "content_block_start" => {
            let content_block = &json["content_block"];
            if content_block["type"].as_str() == Some("tool_use") {
                let event = LlmStreamEvent::ToolCall {
                    id: content_block["id"].as_str().unwrap_or("").to_string(),
                    name: content_block["name"].as_str().unwrap_or("").to_string(),
                    arguments: String::new(),
                };
                tx.send(event).await.ok();
            }
        }
        "message_delta" => {
            if let Some(u) = json["usage"].as_object() {
                usage.completion_tokens =
                    u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            }
        }
        "message_start" => {
            if let Some(u) = json["message"]["usage"].as_object() {
                usage.prompt_tokens =
                    u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            }
        }
        "message_stop" => {
            // Stream is done — handled by the outer loop
        }
        "error" => {
            let msg = json["error"]["message"]
                .as_str()
                .unwrap_or("Unknown Anthropic error");
            let event = LlmStreamEvent::Error {
                message: msg.to_string(),
            };
            tx.send(event).await.ok();
        }
        _ => {}
    }
}

/// Parse an OpenAI Responses API SSE event.
async fn parse_openai_responses_event(
    json: &serde_json::Value,
    tx: &mpsc::Sender<LlmStreamEvent>,
    usage: &mut UsageStats,
) {
    let event_type = json["type"].as_str().unwrap_or("");

    match event_type {
        "response.output_text.delta" => {
            if let Some(delta) = json["delta"].as_str() {
                let event = LlmStreamEvent::Token {
                    content: delta.to_string(),
                };
                tx.send(event).await.ok();
            }
        }
        "response.function_call_arguments.delta" => {
            if let Some(delta) = json["delta"].as_str() {
                let event = LlmStreamEvent::Token {
                    content: delta.to_string(),
                };
                tx.send(event).await.ok();
            }
        }
        "response.output_item.added" => {
            let item = &json["item"];
            if item["type"].as_str() == Some("function_call") {
                let event = LlmStreamEvent::ToolCall {
                    id: item["call_id"].as_str().unwrap_or("").to_string(),
                    name: item["name"].as_str().unwrap_or("").to_string(),
                    arguments: String::new(),
                };
                tx.send(event).await.ok();
            }
        }
        "response.completed" => {
            if let Some(u) = json["response"]["usage"].as_object() {
                usage.prompt_tokens =
                    u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                usage.completion_tokens =
                    u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
            }
        }
        "error" => {
            let msg = json["message"]
                .as_str()
                .unwrap_or("Unknown OpenAI Responses error");
            let event = LlmStreamEvent::Error {
                message: msg.to_string(),
            };
            tx.send(event).await.ok();
        }
        _ => {}
    }
}

/// Parse an OpenAI Chat Completions SSE event.
///
/// Events have the shape `{choices: [{delta: {content, tool_calls}}]}`.
async fn parse_openai_completions_event(
    json: &serde_json::Value,
    tx: &mpsc::Sender<LlmStreamEvent>,
    usage: &mut UsageStats,
) {
    // Usage may appear in the final chunk
    if let Some(u) = json["usage"].as_object() {
        usage.prompt_tokens = u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        usage.completion_tokens = u
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        usage.total_tokens = u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    }

    let choices = match json["choices"].as_array() {
        Some(c) => c,
        None => return,
    };

    for choice in choices {
        let delta = &choice["delta"];

        // Content token
        if let Some(content) = delta["content"].as_str() {
            if !content.is_empty() {
                let event = LlmStreamEvent::Token {
                    content: content.to_string(),
                };
                tx.send(event).await.ok();
            }
        }

        // Tool calls
        if let Some(tool_calls) = delta["tool_calls"].as_array() {
            for tc in tool_calls {
                // Initial tool call
                if let Some(function) = tc["function"].as_object() {
                    let id = tc["id"].as_str().unwrap_or("").to_string();
                    let name = function
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let arguments = function
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if !id.is_empty() && !name.is_empty() {
                        let event = LlmStreamEvent::ToolCall {
                            id,
                            name,
                            arguments,
                        };
                        tx.send(event).await.ok();
                    } else if !arguments.is_empty() {
                        // Streaming tool call arguments
                        let event = LlmStreamEvent::Token { content: arguments };
                        tx.send(event).await.ok();
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_auth_headers_includes_both() {
        let headers = build_auth_headers("sk-test-123", &ApiFormat::OpenAiCompletions);
        assert!(headers.contains_key("Authorization"));
        assert!(headers.contains_key("x-api-key"));
        assert!(headers.contains_key("Content-Type"));
        assert_eq!(
            headers.get("Authorization").unwrap().to_str().unwrap(),
            "Bearer sk-test-123"
        );
        assert_eq!(
            headers.get("x-api-key").unwrap().to_str().unwrap(),
            "sk-test-123"
        );
    }

    #[test]
    fn build_auth_headers_anthropic_includes_version() {
        let headers = build_auth_headers("sk-test", &ApiFormat::Anthropic);
        assert!(headers.contains_key("anthropic-version"));
    }

    #[test]
    fn anthropic_request_extracts_system() {
        let provider = LlmProvider {
            id: "test".into(),
            name: "Test".into(),
            api_base: "https://api.anthropic.com".into(),
            api_format: ApiFormat::Anthropic,
            enabled: true,
            has_api_key: true,
            notes: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "You are helpful.".into(),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "user".into(),
                content: "Hello".into(),
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let (url, body) =
            build_anthropic_request(&provider, "claude-3-5-sonnet", &messages, &None).unwrap();

        assert_eq!(url, "https://api.anthropic.com/v1/messages");
        assert_eq!(body["system"], "You are helpful.");
        assert_eq!(body["model"], "claude-3-5-sonnet");
        assert_eq!(body["stream"], true);

        // Messages should not contain the system message
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
    }

    #[test]
    fn openai_responses_request_includes_reasoning() {
        let provider = LlmProvider {
            id: "test".into(),
            name: "Test".into(),
            api_base: "https://api.openai.com".into(),
            api_format: ApiFormat::OpenAiResponses,
            enabled: true,
            has_api_key: true,
            notes: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "Think hard about this.".into(),
            tool_calls: None,
            tool_call_id: None,
        }];

        let options = Some(LlmOptions {
            reasoning_effort: Some("high".into()),
            ..Default::default()
        });

        let (url, body) =
            build_openai_responses_request(&provider, "o3", &messages, &options).unwrap();

        assert_eq!(url, "https://api.openai.com/v1/responses");
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["model"], "o3");
        // Uses "input" not "messages"
        assert!(body.get("input").is_some());
        assert!(body.get("messages").is_none());
    }

    #[test]
    fn openai_completions_request_standard_format() {
        let provider = LlmProvider {
            id: "test".into(),
            name: "Test".into(),
            api_base: "https://api.openai.com".into(),
            api_format: ApiFormat::OpenAiCompletions,
            enabled: true,
            has_api_key: true,
            notes: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "You are helpful.".into(),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "user".into(),
                content: "Hello".into(),
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let (url, body) =
            build_openai_completions_request(&provider, "gpt-4o", &messages, &None).unwrap();

        assert_eq!(url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["stream"], true);
        // Uses "messages" not "input"
        assert!(body.get("messages").is_some());
        assert!(body.get("input").is_none());
        // System is inside messages (not top-level)
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
    }

    #[test]
    fn anthropic_tool_format() {
        let tool = ToolDefinition {
            name: "container_list".into(),
            description: "List containers".into(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let result = anthropic_tool(&tool);
        assert!(result.get("input_schema").is_some());
        assert!(result.get("parameters").is_none());
    }

    #[test]
    fn openai_completions_tool_format() {
        let tool = ToolDefinition {
            name: "container_list".into(),
            description: "List containers".into(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let result = openai_completions_tool(&tool);
        assert_eq!(result["type"], "function");
        assert!(result.get("function").is_some());
        assert_eq!(result["function"]["name"], "container_list");
    }
}
