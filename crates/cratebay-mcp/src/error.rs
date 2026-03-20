//! MCP-specific error types for the cratebay-mcp server.

use thiserror::Error;

/// Errors specific to MCP server operations.
#[derive(Error, Debug)]
pub enum McpError {
    #[error("Path traversal detected: {requested} is outside workspace root {root}")]
    PathTraversal { requested: String, root: String },

    #[error("Workspace root not configured: set CRATEBAY_MCP_WORKSPACE_ROOT")]
    #[allow(dead_code)]
    WorkspaceRootNotSet,

    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("Sandbox not found: {0}")]
    SandboxNotFound(String),

    #[error("Sandbox not running: {0}")]
    SandboxNotRunning(String),

    #[error("Docker error: {0}")]
    Docker(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<bollard::errors::Error> for McpError {
    fn from(e: bollard::errors::Error) -> Self {
        McpError::Docker(e.to_string())
    }
}

impl From<cratebay_core::AppError> for McpError {
    fn from(e: cratebay_core::AppError) -> Self {
        match e {
            cratebay_core::AppError::Docker(de) => McpError::Docker(de.to_string()),
            cratebay_core::AppError::Io(ie) => McpError::Io(ie),
            cratebay_core::AppError::Serialization(se) => McpError::Serialization(se),
            cratebay_core::AppError::PermissionDenied(msg) => {
                McpError::PathTraversal {
                    requested: msg.clone(),
                    root: String::new(),
                }
            }
            other => McpError::Internal(other.to_string()),
        }
    }
}
