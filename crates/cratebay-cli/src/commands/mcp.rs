//! MCP configuration export.

use std::path::PathBuf;

/// Export MCP client configuration for the specified target.
pub fn export_config(target: &str) -> anyhow::Result<()> {
    let mcp_binary = find_mcp_binary();

    let config = match target {
        "claude" => generate_claude_config(&mcp_binary),
        "cursor" => generate_cursor_config(&mcp_binary),
        "generic" => generate_generic_config(&mcp_binary),
        _ => generate_generic_config(&mcp_binary),
    };

    println!("{}", config);

    eprintln!();
    match target {
        "claude" => {
            let config_path = claude_config_path();
            eprintln!("Copy the JSON above into your Claude Desktop config:");
            eprintln!("  {}", config_path.display());
            eprintln!();
            eprintln!("Then restart Claude Desktop.");
        }
        "cursor" => {
            eprintln!("Add the JSON above to your Cursor MCP settings.");
        }
        _ => {
            eprintln!("Add the JSON above to your MCP client config.");
        }
    }

    Ok(())
}

fn find_mcp_binary() -> String {
    // Try to find cratebay-mcp in PATH or next to the current binary
    if let Ok(current_exe) = std::env::current_exe() {
        let dir = current_exe.parent().unwrap_or(std::path::Path::new("."));
        let mcp_path = dir.join("cratebay-mcp");
        if mcp_path.exists() {
            return mcp_path.to_string_lossy().to_string();
        }
    }

    // Fallback: assume it's in PATH
    "cratebay-mcp".to_string()
}

fn generate_claude_config(mcp_binary: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "cratebay": {
                "command": mcp_binary,
                "args": []
            }
        }
    }))
    .unwrap()
}

fn generate_cursor_config(mcp_binary: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "cratebay": {
                "command": mcp_binary,
                "args": []
            }
        }
    }))
    .unwrap()
}

fn generate_generic_config(mcp_binary: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "cratebay": {
                "command": mcp_binary,
                "args": [],
                "transport": "stdio"
            }
        }
    }))
    .unwrap()
}

fn claude_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    #[cfg(target_os = "macos")]
    {
        PathBuf::from(home).join("Library/Application Support/Claude/claude_desktop_config.json")
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(home))
            .join("Claude/claude_desktop_config.json")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        PathBuf::from(home).join(".config/Claude/claude_desktop_config.json")
    }
}
