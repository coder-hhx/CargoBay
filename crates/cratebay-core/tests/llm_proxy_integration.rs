//! Supplementary tests for the LLM proxy module.
//!
//! These tests complement the 7 inline unit tests in `llm_proxy.rs`
//! by covering additional scenarios: ApiFormat serde roundtrip,
//! message conversion edge cases, and request builder edge cases.

use cratebay_core::models::{
    ApiFormat, ChatMessage, LlmOptions, LlmProvider, LlmStreamEvent, ToolCallInfo, ToolDefinition,
    UsageStats,
};

// ─── ApiFormat Serde & Conversion ───────────────────────────────────

#[test]
fn api_format_as_str_roundtrip() {
    let formats = [
        ApiFormat::Anthropic,
        ApiFormat::OpenAiResponses,
        ApiFormat::OpenAiCompletions,
    ];
    for format in &formats {
        let s = format.as_str();
        let recovered = ApiFormat::from_str(s);
        assert_eq!(
            recovered.as_ref(),
            Some(format),
            "Roundtrip failed for {:?} -> '{}' -> {:?}",
            format,
            s,
            recovered
        );
    }
}

#[test]
fn api_format_from_str_unknown_returns_none() {
    assert_eq!(ApiFormat::from_str("unknown"), None);
    assert_eq!(ApiFormat::from_str(""), None);
    assert_eq!(ApiFormat::from_str("OpenAI"), None); // Case-sensitive
}

#[test]
fn api_format_serde_json_roundtrip() {
    let formats = [
        ApiFormat::Anthropic,
        ApiFormat::OpenAiResponses,
        ApiFormat::OpenAiCompletions,
    ];
    for format in &formats {
        let json = serde_json::to_string(format).unwrap();
        let recovered: ApiFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(
            &recovered, format,
            "JSON serde roundtrip failed for {:?}",
            format
        );
    }
}

#[test]
fn api_format_serde_json_values() {
    // Verify the exact JSON representation (snake_case rename)
    assert_eq!(
        serde_json::to_string(&ApiFormat::Anthropic).unwrap(),
        "\"anthropic\""
    );
    assert_eq!(
        serde_json::to_string(&ApiFormat::OpenAiResponses).unwrap(),
        "\"open_ai_responses\""
    );
    assert_eq!(
        serde_json::to_string(&ApiFormat::OpenAiCompletions).unwrap(),
        "\"open_ai_completions\""
    );
}

// ─── LlmStreamEvent Serde ──────────────────────────────────────────

#[test]
fn llm_stream_event_token_serde() {
    let event = LlmStreamEvent::Token {
        content: "Hello".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"Token\""));
    assert!(json.contains("\"content\":\"Hello\""));

    let recovered: LlmStreamEvent = serde_json::from_str(&json).unwrap();
    match recovered {
        LlmStreamEvent::Token { content } => assert_eq!(content, "Hello"),
        _ => panic!("Expected Token"),
    }
}

#[test]
fn llm_stream_event_tool_call_serde() {
    let event = LlmStreamEvent::ToolCall {
        id: "tc-1".into(),
        name: "container_list".into(),
        arguments: "{}".into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let recovered: LlmStreamEvent = serde_json::from_str(&json).unwrap();
    match recovered {
        LlmStreamEvent::ToolCall {
            id,
            name,
            arguments,
        } => {
            assert_eq!(id, "tc-1");
            assert_eq!(name, "container_list");
            assert_eq!(arguments, "{}");
        }
        _ => panic!("Expected ToolCall"),
    }
}

#[test]
fn llm_stream_event_done_serde() {
    let event = LlmStreamEvent::Done {
        usage: UsageStats {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let recovered: LlmStreamEvent = serde_json::from_str(&json).unwrap();
    match recovered {
        LlmStreamEvent::Done { usage } => {
            assert_eq!(usage.prompt_tokens, 100);
            assert_eq!(usage.completion_tokens, 50);
            assert_eq!(usage.total_tokens, 150);
        }
        _ => panic!("Expected Done"),
    }
}

#[test]
fn llm_stream_event_error_serde() {
    let event = LlmStreamEvent::Error {
        message: "Rate limit exceeded".into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let recovered: LlmStreamEvent = serde_json::from_str(&json).unwrap();
    match recovered {
        LlmStreamEvent::Error { message } => {
            assert_eq!(message, "Rate limit exceeded");
        }
        _ => panic!("Expected Error"),
    }
}

// ─── ChatMessage Serde ──────────────────────────────────────────────

#[test]
fn chat_message_without_tool_calls_skips_fields() {
    let msg = ChatMessage {
        role: "user".into(),
        content: "Hello".into(),
        tool_calls: None,
        tool_call_id: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    // skip_serializing_if should omit null fields
    assert!(!json.contains("tool_calls"));
    assert!(!json.contains("tool_call_id"));
}

#[test]
fn chat_message_with_tool_calls_roundtrip() {
    let msg = ChatMessage {
        role: "assistant".into(),
        content: "Let me check.".into(),
        tool_calls: Some(vec![ToolCallInfo {
            id: "tc-1".into(),
            name: "container_list".into(),
            arguments: r#"{"status":"all"}"#.into(),
        }]),
        tool_call_id: None,
    };

    let json = serde_json::to_string(&msg).unwrap();
    let recovered: ChatMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(recovered.role, "assistant");
    let tcs = recovered.tool_calls.unwrap();
    assert_eq!(tcs.len(), 1);
    assert_eq!(tcs[0].name, "container_list");
    assert_eq!(tcs[0].arguments, r#"{"status":"all"}"#);
}

#[test]
fn chat_message_tool_result_roundtrip() {
    let msg = ChatMessage {
        role: "tool".into(),
        content: "[]".into(),
        tool_calls: None,
        tool_call_id: Some("tc-1".into()),
    };

    let json = serde_json::to_string(&msg).unwrap();
    let recovered: ChatMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(recovered.role, "tool");
    assert_eq!(recovered.tool_call_id, Some("tc-1".to_string()));
}

// ─── ToolDefinition Serde ───────────────────────────────────────────

#[test]
fn tool_definition_serde_roundtrip() {
    let tool = ToolDefinition {
        name: "container_create".into(),
        description: "Create a new container".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "image": {"type": "string"},
            },
            "required": ["name", "image"],
        }),
    };

    let json = serde_json::to_string(&tool).unwrap();
    let recovered: ToolDefinition = serde_json::from_str(&json).unwrap();

    assert_eq!(recovered.name, "container_create");
    assert_eq!(recovered.parameters["type"], "object");
}

// ─── LlmOptions Default ────────────────────────────────────────────

#[test]
fn llm_options_default_all_none() {
    let opts = LlmOptions::default();
    assert!(opts.model.is_none());
    assert!(opts.temperature.is_none());
    assert!(opts.max_tokens.is_none());
    assert!(opts.top_p.is_none());
    assert!(opts.tools.is_none());
    assert!(opts.reasoning_effort.is_none());
}

// ─── LlmProvider Model Serde ───────────────────────────────────────

#[test]
fn llm_provider_serde_roundtrip() {
    let provider = LlmProvider {
        id: "test-id".into(),
        name: "Test Provider".into(),
        api_base: "https://api.test.com".into(),
        api_format: ApiFormat::Anthropic,
        enabled: true,
        has_api_key: false,
        notes: "Test notes".into(),
        created_at: "2026-03-20T00:00:00Z".into(),
        updated_at: "2026-03-20T00:00:00Z".into(),
    };

    let json = serde_json::to_string(&provider).unwrap();
    let recovered: LlmProvider = serde_json::from_str(&json).unwrap();

    assert_eq!(recovered.id, "test-id");
    assert_eq!(recovered.api_format, ApiFormat::Anthropic);
    assert_eq!(recovered.enabled, true);
    assert_eq!(recovered.has_api_key, false);
}

// ─── UsageStats Default ────────────────────────────────────────────

#[test]
fn usage_stats_default_zeros() {
    let stats = UsageStats::default();
    assert_eq!(stats.prompt_tokens, 0);
    assert_eq!(stats.completion_tokens, 0);
    assert_eq!(stats.total_tokens, 0);
}
