//! JSON-RPC 2.0 protocol types for MCP communication.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Global request ID counter.
static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique request ID.
pub fn next_request_id() -> u64 {
    REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request with an auto-generated ID.
    pub fn new(method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id: next_request_id(),
            method: method.to_string(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 notification (no `id` field, no response expected).
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    pub fn new(method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_request_id_increments() {
        let id1 = next_request_id();
        let id2 = next_request_id();
        assert!(id2 > id1, "IDs should increment");
    }

    #[test]
    fn test_request_new_has_correct_fields() {
        let req = JsonRpcRequest::new("initialize", Some(serde_json::json!({"key": "val"})));
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "initialize");
        assert!(req.params.is_some());
        assert_eq!(req.params.as_ref().unwrap()["key"], "val");
    }

    #[test]
    fn test_request_new_without_params() {
        let req = JsonRpcRequest::new("ping", None);
        assert_eq!(req.method, "ping");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_request_serialization() {
        let req = JsonRpcRequest::new("tools/list", Some(serde_json::json!({})));
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"tools/list\""));
        assert!(json.contains("\"id\":"));
    }

    #[test]
    fn test_request_serialization_omits_null_params() {
        let req = JsonRpcRequest::new("ping", None);
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(!json.contains("\"params\""));
    }

    #[test]
    fn test_notification_new() {
        let notif = JsonRpcNotification::new(
            "notifications/initialized",
            Some(serde_json::json!({})),
        );
        assert_eq!(notif.jsonrpc, "2.0");
        assert_eq!(notif.method, "notifications/initialized");
    }

    #[test]
    fn test_notification_serialization_no_id() {
        let notif = JsonRpcNotification::new("notifications/initialized", None);
        let json = serde_json::to_string(&notif).expect("serialize");
        // Notifications must NOT have an "id" field
        assert!(!json.contains("\"id\""));
        assert!(json.contains("\"method\":\"notifications/initialized\""));
    }

    #[test]
    fn test_response_deserialization_success() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"tools": []}
        }"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_response_deserialization_error() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        }"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_none());
        let error = resp.error.expect("should have error");
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
    }

    #[test]
    fn test_response_deserialization_with_error_data() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 5,
            "error": {
                "code": -32602,
                "message": "Invalid params",
                "data": {"field": "name", "reason": "required"}
            }
        }"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).expect("deserialize");
        let error = resp.error.expect("should have error");
        assert_eq!(error.code, -32602);
        let data = error.data.expect("should have data");
        assert_eq!(data["field"], "name");
    }

    #[test]
    fn test_response_deserialization_no_id() {
        let json = r#"{
            "jsonrpc": "2.0",
            "result": {}
        }"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).expect("deserialize");
        assert!(resp.id.is_none());
    }

    #[test]
    fn test_json_rpc_error_display() {
        let error = JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        };
        let display = format!("{}", error);
        assert_eq!(display, "JSON-RPC error -32601: Method not found");
    }

    #[test]
    fn test_json_rpc_error_serialization() {
        let error = JsonRpcError {
            code: -32700,
            message: "Parse error".to_string(),
            data: Some(serde_json::json!({"detail": "unexpected token"})),
        };
        let json = serde_json::to_value(&error).expect("serialize");
        assert_eq!(json["code"], -32700);
        assert_eq!(json["message"], "Parse error");
        assert_eq!(json["data"]["detail"], "unexpected token");
    }

    #[test]
    fn test_request_ids_are_unique() {
        let req1 = JsonRpcRequest::new("a", None);
        let req2 = JsonRpcRequest::new("b", None);
        let req3 = JsonRpcRequest::new("c", None);
        assert_ne!(req1.id, req2.id);
        assert_ne!(req2.id, req3.id);
    }
}
