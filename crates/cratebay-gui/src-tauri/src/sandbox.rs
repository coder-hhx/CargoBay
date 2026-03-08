use super::*;

mod audit;
mod error;
mod exec;
mod lifecycle;
mod policy;

pub(crate) use audit::*;
pub(crate) use error::*;
pub(crate) use exec::*;
pub(crate) use lifecycle::*;
pub(crate) use policy::*;

const SANDBOX_LABEL_MANAGED: &str = "com.cratebay.sandbox.managed";
const SANDBOX_LABEL_TEMPLATE_ID: &str = "com.cratebay.sandbox.template_id";
const SANDBOX_LABEL_OWNER: &str = "com.cratebay.sandbox.owner";
const SANDBOX_LABEL_CREATED_AT: &str = "com.cratebay.sandbox.created_at";
const SANDBOX_LABEL_EXPIRES_AT: &str = "com.cratebay.sandbox.expires_at";
const SANDBOX_LABEL_TTL_HOURS: &str = "com.cratebay.sandbox.ttl_hours";
const SANDBOX_LABEL_CPU_CORES: &str = "com.cratebay.sandbox.cpu_cores";
const SANDBOX_LABEL_MEMORY_MB: &str = "com.cratebay.sandbox.memory_mb";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SandboxTemplateDto {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) image: String,
    pub(crate) default_command: String,
    pub(crate) cpu_default: u32,
    pub(crate) memory_mb_default: u64,
    pub(crate) ttl_hours_default: u32,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct SandboxTemplateDef {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    image: &'static str,
    default_command: &'static str,
    cpu_default: u32,
    memory_mb_default: u64,
    ttl_hours_default: u32,
    tags: &'static [&'static str],
    default_env: &'static [&'static str],
}

fn sandbox_template_defs() -> Vec<SandboxTemplateDef> {
    vec![
        SandboxTemplateDef {
            id: "node-dev",
            name: "Node.js Dev",
            description: "Node 20 development runtime for coding agents and MCP tasks",
            image: "node:20-bookworm",
            default_command: "sleep infinity",
            cpu_default: 2,
            memory_mb_default: 2048,
            ttl_hours_default: 8,
            tags: &["node", "javascript", "typescript"],
            default_env: &["CRATEBAY_SANDBOX=1", "NODE_ENV=development"],
        },
        SandboxTemplateDef {
            id: "python-dev",
            name: "Python Dev",
            description: "Python 3.11 runtime for agent tools, scripts, and notebooks",
            image: "python:3.11-bookworm",
            default_command: "sleep infinity",
            cpu_default: 2,
            memory_mb_default: 3072,
            ttl_hours_default: 8,
            tags: &["python", "llm-tools", "automation"],
            default_env: &["CRATEBAY_SANDBOX=1", "PYTHONUNBUFFERED=1"],
        },
        SandboxTemplateDef {
            id: "rust-dev",
            name: "Rust Dev",
            description: "Rust toolchain runtime for compile/test oriented agent workflows",
            image: "rust:1.77-bookworm",
            default_command: "sleep infinity",
            cpu_default: 2,
            memory_mb_default: 4096,
            ttl_hours_default: 8,
            tags: &["rust", "cargo", "systems"],
            default_env: &["CRATEBAY_SANDBOX=1", "CARGO_TERM_COLOR=always"],
        },
    ]
}

fn sandbox_templates_catalog() -> Vec<SandboxTemplateDto> {
    sandbox_template_defs()
        .into_iter()
        .map(|it| SandboxTemplateDto {
            id: it.id.to_string(),
            name: it.name.to_string(),
            description: it.description.to_string(),
            image: it.image.to_string(),
            default_command: it.default_command.to_string(),
            cpu_default: it.cpu_default,
            memory_mb_default: it.memory_mb_default,
            ttl_hours_default: it.ttl_hours_default,
            tags: it.tags.iter().map(|v| v.to_string()).collect(),
        })
        .collect()
}

fn sandbox_find_template(template_id: &str) -> Option<SandboxTemplateDef> {
    sandbox_template_defs()
        .into_iter()
        .find(|it| it.id == template_id)
}

pub(crate) fn sandbox_short_id(id: &str) -> String {
    id.chars().take(12).collect::<String>()
}

fn sandbox_parse_u32_label(labels: &HashMap<String, String>, key: &str, default: u32) -> u32 {
    labels
        .get(key)
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(default)
}

fn sandbox_parse_u64_label(labels: &HashMap<String, String>, key: &str, default: u64) -> u64 {
    labels
        .get(key)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

pub(crate) fn sandbox_is_managed(labels: &HashMap<String, String>) -> bool {
    labels
        .get(SANDBOX_LABEL_MANAGED)
        .map(|v| v == "true")
        .unwrap_or(false)
}

pub(crate) fn sandbox_is_expired(expires_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(expires_at)
        .map(|dt| dt.with_timezone(&chrono::Utc) <= chrono::Utc::now())
        .unwrap_or(false)
}

fn sandbox_default_owner() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "local-user".to_string())
}

fn sandbox_normalize_owner(owner: Option<String>) -> String {
    let mut value = owner
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(sandbox_default_owner);
    if value.len() > 64 {
        value = value.chars().take(64).collect();
    }
    value
}

pub(crate) fn sandbox_normalize_env(env: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for item in env.unwrap_or_default() {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > 512 {
            return Err("sandbox env entry is too long".to_string());
        }
        if trimmed.contains('\0') {
            return Err("sandbox env entry contains null byte".to_string());
        }
        if !trimmed.contains('=') {
            return Err(format!(
                "sandbox env entry '{}' must follow KEY=VALUE",
                trimmed
            ));
        }
        let key = trimmed.split('=').next().unwrap_or_default().trim();
        if key.is_empty() {
            return Err(format!("sandbox env entry '{}' has empty key", trimmed));
        }
        if !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(format!(
                "sandbox env key '{}' contains invalid characters",
                key
            ));
        }
        out.push(trimmed.to_string());
    }
    if out.len() > 64 {
        return Err("sandbox env has too many entries (max 64)".to_string());
    }
    Ok(out)
}

fn sandbox_generate_name(template_id: &str) -> String {
    let suffix = SANDBOX_SEQ.fetch_add(1, Ordering::Relaxed) % 10_000;
    let stamp = chrono::Utc::now().format("%m%d%H%M%S");
    let mut name = format!("cbx-{}-{}-{:04}", template_id, stamp, suffix);
    if name.len() > 128 {
        name = name.chars().take(128).collect();
    }
    name
}

#[derive(Debug, Serialize)]
pub(crate) struct SandboxCleanupResultDto {
    pub(crate) removed_count: usize,
    pub(crate) removed_names: Vec<String>,
    pub(crate) message: String,
}
