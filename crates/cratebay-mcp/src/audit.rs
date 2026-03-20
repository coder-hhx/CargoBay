//! Audit logging for MCP tool calls.
//!
//! Per mcp-spec.md §2.5, all tool calls are logged to
//! `~/.cratebay/logs/mcp-audit.jsonl` (one JSON object per line).

use chrono::Utc;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// A single audit log entry per mcp-spec.md §2.5.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub result: AuditResult,
    pub caller: String,
    pub duration_ms: u64,
}

/// Outcome of a tool call.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditResult {
    Success(String),
    Error(String),
}

/// Get the audit log file path: `~/.cratebay/logs/mcp-audit.jsonl`.
fn audit_log_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cratebay").join("logs").join("mcp-audit.jsonl"))
}

/// Write an audit entry to the JSONL log file.
///
/// Creates the directory structure if it doesn't exist.
/// Errors are logged but do not propagate — audit failure should
/// not break tool execution.
pub fn write_audit_entry(entry: &AuditEntry) {
    let Some(path) = audit_log_path() else {
        tracing::warn!("Cannot determine home directory for audit logging");
        return;
    };

    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            tracing::warn!("Failed to create audit log directory: {}", e);
            return;
        }
    }

    let line = match serde_json::to_string(entry) {
        Ok(json) => json,
        Err(e) => {
            tracing::warn!("Failed to serialize audit entry: {}", e);
            return;
        }
    };

    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(e) = writeln!(file, "{}", line) {
                tracing::warn!("Failed to write audit entry: {}", e);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to open audit log file: {}", e);
        }
    }
}

/// Create an audit entry for a tool call.
pub fn create_entry(
    tool_name: &str,
    parameters: &serde_json::Value,
    result: AuditResult,
    caller: &str,
    duration_ms: u64,
) -> AuditEntry {
    AuditEntry {
        timestamp: Utc::now().to_rfc3339(),
        tool_name: tool_name.to_string(),
        parameters: sanitize_parameters(parameters),
        result,
        caller: caller.to_string(),
        duration_ms,
    }
}

/// Sanitize parameters to remove potentially sensitive data.
///
/// Currently passes through as-is, but provides a hook for future
/// secret redaction.
fn sanitize_parameters(params: &serde_json::Value) -> serde_json::Value {
    // For file content (base64), truncate to avoid huge log entries
    if let Some(obj) = params.as_object() {
        let mut sanitized = serde_json::Map::new();
        for (key, value) in obj {
            if key == "content" {
                if let Some(s) = value.as_str() {
                    if s.len() > 100 {
                        sanitized.insert(
                            key.clone(),
                            serde_json::Value::String(format!("{}...[truncated, {} bytes]", &s[..100], s.len())),
                        );
                        continue;
                    }
                }
            }
            sanitized.insert(key.clone(), value.clone());
        }
        return serde_json::Value::Object(sanitized);
    }
    params.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_short_content() {
        let params = serde_json::json!({
            "sandbox_id": "abc123",
            "content": "short"
        });
        let sanitized = sanitize_parameters(&params);
        assert_eq!(sanitized["content"], "short");
    }

    #[test]
    fn test_sanitize_long_content() {
        let long_content = "x".repeat(200);
        let params = serde_json::json!({
            "sandbox_id": "abc123",
            "content": long_content
        });
        let sanitized = sanitize_parameters(&params);
        let text = sanitized["content"].as_str().expect("should be string");
        assert!(text.contains("[truncated"));
        assert!(text.contains("200 bytes"));
    }

    #[test]
    fn test_sanitize_exactly_100_chars() {
        let content = "x".repeat(100);
        let params = serde_json::json!({
            "content": content
        });
        let sanitized = sanitize_parameters(&params);
        // 100 chars is NOT > 100, so it should be preserved as-is
        assert_eq!(sanitized["content"].as_str().unwrap(), content);
    }

    #[test]
    fn test_sanitize_101_chars_truncated() {
        let content = "a".repeat(101);
        let params = serde_json::json!({
            "content": content
        });
        let sanitized = sanitize_parameters(&params);
        let text = sanitized["content"].as_str().unwrap();
        assert!(text.contains("[truncated"));
    }

    #[test]
    fn test_sanitize_preserves_non_content_fields() {
        let params = serde_json::json!({
            "sandbox_id": "abc123",
            "path": "/tmp/test"
        });
        let sanitized = sanitize_parameters(&params);
        assert_eq!(sanitized["sandbox_id"], "abc123");
        assert_eq!(sanitized["path"], "/tmp/test");
    }

    #[test]
    fn test_sanitize_non_object_passthrough() {
        let params = serde_json::json!("just a string");
        let sanitized = sanitize_parameters(&params);
        assert_eq!(sanitized, serde_json::json!("just a string"));
    }

    #[test]
    fn test_sanitize_null_passthrough() {
        let params = serde_json::Value::Null;
        let sanitized = sanitize_parameters(&params);
        assert_eq!(sanitized, serde_json::Value::Null);
    }

    #[test]
    fn test_create_entry() {
        let entry = create_entry(
            "cratebay_sandbox_list",
            &serde_json::json!({}),
            AuditResult::Success("Listed 3 sandboxes".to_string()),
            "test-client",
            42,
        );
        assert_eq!(entry.tool_name, "cratebay_sandbox_list");
        assert_eq!(entry.duration_ms, 42);
        assert_eq!(entry.caller, "test-client");
        assert!(!entry.timestamp.is_empty());
    }

    #[test]
    fn test_create_entry_error_result() {
        let entry = create_entry(
            "cratebay_sandbox_create",
            &serde_json::json!({"template_id": "node-dev"}),
            AuditResult::Error("Docker not available".to_string()),
            "test-client",
            100,
        );
        assert_eq!(entry.tool_name, "cratebay_sandbox_create");
        assert!(matches!(entry.result, AuditResult::Error(_)));
    }

    #[test]
    fn test_audit_entry_serializes_to_json() {
        let entry = create_entry(
            "cratebay_sandbox_list",
            &serde_json::json!({}),
            AuditResult::Success("ok".to_string()),
            "test-client",
            10,
        );
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("cratebay_sandbox_list"));
        assert!(json.contains("test-client"));
        assert!(json.contains("10"));
    }

    #[test]
    fn test_audit_entry_is_single_line_jsonl() {
        let entry = create_entry(
            "cratebay_sandbox_exec",
            &serde_json::json!({"sandbox_id": "abc", "command": "ls -la"}),
            AuditResult::Success("file1\nfile2\nfile3".to_string()),
            "claude",
            250,
        );
        // Serialize to a single JSON line (no pretty-printing)
        let json = serde_json::to_string(&entry).expect("should serialize");
        // JSONL format: no newlines within the JSON object
        assert!(!json.contains('\n'), "JSONL entry should be a single line");
    }

    #[test]
    fn test_audit_result_serialization() {
        let success = AuditResult::Success("done".to_string());
        let json = serde_json::to_value(&success).expect("serialize success");
        assert!(json.get("success").is_some());
        assert_eq!(json["success"], "done");

        let error = AuditResult::Error("failed".to_string());
        let json = serde_json::to_value(&error).expect("serialize error");
        assert!(json.get("error").is_some());
        assert_eq!(json["error"], "failed");
    }

    #[test]
    fn test_audit_entry_timestamp_is_rfc3339() {
        let entry = create_entry(
            "test_tool",
            &serde_json::json!({}),
            AuditResult::Success("ok".to_string()),
            "test",
            0,
        );
        // Verify the timestamp can be parsed back as RFC 3339
        let parsed = chrono::DateTime::parse_from_rfc3339(&entry.timestamp);
        assert!(parsed.is_ok(), "Timestamp should be valid RFC 3339: {}", entry.timestamp);
    }
}
