#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SandboxErrorKind {
    Runtime,
    Permission,
    Template,
    Resource,
    Validation,
    NotFound,
    Internal,
}

impl SandboxErrorKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            SandboxErrorKind::Runtime => "runtime",
            SandboxErrorKind::Permission => "permission",
            SandboxErrorKind::Template => "template",
            SandboxErrorKind::Resource => "resource",
            SandboxErrorKind::Validation => "validation",
            SandboxErrorKind::NotFound => "not_found",
            SandboxErrorKind::Internal => "internal",
        }
    }
}

pub(crate) fn sandbox_error(kind: SandboxErrorKind, message: impl Into<String>) -> String {
    format!("sandbox_error::{}::{}", kind.as_str(), message.into())
}

pub(crate) fn sandbox_validation_error(message: impl Into<String>) -> String {
    sandbox_error(SandboxErrorKind::Validation, message)
}

pub(crate) fn sandbox_template_error(message: impl Into<String>) -> String {
    sandbox_error(SandboxErrorKind::Template, message)
}

pub(crate) fn sandbox_not_managed_error() -> String {
    sandbox_error(
        SandboxErrorKind::Validation,
        "Target container is not a CrateBay-managed sandbox",
    )
}

pub(crate) fn sandbox_internal_error(message: impl Into<String>) -> String {
    sandbox_error(SandboxErrorKind::Internal, message)
}

fn sandbox_classify_raw_message(raw: &str) -> SandboxErrorKind {
    let lower = raw.to_ascii_lowercase();

    if lower.contains("permission denied")
        || lower.contains("operation not permitted")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
    {
        SandboxErrorKind::Permission
    } else if lower.contains("manifest unknown")
        || lower.contains("pull access denied")
        || lower.contains("repository does not exist")
        || lower.contains("name unknown")
        || lower.contains("not found")
        || lower.contains("404")
        || lower.contains("no such")
    {
        SandboxErrorKind::NotFound
    } else if lower.contains("cannot connect")
        || lower.contains("connection refused")
        || lower.contains("docker daemon")
        || lower.contains("socket")
        || lower.contains("timed out")
        || lower.contains("deadline exceeded")
        || lower.contains("eof")
        || lower.contains("no docker socket found")
    {
        SandboxErrorKind::Runtime
    } else if lower.contains("no space left")
        || lower.contains("insufficient memory")
        || lower.contains("out of memory")
        || lower.contains("oom")
        || lower.contains("quota")
        || lower.contains("resource busy")
    {
        SandboxErrorKind::Resource
    } else {
        SandboxErrorKind::Internal
    }
}

pub(crate) fn sandbox_connect_error(error: &impl std::fmt::Display) -> String {
    let raw = error.to_string();
    let kind = sandbox_classify_raw_message(&raw);
    sandbox_error(
        kind,
        format!("Docker-compatible runtime is unavailable: {}", raw),
    )
}

pub(crate) fn sandbox_image_pull_error(reference: &str, error: &impl std::fmt::Display) -> String {
    let raw = error.to_string();
    let kind = sandbox_classify_raw_message(&raw);
    sandbox_error(
        kind,
        format!("Failed to pull sandbox image {}: {}", reference, raw),
    )
}

pub(crate) fn sandbox_stream_error(
    action: &str,
    target: &str,
    error: &impl std::fmt::Display,
) -> String {
    let raw = error.to_string();
    let kind = sandbox_classify_raw_message(&raw);
    sandbox_error(
        kind,
        format!("Failed to stream {} {}: {}", action, target, raw),
    )
}

pub(crate) fn sandbox_docker_error(
    action: &str,
    target: &str,
    error: &impl std::fmt::Display,
) -> String {
    let raw = error.to_string();
    let kind = sandbox_classify_raw_message(&raw);
    sandbox_error(kind, format!("Failed to {} {}: {}", action, target, raw))
}
