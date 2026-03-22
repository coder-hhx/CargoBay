//! MCP Client implementation for CrateBay.
//!
//! Supports two transport mechanisms:
//! - **stdio**: JSON-RPC over stdin/stdout of a spawned child process
//! - **SSE**: Server-Sent Events over HTTP
//!
//! Provides `McpManager` for managing multiple MCP server connections,
//! and `McpServerConnection` for individual server lifecycle management.

mod config;
mod jsonrpc;
mod manager;
mod transport;

pub use config::{
    expand_env_vars, load_mcp_json, merge_server_configs, McpJsonConfig, McpServerConfigEntry,
    McpServerDbRow, McpTransportType, ResolvedMcpServer,
};
pub use manager::{McpManager, McpServerConnection};
