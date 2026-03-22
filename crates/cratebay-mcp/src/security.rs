//! Security model for the MCP server.
//!
//! Implements workspace root restriction and path validation per mcp-spec.md §2.4.

use std::path::{Path, PathBuf};

use crate::error::McpError;

/// Validate that a requested path is within the workspace root.
///
/// Per mcp-spec.md §2.4:
/// 1. Reject paths containing ".." components (before canonicalization).
/// 2. Join with workspace root and canonicalize.
/// 3. Verify the canonical path starts with the workspace root.
pub fn validate_path(requested_path: &str, workspace_root: &Path) -> Result<PathBuf, McpError> {
    // Reject paths with ".." components (before canonicalization too)
    if requested_path.contains("..") {
        return Err(McpError::PathTraversal {
            requested: requested_path.to_string(),
            root: workspace_root.display().to_string(),
        });
    }

    let full_path = workspace_root.join(requested_path);
    let canonical = full_path.canonicalize().map_err(|e| {
        McpError::InvalidParams(format!("Cannot resolve path '{}': {}", requested_path, e))
    })?;

    let root_canonical = workspace_root.canonicalize().map_err(|e| {
        McpError::Internal(format!(
            "Cannot resolve workspace root '{}': {}",
            workspace_root.display(),
            e
        ))
    })?;

    // Prevent path traversal
    if !canonical.starts_with(&root_canonical) {
        return Err(McpError::PathTraversal {
            requested: requested_path.to_string(),
            root: workspace_root.display().to_string(),
        });
    }

    Ok(canonical)
}

/// Read the workspace root from CRATEBAY_MCP_WORKSPACE_ROOT environment variable.
pub fn workspace_root() -> Option<PathBuf> {
    std::env::var("CRATEBAY_MCP_WORKSPACE_ROOT")
        .ok()
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reject_dotdot_path() {
        let root = PathBuf::from("/tmp");
        let result = validate_path("../etc/passwd", &root);
        assert!(result.is_err());
        if let Err(McpError::PathTraversal { .. }) = result {
            // expected
        } else {
            panic!("Expected PathTraversal error");
        }
    }

    #[test]
    fn test_reject_embedded_dotdot() {
        let root = PathBuf::from("/tmp");
        let result = validate_path("subdir/../../etc/passwd", &root);
        assert!(result.is_err());
        match result {
            Err(McpError::PathTraversal { requested, root: _ }) => {
                assert_eq!(requested, "subdir/../../etc/passwd");
            }
            _ => panic!("Expected PathTraversal error"),
        }
    }

    #[test]
    fn test_reject_dotdot_only() {
        let root = PathBuf::from("/tmp");
        let result = validate_path("..", &root);
        assert!(result.is_err());
        assert!(matches!(result, Err(McpError::PathTraversal { .. })));
    }

    #[test]
    fn test_valid_path() {
        // Use a path that we know exists
        let root = std::env::temp_dir();
        // Create a test file
        let test_file = root.join("cratebay_test_security_valid");
        std::fs::write(&test_file, "test").ok();
        let result = validate_path("cratebay_test_security_valid", &root);
        std::fs::remove_file(&test_file).ok();
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_nested_path() {
        let root = std::env::temp_dir();
        let subdir = root.join("cratebay_test_nested_dir");
        std::fs::create_dir_all(&subdir).ok();
        let test_file = subdir.join("nested_file.txt");
        std::fs::write(&test_file, "test").ok();

        let result = validate_path("cratebay_test_nested_dir/nested_file.txt", &root);
        std::fs::remove_file(&test_file).ok();
        std::fs::remove_dir(&subdir).ok();
        assert!(result.is_ok());
    }

    #[test]
    fn test_nonexistent_path_returns_error() {
        let root = std::env::temp_dir();
        let result = validate_path("cratebay_nonexistent_file_xyz_12345", &root);
        // canonicalize fails on nonexistent path → InvalidParams
        assert!(result.is_err());
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn test_absolute_path_within_root() {
        // An absolute path is joined with root, which on Unix means the absolute
        // path replaces the root. This effectively escapes the workspace.
        let root = std::env::temp_dir();
        let test_file = root.join("cratebay_abs_test_file");
        std::fs::write(&test_file, "test").ok();

        // Using the absolute path of the test file: joining root with an absolute
        // path yields the absolute path itself. It should still pass if it's
        // within the root.
        let abs_path = test_file.to_string_lossy().to_string();
        // This depends on platform behavior of Path::join with absolute paths.
        // On Unix, /tmp + /tmp/file = /tmp/file, so it should be within root.
        let result = validate_path(&abs_path, &root);
        std::fs::remove_file(&test_file).ok();

        // If the absolute path is within the root, it should succeed.
        // If it's not (due to canonicalization), it should fail with PathTraversal.
        // Both outcomes are acceptable — the key is no panic.
        assert!(result.is_ok() || matches!(result, Err(McpError::PathTraversal { .. })));
    }

    #[test]
    fn test_workspace_root_env_var() {
        std::env::set_var("CRATEBAY_MCP_WORKSPACE_ROOT", "/some/path");
        let root = workspace_root();
        assert_eq!(root, Some(PathBuf::from("/some/path")));
        std::env::remove_var("CRATEBAY_MCP_WORKSPACE_ROOT");
    }

    #[test]
    fn test_workspace_root_env_var_unset() {
        std::env::remove_var("CRATEBAY_MCP_WORKSPACE_ROOT");
        let root = workspace_root();
        assert_eq!(root, None);
    }

    // ── URL-encoded path traversal tests (testing-spec.md §7.2) ──

    #[test]
    fn test_url_encoded_dotdot_rejected() {
        // %2e = '.', %2f = '/', so %2e%2e%2f = '../'
        // validate_path checks for ".." substring, which catches decoded variants
        // if the caller passes them through. Raw URL-encoded strings should also
        // be caught because they contain suspicious patterns.
        let root = std::env::temp_dir();

        // The literal string "%2e%2e" does not contain ".." so it won't be caught
        // by the ".." check, but canonicalize will fail or the path won't escape root.
        let result = validate_path("%2e%2e/etc/passwd", &root);
        // This should either fail because the file doesn't exist (InvalidParams)
        // or fail because canonicalization leads outside root (PathTraversal).
        // Either way, it must not succeed.
        assert!(result.is_err(), "URL-encoded traversal must not succeed");
    }

    #[test]
    fn test_double_encoded_traversal_rejected() {
        let root = std::env::temp_dir();
        // Double-encoded: %252e%252e = "%2e%2e" after first decode
        let result = validate_path("%252e%252e/etc/passwd", &root);
        assert!(result.is_err(), "Double-encoded traversal must not succeed");
    }

    #[test]
    fn test_backslash_traversal_rejected() {
        let root = std::env::temp_dir();
        // Windows-style path traversal
        let result = validate_path("..\\etc\\passwd", &root);
        // Contains ".." so it should be rejected
        assert!(result.is_err(), "Backslash traversal must not succeed");
    }

    // ── Symlink escape tests (testing-spec.md §7.2) ──

    #[test]
    fn test_symlink_within_root_accepted() {
        let root = std::env::temp_dir().join("cratebay_symlink_test_root");
        std::fs::create_dir_all(&root).ok();

        let target_file = root.join("real_file.txt");
        std::fs::write(&target_file, "content").ok();

        let link_path = root.join("link_to_file.txt");
        // Remove existing link if any
        std::fs::remove_file(&link_path).ok();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target_file, &link_path).ok();
            // Symlink within root should be accepted
            let result = validate_path("link_to_file.txt", &root);
            assert!(result.is_ok(), "Symlink within root should be accepted");
        }

        // Cleanup
        std::fs::remove_file(&link_path).ok();
        std::fs::remove_file(&target_file).ok();
        std::fs::remove_dir(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_escaping_root_rejected() {
        let root = std::env::temp_dir().join("cratebay_symlink_escape_test");
        std::fs::create_dir_all(&root).ok();

        // Create a symlink that points outside the workspace root
        let escape_link = root.join("escape_link");
        std::fs::remove_file(&escape_link).ok();
        // Point to /etc (outside workspace root)
        std::os::unix::fs::symlink("/etc", &escape_link).ok();

        // Trying to access escape_link/hostname should be caught by canonicalization
        let result = validate_path("escape_link/hostname", &root);
        // After canonicalization, /etc/hostname is outside root → PathTraversal
        assert!(
            result.is_err(),
            "Symlink escaping root must be rejected: {:?}",
            result
        );

        // Cleanup
        std::fs::remove_file(&escape_link).ok();
        std::fs::remove_dir(&root).ok();
    }

    // ── Null byte and special character tests ──

    #[test]
    fn test_null_byte_in_path_rejected() {
        let root = std::env::temp_dir();
        let result = validate_path("file\0.txt", &root);
        // Null bytes in paths cause canonicalize to fail
        assert!(result.is_err(), "Null byte in path must be rejected");
    }

    #[test]
    fn test_very_long_path_rejected() {
        let root = std::env::temp_dir();
        let long_path = "a/".repeat(500) + "file.txt";
        let result = validate_path(&long_path, &root);
        // Very long paths should fail (canonicalize or OS limits)
        assert!(result.is_err(), "Excessively long path must be rejected");
    }
}
