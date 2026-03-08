use super::*;

pub(crate) fn sandbox_audit_path() -> PathBuf {
    cratebay_core::config_dir()
        .join("audit")
        .join("sandboxes.jsonl")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SandboxAuditEventDto {
    pub(crate) timestamp: String,
    pub(crate) action: String,
    pub(crate) sandbox_id: String,
    pub(crate) sandbox_name: String,
    pub(crate) level: String,
    pub(crate) detail: String,
}

pub(crate) fn sandbox_audit_log(
    action: &str,
    sandbox_id: &str,
    sandbox_name: &str,
    level: &str,
    detail: &str,
) {
    let path = sandbox_audit_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let event = SandboxAuditEventDto {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: action.to_string(),
        sandbox_id: sandbox_id.to_string(),
        sandbox_name: sandbox_name.to_string(),
        level: level.to_string(),
        detail: cratebay_core::validation::sanitize_log_string(detail),
    };
    if let Ok(line) = serde_json::to_string(&event) {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{}", line);
        }
    }
}
#[tauri::command]
pub(crate) fn sandbox_audit_list(
    limit: Option<usize>,
) -> Result<Vec<SandboxAuditEventDto>, String> {
    let path = sandbox_audit_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| {
        sandbox_internal_error(format!(
            "Failed to read sandbox audit log {}: {}",
            path.display(),
            e
        ))
    })?;
    let limit = limit.unwrap_or(60).clamp(1, 500);
    let mut events = raw
        .lines()
        .filter_map(|line| serde_json::from_str::<SandboxAuditEventDto>(line).ok())
        .collect::<Vec<_>>();
    if events.len() > limit {
        events = events.split_off(events.len() - limit);
    }
    events.reverse();
    Ok(events)
}
