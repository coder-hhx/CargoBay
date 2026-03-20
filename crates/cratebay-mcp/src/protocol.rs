//! JSON-RPC 2.0 protocol types for MCP communication.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// JSON-RPC 2.0 success response.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    pub result: serde_json::Value,
}

/// JSON-RPC 2.0 error response.
#[derive(Debug, Serialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    pub error: JsonRpcError,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Standard JSON-RPC error codes.
pub const PARSE_ERROR: i64 = -32700;
#[allow(dead_code)]
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

impl JsonRpcResponse {
    pub fn new(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result,
        }
    }
}

impl JsonRpcErrorResponse {
    pub fn new(id: Option<serde_json::Value>, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            error: JsonRpcError {
                code,
                message,
                data: None,
            },
        }
    }

    #[allow(dead_code)]
    pub fn with_data(
        id: Option<serde_json::Value>,
        code: i64,
        message: String,
        data: serde_json::Value,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            error: JsonRpcError {
                code,
                message,
                data: Some(data),
            },
        }
    }
}

/// MCP initialize result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

/// Server capabilities advertised during initialization.
#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
}

/// Tools capability.
#[derive(Debug, Serialize)]
pub struct ToolsCapability {
    /// Whether the server supports tool list changes notifications.
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

/// Server identification.
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// MCP tool definition for tools/list response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    /// Extension: marks destructive tools for client-side confirmation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_required: Option<bool>,
}

/// MCP tools/list response.
#[derive(Debug, Serialize)]
pub struct ToolsListResult {
    pub tools: Vec<McpToolDefinition>,
}

/// MCP tools/call response content item.
#[derive(Debug, Serialize)]
pub struct ToolCallContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// MCP tools/call result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub content: Vec<ToolCallContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolCallResult {
    /// Create a successful text result.
    pub fn success(text: String) -> Self {
        Self {
            content: vec![ToolCallContent {
                content_type: "text".to_string(),
                text,
            }],
            is_error: None,
        }
    }

    /// Create an error result.
    pub fn error(message: String) -> Self {
        Self {
            content: vec![ToolCallContent {
                content_type: "text".to_string(),
                text: message,
            }],
            is_error: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // JSON-RPC Request Parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_initialize_request() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "clientInfo": { "name": "test-client" }
            }
        }"#;
        let req: JsonRpcRequest = serde_json::from_str(json).expect("parse initialize");
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(serde_json::json!(1)));
        assert_eq!(req.params["protocolVersion"], "2024-11-05");
    }

    #[test]
    fn test_parse_tools_list_request() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }"#;
        let req: JsonRpcRequest = serde_json::from_str(json).expect("parse tools/list");
        assert_eq!(req.method, "tools/list");
        assert_eq!(req.id, Some(serde_json::json!(2)));
        // params defaults to Null when missing
        assert!(req.params.is_null());
    }

    #[test]
    fn test_parse_tools_call_request() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "cratebay_sandbox_list",
                "arguments": { "status": "running" }
            }
        }"#;
        let req: JsonRpcRequest = serde_json::from_str(json).expect("parse tools/call");
        assert_eq!(req.method, "tools/call");
        assert_eq!(req.params["name"], "cratebay_sandbox_list");
        assert_eq!(req.params["arguments"]["status"], "running");
    }

    #[test]
    fn test_parse_notification_no_id() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }"#;
        let req: JsonRpcRequest = serde_json::from_str(json).expect("parse notification");
        assert_eq!(req.method, "notifications/initialized");
        assert!(req.id.is_none());
    }

    #[test]
    fn test_parse_request_with_string_id() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": "req-001",
            "method": "ping"
        }"#;
        let req: JsonRpcRequest = serde_json::from_str(json).expect("parse string id");
        assert_eq!(req.id, Some(serde_json::json!("req-001")));
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = "not valid json{";
        let result = serde_json::from_str::<JsonRpcRequest>(json);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // JSON-RPC Response Construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_json_rpc_response_construction() {
        let response = JsonRpcResponse::new(
            Some(serde_json::json!(1)),
            serde_json::json!({"status": "ok"}),
        );
        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, Some(serde_json::json!(1)));
        assert_eq!(response.result["status"], "ok");
    }

    #[test]
    fn test_json_rpc_response_serialization() {
        let response = JsonRpcResponse::new(
            Some(serde_json::json!(1)),
            serde_json::json!({"data": "test"}),
        );
        let json = serde_json::to_string(&response).expect("serialize");
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"data\":\"test\""));
    }

    #[test]
    fn test_json_rpc_response_null_id_omitted() {
        let response = JsonRpcResponse::new(None, serde_json::json!({}));
        let json = serde_json::to_string(&response).expect("serialize");
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn test_json_rpc_error_response() {
        let error_resp = JsonRpcErrorResponse::new(
            Some(serde_json::json!(5)),
            METHOD_NOT_FOUND,
            "Method not found: foo".to_string(),
        );
        assert_eq!(error_resp.jsonrpc, "2.0");
        assert_eq!(error_resp.error.code, METHOD_NOT_FOUND);
        assert_eq!(error_resp.error.message, "Method not found: foo");
        assert!(error_resp.error.data.is_none());
    }

    #[test]
    fn test_json_rpc_error_response_serialization() {
        let error_resp = JsonRpcErrorResponse::new(
            Some(serde_json::json!(1)),
            PARSE_ERROR,
            "Parse error".to_string(),
        );
        let json = serde_json::to_string(&error_resp).expect("serialize");
        assert!(json.contains("-32700"));
        assert!(json.contains("Parse error"));
    }

    #[test]
    fn test_json_rpc_error_response_with_data() {
        let error_resp = JsonRpcErrorResponse::with_data(
            Some(serde_json::json!(1)),
            INVALID_PARAMS,
            "Invalid params".to_string(),
            serde_json::json!({"detail": "missing field"}),
        );
        assert_eq!(error_resp.error.code, INVALID_PARAMS);
        assert!(error_resp.error.data.is_some());
        assert_eq!(error_resp.error.data.unwrap()["detail"], "missing field");
    }

    // -----------------------------------------------------------------------
    // Error codes
    // -----------------------------------------------------------------------

    #[test]
    fn test_error_codes() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
    }

    // -----------------------------------------------------------------------
    // MCP Protocol Types
    // -----------------------------------------------------------------------

    #[test]
    fn test_initialize_result_serialization() {
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability { list_changed: false },
            },
            server_info: ServerInfo {
                name: "CrateBay".to_string(),
                version: "1.0.0".to_string(),
            },
        };
        let json = serde_json::to_value(&result).expect("serialize");
        assert_eq!(json["protocolVersion"], "2024-11-05");
        assert_eq!(json["capabilities"]["tools"]["listChanged"], false);
        assert_eq!(json["serverInfo"]["name"], "CrateBay");
    }

    #[test]
    fn test_tools_list_result_serialization() {
        let result = ToolsListResult {
            tools: vec![
                McpToolDefinition {
                    name: "test_tool".to_string(),
                    description: "A test tool".to_string(),
                    input_schema: serde_json::json!({"type": "object"}),
                    confirmation_required: None,
                },
                McpToolDefinition {
                    name: "destructive_tool".to_string(),
                    description: "Deletes things".to_string(),
                    input_schema: serde_json::json!({"type": "object"}),
                    confirmation_required: Some(true),
                },
            ],
        };
        let json = serde_json::to_value(&result).expect("serialize");
        let tools = json["tools"].as_array().expect("tools array");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "test_tool");
        // confirmation_required is None → should be omitted
        assert!(tools[0].get("confirmationRequired").is_none());
        // confirmation_required is Some(true) → should be present
        assert_eq!(tools[1]["confirmationRequired"], true);
    }

    #[test]
    fn test_tool_call_result_success() {
        let result = ToolCallResult::success("Hello world".to_string());
        assert!(result.is_error.is_none());
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].content_type, "text");
        assert_eq!(result.content[0].text, "Hello world");
    }

    #[test]
    fn test_tool_call_result_error() {
        let result = ToolCallResult::error("Something went wrong".to_string());
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text, "Something went wrong");
    }

    #[test]
    fn test_tool_call_result_success_serialization() {
        let result = ToolCallResult::success("ok".to_string());
        let json = serde_json::to_value(&result).expect("serialize");
        // isError should be omitted for success (it's None)
        assert!(json.get("isError").is_none());
        let content = json["content"].as_array().expect("content array");
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "ok");
    }

    #[test]
    fn test_tool_call_result_error_serialization() {
        let result = ToolCallResult::error("fail".to_string());
        let json = serde_json::to_value(&result).expect("serialize");
        assert_eq!(json["isError"], true);
    }

    // ── JSON-RPC injection defense tests (testing-spec.md §7.4) ──

    #[test]
    fn test_invalid_jsonrpc_version_handled() {
        // A request with wrong jsonrpc version should parse but is not "2.0"
        let json = r#"{
            "jsonrpc": "1.0",
            "id": 1,
            "method": "tools/list"
        }"#;
        let req: JsonRpcRequest = serde_json::from_str(json).expect("parse");
        // The parser should not panic; the server layer can reject non-2.0 versions.
        assert_eq!(req.jsonrpc, "1.0");
        assert_eq!(req.method, "tools/list");
    }

    #[test]
    fn test_oversized_payload_parsing() {
        // A very large JSON payload should not cause a panic
        let big_content = "x".repeat(10_000_000); // 10MB string
        let json = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"test","arguments":{{"content":"{}"}}}}}}"#,
            big_content
        );
        let result = serde_json::from_str::<JsonRpcRequest>(&json);
        // Should successfully parse (serde has no inherent size limit)
        assert!(result.is_ok());
        let req = result.unwrap();
        assert_eq!(req.method, "tools/call");
    }

    #[test]
    fn test_deeply_nested_json_handled() {
        // Deeply nested JSON objects: serde_json has a default recursion limit of 128
        let mut json = String::from(r#"{"jsonrpc":"2.0","id":1,"method":"test","params":"#);
        for _ in 0..200 {
            json.push_str(r#"{"a":"#);
        }
        json.push_str("1");
        for _ in 0..200 {
            json.push('}');
        }
        json.push('}');

        let result = serde_json::from_str::<JsonRpcRequest>(&json);
        // Should either parse or return an error — never panic
        // serde_json's recursion limit will likely cause an error here
        if let Err(e) = &result {
            // Expected: recursion limit exceeded
            assert!(
                e.to_string().contains("recursion") || e.to_string().contains("depth"),
                "Unexpected error: {}",
                e
            );
        }
    }

    #[test]
    fn test_null_method_rejected() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": null
        }"#;
        let result = serde_json::from_str::<JsonRpcRequest>(json);
        // method is String type, null should fail parsing
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_method_field_rejected() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1
        }"#;
        let result = serde_json::from_str::<JsonRpcRequest>(json);
        // method is required, missing it should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_extra_fields_ignored() {
        // Extra fields in JSON-RPC request should be safely ignored
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "evil_field": "malicious_value",
            "__proto__": {"polluted": true}
        }"#;
        let result = serde_json::from_str::<JsonRpcRequest>(json);
        assert!(result.is_ok());
        let req = result.unwrap();
        assert_eq!(req.method, "tools/list");
        // Extra fields are dropped by serde (deny_unknown_fields is not set,
        // which is correct for forward-compatibility)
    }

    #[test]
    fn test_unicode_method_name_handled() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "工具/列表"
        }"#;
        let result = serde_json::from_str::<JsonRpcRequest>(json);
        // Should parse without panicking
        assert!(result.is_ok());
        // The server will return method-not-found for unknown methods
    }

    #[test]
    fn test_empty_string_method_handled() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": ""
        }"#;
        let result = serde_json::from_str::<JsonRpcRequest>(json);
        assert!(result.is_ok());
        let req = result.unwrap();
        assert_eq!(req.method, "");
        // The server layer handles empty method names
    }
}
