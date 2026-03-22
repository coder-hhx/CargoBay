//! Sandbox template definitions.
//!
//! Templates provide pre-configured development environments per mcp-spec.md §2.3.

use serde::Serialize;

/// A sandbox template definition.
#[derive(Debug, Clone, Serialize)]
pub struct SandboxTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image: String,
    pub default_command: String,
    pub default_cpu_cores: u32,
    pub default_memory_mb: u64,
    pub tags: Vec<String>,
}

/// Return the 4 built-in sandbox templates per mcp-spec.md §2.3.
pub fn builtin_templates() -> Vec<SandboxTemplate> {
    vec![
        SandboxTemplate {
            id: "node-dev".to_string(),
            name: "Node.js Development".to_string(),
            description: "Node.js 20 LTS with npm, yarn, and common dev tools".to_string(),
            image: "node:20-bookworm".to_string(),
            default_command: "sleep infinity".to_string(),
            default_cpu_cores: 2,
            default_memory_mb: 2048,
            tags: vec![
                "javascript".to_string(),
                "typescript".to_string(),
                "node".to_string(),
            ],
        },
        SandboxTemplate {
            id: "python-dev".to_string(),
            name: "Python Development".to_string(),
            description: "Python 3.12 with pip, venv, and common scientific packages".to_string(),
            image: "python:3.12-bookworm".to_string(),
            default_command: "sleep infinity".to_string(),
            default_cpu_cores: 2,
            default_memory_mb: 2048,
            tags: vec![
                "python".to_string(),
                "data-science".to_string(),
                "ml".to_string(),
            ],
        },
        SandboxTemplate {
            id: "rust-dev".to_string(),
            name: "Rust Development".to_string(),
            description: "Rust stable with cargo, rustfmt, clippy".to_string(),
            image: "rust:1-bookworm".to_string(),
            default_command: "sleep infinity".to_string(),
            default_cpu_cores: 4,
            default_memory_mb: 4096,
            tags: vec!["rust".to_string(), "systems".to_string()],
        },
        SandboxTemplate {
            id: "ubuntu-base".to_string(),
            name: "Ubuntu Base".to_string(),
            description: "Clean Ubuntu 24.04 with basic tools".to_string(),
            image: "ubuntu:24.04".to_string(),
            default_command: "sleep infinity".to_string(),
            default_cpu_cores: 1,
            default_memory_mb: 1024,
            tags: vec!["general".to_string(), "linux".to_string()],
        },
    ]
}

/// Look up a template by ID.
pub fn find_template(id: &str) -> Option<SandboxTemplate> {
    builtin_templates().into_iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_templates_count() {
        assert_eq!(builtin_templates().len(), 4);
    }

    #[test]
    fn test_builtin_template_ids() {
        let templates = builtin_templates();
        let ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"node-dev"));
        assert!(ids.contains(&"python-dev"));
        assert!(ids.contains(&"rust-dev"));
        assert!(ids.contains(&"ubuntu-base"));
    }

    #[test]
    fn test_template_images_are_set() {
        let templates = builtin_templates();
        for template in &templates {
            assert!(
                !template.image.is_empty(),
                "Template '{}' has empty image",
                template.id
            );
        }
    }

    #[test]
    fn test_template_default_resources() {
        let templates = builtin_templates();
        for template in &templates {
            assert!(
                template.default_cpu_cores >= 1,
                "Template '{}' has 0 cpu_cores",
                template.id
            );
            assert!(
                template.default_memory_mb >= 256,
                "Template '{}' has memory_mb < 256",
                template.id
            );
        }
    }

    #[test]
    fn test_template_default_command() {
        let templates = builtin_templates();
        for template in &templates {
            assert_eq!(
                template.default_command, "sleep infinity",
                "Template '{}' has unexpected default command",
                template.id
            );
        }
    }

    #[test]
    fn test_template_tags_not_empty() {
        let templates = builtin_templates();
        for template in &templates {
            assert!(
                !template.tags.is_empty(),
                "Template '{}' has no tags",
                template.id
            );
        }
    }

    #[test]
    fn test_node_dev_template_details() {
        let t = find_template("node-dev").expect("node-dev should exist");
        assert_eq!(t.name, "Node.js Development");
        assert_eq!(t.image, "node:20-bookworm");
        assert_eq!(t.default_cpu_cores, 2);
        assert_eq!(t.default_memory_mb, 2048);
        assert!(t.tags.contains(&"javascript".to_string()));
        assert!(t.tags.contains(&"typescript".to_string()));
    }

    #[test]
    fn test_rust_dev_template_details() {
        let t = find_template("rust-dev").expect("rust-dev should exist");
        assert_eq!(t.name, "Rust Development");
        assert_eq!(t.image, "rust:1-bookworm");
        assert_eq!(t.default_cpu_cores, 4);
        assert_eq!(t.default_memory_mb, 4096);
    }

    #[test]
    fn test_find_template() {
        assert!(find_template("node-dev").is_some());
        assert!(find_template("python-dev").is_some());
        assert!(find_template("rust-dev").is_some());
        assert!(find_template("ubuntu-base").is_some());
        assert!(find_template("nonexistent").is_none());
    }

    #[test]
    fn test_find_template_case_sensitive() {
        assert!(find_template("Node-Dev").is_none());
        assert!(find_template("NODE-DEV").is_none());
    }

    #[test]
    fn test_templates_serializable() {
        let templates = builtin_templates();
        let json = serde_json::to_string(&templates);
        assert!(json.is_ok(), "Templates should serialize to JSON");
        let json_str = json.unwrap();
        assert!(json_str.contains("node-dev"));
        assert!(json_str.contains("python-dev"));
    }
}
