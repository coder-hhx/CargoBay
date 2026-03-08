#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SandboxActionPolicy {
    pub(crate) risk_level: &'static str,
    pub(crate) requires_confirmation: bool,
}

pub(crate) fn sandbox_action_policy(action: &str) -> Option<SandboxActionPolicy> {
    match action {
        "sandbox_list" => Some(SandboxActionPolicy {
            risk_level: "read",
            requires_confirmation: false,
        }),
        "sandbox_start" | "sandbox_stop" => Some(SandboxActionPolicy {
            risk_level: "write",
            requires_confirmation: false,
        }),
        "sandbox_cleanup_expired" | "sandbox_exec" => Some(SandboxActionPolicy {
            risk_level: "write",
            requires_confirmation: true,
        }),
        "sandbox_delete" => Some(SandboxActionPolicy {
            risk_level: "destructive",
            requires_confirmation: true,
        }),
        _ => None,
    }
}
