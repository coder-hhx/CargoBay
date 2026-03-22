//! .mcp.json configuration parsing and server discovery.

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// .mcp.json schema types
// ---------------------------------------------------------------------------

/// Top-level structure of `.mcp.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpJsonConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, McpServerConfigEntry>,
}

/// A single MCP server entry in `.mcp.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfigEntry {
    /// Executable path or command name (required for stdio).
    #[serde(default)]
    pub command: Option<String>,

    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables (supports `${VAR_NAME}` expansion).
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Transport mechanism: "stdio" (default) or "sse".
    #[serde(default = "default_transport")]
    pub transport: McpTransportType,

    /// Server URL for SSE transport.
    #[serde(default)]
    pub url: Option<String>,

    /// HTTP headers for SSE transport (supports `${VAR_NAME}` expansion).
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Enable/disable without removing config.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Human-readable description.
    #[serde(default)]
    pub notes: String,
}

/// Transport type for MCP servers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpTransportType {
    Stdio,
    Sse,
}

impl Default for McpTransportType {
    fn default() -> Self {
        McpTransportType::Stdio
    }
}

fn default_transport() -> McpTransportType {
    McpTransportType::Stdio
}

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Database row representation (matches mcp_servers table)
// ---------------------------------------------------------------------------

/// MCP server configuration as stored in the SQLite `mcp_servers` table.
#[derive(Debug, Clone)]
pub struct McpServerDbRow {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
    pub working_dir: String,
    pub enabled: bool,
    pub notes: String,
    pub auto_start: bool,
}

// ---------------------------------------------------------------------------
// Merged configuration used by McpManager
// ---------------------------------------------------------------------------

/// Fully resolved MCP server configuration ready for connection.
#[derive(Debug, Clone)]
pub struct ResolvedMcpServer {
    pub id: String,
    pub name: String,
    pub transport: McpTransportType,
    /// For stdio: the executable command.
    pub command: Option<String>,
    /// For stdio: command arguments.
    pub args: Vec<String>,
    /// Fully expanded environment variables as KEY=VALUE pairs.
    pub env: Vec<String>,
    /// Working directory for the spawned process.
    pub working_dir: Option<String>,
    /// For SSE: the server URL.
    pub url: Option<String>,
    /// For SSE: HTTP headers.
    pub headers: HashMap<String, String>,
    pub enabled: bool,
    pub notes: String,
    pub auto_start: bool,
}

// ---------------------------------------------------------------------------
// Configuration loading
// ---------------------------------------------------------------------------

/// Load and parse `.mcp.json` from a given directory.
///
/// Returns `Ok(None)` if the file does not exist.
/// Returns `Err` if the file exists but cannot be parsed.
pub fn load_mcp_json(project_root: &Path) -> Result<Option<McpJsonConfig>, AppError> {
    let config_path = project_root.join(".mcp.json");
    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to read {}: {}", config_path.display(), e),
        ))
    })?;

    let config: McpJsonConfig = serde_json::from_str(&content)
        .map_err(|e| AppError::Mcp(format!("Failed to parse {}: {}", config_path.display(), e)))?;

    Ok(Some(config))
}

/// Expand `${VAR_NAME}` patterns in a string using the process environment.
///
/// Unresolved variables are left as-is (no error, just kept verbatim).
pub fn expand_env_vars(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            // Consume '{'
            chars.next();
            let mut var_name = String::new();
            let mut found_closing = false;

            for inner_ch in chars.by_ref() {
                if inner_ch == '}' {
                    found_closing = true;
                    break;
                }
                var_name.push(inner_ch);
            }

            if found_closing && !var_name.is_empty() {
                match std::env::var(&var_name) {
                    Ok(value) => result.push_str(&value),
                    Err(_) => {
                        // Leave unresolved variables as-is
                        result.push_str("${");
                        result.push_str(&var_name);
                        result.push('}');
                    }
                }
            } else {
                // Malformed: write back what we consumed
                result.push_str("${");
                result.push_str(&var_name);
                if !found_closing {
                    // No closing brace found — just stop
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Merge configurations from `.mcp.json` and SQLite database rows.
///
/// SQLite entries override `.mcp.json` entries with the same server name.
pub fn merge_server_configs(
    mcp_json: Option<&McpJsonConfig>,
    db_rows: &[McpServerDbRow],
) -> Vec<ResolvedMcpServer> {
    let mut servers: HashMap<String, ResolvedMcpServer> = HashMap::new();

    // 1. Load from .mcp.json first
    if let Some(config) = mcp_json {
        for (name, entry) in &config.mcp_servers {
            // Expand environment variables in env values
            let expanded_env: Vec<String> = entry
                .env
                .iter()
                .map(|(k, v)| format!("{}={}", k, expand_env_vars(v)))
                .collect();

            // Expand environment variables in headers
            let expanded_headers: HashMap<String, String> = entry
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), expand_env_vars(v)))
                .collect();

            servers.insert(
                name.clone(),
                ResolvedMcpServer {
                    id: name.clone(),
                    name: name.clone(),
                    transport: entry.transport.clone(),
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                    env: expanded_env,
                    working_dir: None,
                    url: entry.url.clone(),
                    headers: expanded_headers,
                    enabled: entry.enabled,
                    notes: entry.notes.clone(),
                    auto_start: false,
                },
            );
        }
    }

    // 2. Merge SQLite rows (override same-name entries)
    for row in db_rows {
        // Database rows are always stdio transport (SSE servers are only from .mcp.json)
        let env: Vec<String> = row
            .env
            .iter()
            .map(|e| {
                // Each entry is "KEY=VALUE"; expand the VALUE part
                if let Some(eq_pos) = e.find('=') {
                    let key = &e[..eq_pos];
                    let val = &e[eq_pos + 1..];
                    format!("{}={}", key, expand_env_vars(val))
                } else {
                    e.clone()
                }
            })
            .collect();

        servers.insert(
            row.name.clone(),
            ResolvedMcpServer {
                id: row.id.clone(),
                name: row.name.clone(),
                transport: McpTransportType::Stdio,
                command: Some(row.command.clone()),
                args: row.args.clone(),
                env,
                working_dir: if row.working_dir.is_empty() {
                    None
                } else {
                    Some(row.working_dir.clone())
                },
                url: None,
                headers: HashMap::new(),
                enabled: row.enabled,
                notes: row.notes.clone(),
                auto_start: row.auto_start,
            },
        );
    }

    servers.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Environment variable expansion
    // -----------------------------------------------------------------------

    #[test]
    fn expand_env_vars_basic() {
        std::env::set_var("CRATEBAY_TEST_VAR", "hello");
        assert_eq!(expand_env_vars("${CRATEBAY_TEST_VAR}"), "hello");
        assert_eq!(
            expand_env_vars("prefix_${CRATEBAY_TEST_VAR}_suffix"),
            "prefix_hello_suffix"
        );
        std::env::remove_var("CRATEBAY_TEST_VAR");
    }

    #[test]
    fn expand_env_vars_missing_var() {
        let input = "${NONEXISTENT_CRATEBAY_VAR_12345}";
        assert_eq!(expand_env_vars(input), input);
    }

    #[test]
    fn expand_env_vars_no_expansion() {
        assert_eq!(expand_env_vars("plain text"), "plain text");
        assert_eq!(expand_env_vars("$NOT_A_VAR"), "$NOT_A_VAR");
    }

    #[test]
    fn expand_env_vars_multiple_vars() {
        std::env::set_var("CRATEBAY_VAR_A", "foo");
        std::env::set_var("CRATEBAY_VAR_B", "bar");
        assert_eq!(
            expand_env_vars("${CRATEBAY_VAR_A}:${CRATEBAY_VAR_B}"),
            "foo:bar"
        );
        std::env::remove_var("CRATEBAY_VAR_A");
        std::env::remove_var("CRATEBAY_VAR_B");
    }

    #[test]
    fn expand_env_vars_empty_input() {
        assert_eq!(expand_env_vars(""), "");
    }

    #[test]
    fn expand_env_vars_only_dollar_sign() {
        assert_eq!(expand_env_vars("$"), "$");
    }

    #[test]
    fn expand_env_vars_unclosed_brace() {
        // Malformed: ${VAR without closing brace
        let result = expand_env_vars("${UNCLOSED");
        // Should output the consumed characters as-is
        assert!(result.starts_with("${"));
    }

    #[test]
    fn expand_env_vars_empty_var_name() {
        // ${}  — empty variable name, closing brace found but var_name is empty
        // The implementation writes back "${" (consumed chars minus the closing brace logic)
        let result = expand_env_vars("${}");
        // Empty var name is not expanded; result is the consumed prefix
        assert_eq!(result, "${");
    }

    #[test]
    fn expand_env_vars_adjacent_to_text() {
        std::env::set_var("CRATEBAY_ADJACENT", "world");
        assert_eq!(expand_env_vars("hello${CRATEBAY_ADJACENT}!"), "helloworld!");
        std::env::remove_var("CRATEBAY_ADJACENT");
    }

    // -----------------------------------------------------------------------
    // .mcp.json parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_mcp_json_config() {
        let json = r#"{
            "mcpServers": {
                "shadcn": {
                    "command": "npx",
                    "args": ["shadcn@latest", "mcp"],
                    "transport": "stdio"
                },
                "remote": {
                    "url": "https://example.com/sse",
                    "transport": "sse",
                    "headers": {
                        "Authorization": "Bearer ${TOKEN}"
                    }
                }
            }
        }"#;

        let config: McpJsonConfig = serde_json::from_str(json).expect("parse failed");
        assert_eq!(config.mcp_servers.len(), 2);

        let shadcn = &config.mcp_servers["shadcn"];
        assert_eq!(shadcn.command.as_deref(), Some("npx"));
        assert_eq!(shadcn.transport, McpTransportType::Stdio);

        let remote = &config.mcp_servers["remote"];
        assert_eq!(remote.url.as_deref(), Some("https://example.com/sse"));
        assert_eq!(remote.transport, McpTransportType::Sse);
    }

    #[test]
    fn parse_mcp_json_empty() {
        let json = r#"{"mcpServers": {}}"#;
        let config: McpJsonConfig = serde_json::from_str(json).expect("parse empty");
        assert_eq!(config.mcp_servers.len(), 0);
    }

    #[test]
    fn parse_mcp_json_defaults() {
        // Minimal config with just a command
        let json = r#"{
            "mcpServers": {
                "test": {
                    "command": "test-server"
                }
            }
        }"#;
        let config: McpJsonConfig = serde_json::from_str(json).expect("parse defaults");
        let server = &config.mcp_servers["test"];
        assert_eq!(server.transport, McpTransportType::Stdio); // default
        assert!(server.enabled); // default true
        assert!(server.args.is_empty());
        assert!(server.env.is_empty());
        assert!(server.headers.is_empty());
        assert!(server.url.is_none());
    }

    #[test]
    fn parse_mcp_json_with_env() {
        let json = r#"{
            "mcpServers": {
                "server": {
                    "command": "my-server",
                    "env": {
                        "API_KEY": "${MY_API_KEY}",
                        "PORT": "3000"
                    }
                }
            }
        }"#;
        let config: McpJsonConfig = serde_json::from_str(json).expect("parse with env");
        let server = &config.mcp_servers["server"];
        assert_eq!(server.env.len(), 2);
        assert_eq!(server.env["PORT"], "3000");
        assert_eq!(server.env["API_KEY"], "${MY_API_KEY}");
    }

    #[test]
    fn parse_mcp_json_disabled_server() {
        let json = r#"{
            "mcpServers": {
                "disabled": {
                    "command": "server",
                    "enabled": false,
                    "notes": "temporarily disabled"
                }
            }
        }"#;
        let config: McpJsonConfig = serde_json::from_str(json).expect("parse disabled");
        let server = &config.mcp_servers["disabled"];
        assert!(!server.enabled);
        assert_eq!(server.notes, "temporarily disabled");
    }

    #[test]
    fn parse_mcp_json_missing_mcp_servers_key() {
        // mcpServers key missing — default should be empty
        let json = r#"{}"#;
        let config: McpJsonConfig = serde_json::from_str(json).expect("parse no key");
        assert_eq!(config.mcp_servers.len(), 0);
    }

    #[test]
    fn load_mcp_json_nonexistent_file() {
        let result = load_mcp_json(Path::new("/nonexistent/path/to/project"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn load_mcp_json_from_temp_dir() {
        let dir = std::env::temp_dir().join("cratebay_test_mcp_json");
        std::fs::create_dir_all(&dir).ok();
        let config_file = dir.join(".mcp.json");
        std::fs::write(
            &config_file,
            r#"{"mcpServers":{"test":{"command":"echo"}}}"#,
        )
        .expect("write test file");

        let result = load_mcp_json(&dir);
        std::fs::remove_file(&config_file).ok();
        std::fs::remove_dir(&dir).ok();

        assert!(result.is_ok());
        let config = result.unwrap().expect("should have config");
        assert_eq!(config.mcp_servers.len(), 1);
        assert!(config.mcp_servers.contains_key("test"));
    }

    #[test]
    fn load_mcp_json_invalid_json() {
        let dir = std::env::temp_dir().join("cratebay_test_mcp_invalid");
        std::fs::create_dir_all(&dir).ok();
        let config_file = dir.join(".mcp.json");
        std::fs::write(&config_file, "not valid json").expect("write test file");

        let result = load_mcp_json(&dir);
        std::fs::remove_file(&config_file).ok();
        std::fs::remove_dir(&dir).ok();

        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Transport type
    // -----------------------------------------------------------------------

    #[test]
    fn transport_type_default_is_stdio() {
        assert_eq!(McpTransportType::default(), McpTransportType::Stdio);
    }

    #[test]
    fn transport_type_serde() {
        let stdio: McpTransportType = serde_json::from_str("\"stdio\"").unwrap();
        assert_eq!(stdio, McpTransportType::Stdio);
        let sse: McpTransportType = serde_json::from_str("\"sse\"").unwrap();
        assert_eq!(sse, McpTransportType::Sse);
    }

    // -----------------------------------------------------------------------
    // Server config merging
    // -----------------------------------------------------------------------

    #[test]
    fn merge_sqlite_overrides_mcp_json() {
        let mcp_json = McpJsonConfig {
            mcp_servers: {
                let mut map = HashMap::new();
                map.insert(
                    "server1".to_string(),
                    McpServerConfigEntry {
                        command: Some("original-cmd".to_string()),
                        args: vec![],
                        env: HashMap::new(),
                        transport: McpTransportType::Stdio,
                        url: None,
                        headers: HashMap::new(),
                        enabled: true,
                        notes: "from json".to_string(),
                    },
                );
                map
            },
        };

        let db_rows = vec![McpServerDbRow {
            id: "server1-db".to_string(),
            name: "server1".to_string(),
            command: "overridden-cmd".to_string(),
            args: vec!["--flag".to_string()],
            env: vec![],
            working_dir: String::new(),
            enabled: false,
            notes: "from db".to_string(),
            auto_start: true,
        }];

        let merged = merge_server_configs(Some(&mcp_json), &db_rows);
        assert_eq!(merged.len(), 1);

        let server = &merged[0];
        assert_eq!(server.command.as_deref(), Some("overridden-cmd"));
        assert_eq!(server.notes, "from db");
        assert!(!server.enabled);
        assert!(server.auto_start);
    }

    #[test]
    fn merge_no_overlap_keeps_both() {
        let mcp_json = McpJsonConfig {
            mcp_servers: {
                let mut map = HashMap::new();
                map.insert(
                    "json-server".to_string(),
                    McpServerConfigEntry {
                        command: Some("json-cmd".to_string()),
                        args: vec![],
                        env: HashMap::new(),
                        transport: McpTransportType::Stdio,
                        url: None,
                        headers: HashMap::new(),
                        enabled: true,
                        notes: String::new(),
                    },
                );
                map
            },
        };

        let db_rows = vec![McpServerDbRow {
            id: "db-server-id".to_string(),
            name: "db-server".to_string(),
            command: "db-cmd".to_string(),
            args: vec![],
            env: vec![],
            working_dir: String::new(),
            enabled: true,
            notes: String::new(),
            auto_start: false,
        }];

        let merged = merge_server_configs(Some(&mcp_json), &db_rows);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_empty_sources() {
        let merged = merge_server_configs(None, &[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_only_mcp_json() {
        let mcp_json = McpJsonConfig {
            mcp_servers: {
                let mut map = HashMap::new();
                map.insert(
                    "only-json".to_string(),
                    McpServerConfigEntry {
                        command: Some("json-cmd".to_string()),
                        args: vec!["arg1".to_string()],
                        env: HashMap::new(),
                        transport: McpTransportType::Stdio,
                        url: None,
                        headers: HashMap::new(),
                        enabled: true,
                        notes: String::new(),
                    },
                );
                map
            },
        };
        let merged = merge_server_configs(Some(&mcp_json), &[]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "only-json");
    }

    #[test]
    fn merge_only_db_rows() {
        let db_rows = vec![McpServerDbRow {
            id: "db-only".to_string(),
            name: "db-only".to_string(),
            command: "db-cmd".to_string(),
            args: vec![],
            env: vec!["KEY=value".to_string()],
            working_dir: "/tmp".to_string(),
            enabled: true,
            notes: String::new(),
            auto_start: false,
        }];
        let merged = merge_server_configs(None, &db_rows);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command.as_deref(), Some("db-cmd"));
        assert_eq!(merged[0].working_dir.as_deref(), Some("/tmp"));
    }

    #[test]
    fn merge_db_row_empty_working_dir_becomes_none() {
        let db_rows = vec![McpServerDbRow {
            id: "test".to_string(),
            name: "test".to_string(),
            command: "cmd".to_string(),
            args: vec![],
            env: vec![],
            working_dir: String::new(),
            enabled: true,
            notes: String::new(),
            auto_start: false,
        }];
        let merged = merge_server_configs(None, &db_rows);
        assert!(merged[0].working_dir.is_none());
    }

    #[test]
    fn merge_env_vars_expanded_in_json() {
        std::env::set_var("CRATEBAY_MERGE_TEST_KEY", "secret_value");
        let mcp_json = McpJsonConfig {
            mcp_servers: {
                let mut map = HashMap::new();
                let mut env = HashMap::new();
                env.insert(
                    "API_KEY".to_string(),
                    "${CRATEBAY_MERGE_TEST_KEY}".to_string(),
                );
                map.insert(
                    "test".to_string(),
                    McpServerConfigEntry {
                        command: Some("cmd".to_string()),
                        args: vec![],
                        env,
                        transport: McpTransportType::Stdio,
                        url: None,
                        headers: HashMap::new(),
                        enabled: true,
                        notes: String::new(),
                    },
                );
                map
            },
        };
        let merged = merge_server_configs(Some(&mcp_json), &[]);
        std::env::remove_var("CRATEBAY_MERGE_TEST_KEY");

        let server = &merged[0];
        assert!(server.env.contains(&"API_KEY=secret_value".to_string()));
    }

    #[test]
    fn merge_env_vars_expanded_in_db() {
        std::env::set_var("CRATEBAY_DB_TEST_KEY", "db_secret");
        let db_rows = vec![McpServerDbRow {
            id: "test".to_string(),
            name: "test".to_string(),
            command: "cmd".to_string(),
            args: vec![],
            env: vec!["API_KEY=${CRATEBAY_DB_TEST_KEY}".to_string()],
            working_dir: String::new(),
            enabled: true,
            notes: String::new(),
            auto_start: false,
        }];
        let merged = merge_server_configs(None, &db_rows);
        std::env::remove_var("CRATEBAY_DB_TEST_KEY");

        assert!(merged[0].env.contains(&"API_KEY=db_secret".to_string()));
    }

    #[test]
    fn merge_headers_expanded_in_json() {
        std::env::set_var("CRATEBAY_HDR_TOKEN", "my-token");
        let mcp_json = McpJsonConfig {
            mcp_servers: {
                let mut map = HashMap::new();
                let mut headers = HashMap::new();
                headers.insert(
                    "Authorization".to_string(),
                    "Bearer ${CRATEBAY_HDR_TOKEN}".to_string(),
                );
                map.insert(
                    "sse-server".to_string(),
                    McpServerConfigEntry {
                        command: None,
                        args: vec![],
                        env: HashMap::new(),
                        transport: McpTransportType::Sse,
                        url: Some("https://example.com/sse".to_string()),
                        headers,
                        enabled: true,
                        notes: String::new(),
                    },
                );
                map
            },
        };
        let merged = merge_server_configs(Some(&mcp_json), &[]);
        std::env::remove_var("CRATEBAY_HDR_TOKEN");

        let server = &merged[0];
        assert_eq!(server.headers["Authorization"], "Bearer my-token");
    }

    #[test]
    fn merge_db_rows_are_always_stdio() {
        let db_rows = vec![McpServerDbRow {
            id: "test".to_string(),
            name: "test".to_string(),
            command: "cmd".to_string(),
            args: vec![],
            env: vec![],
            working_dir: String::new(),
            enabled: true,
            notes: String::new(),
            auto_start: false,
        }];
        let merged = merge_server_configs(None, &db_rows);
        assert_eq!(merged[0].transport, McpTransportType::Stdio);
    }
}
