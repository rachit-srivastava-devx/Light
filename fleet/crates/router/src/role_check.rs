//! The two routing-specific safety-gate rules. Re-homed from
//! `fleet/keel/fleet/src/roles.rs:83-110` as `RoleRefusal`/`evaluate_role_check` -- nothing
//! outside routing consumes these today, so they live here rather than in `fleet-types`.

use types::Role;

/// A safety-gate check, mirroring fleet's `roles::Check`/`roles::evaluate`.
#[derive(Debug, Eq, PartialEq)]
pub struct RoleCheck<'a> {
    pub role: Role,
    pub diff_adds_code: bool,
    pub builder_model: Option<&'a str>,
    pub verifier_model: Option<&'a str>,
}

/// Why a `RoleCheck` failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoleRefusal {
    /// A Lead-role check saw a diff that adds code. Leads architect; they do not implement.
    LeadWroteCode,
    /// A Verifier's resolved model equals the Builder's resolved model -- a model cannot verify
    /// its own work.
    SelfVerified,
}

impl RoleRefusal {
    pub fn reason(self) -> &'static str {
        match self {
            Self::LeadWroteCode => "LEAD_WROTE_CODE",
            Self::SelfVerified => "SELF_VERIFIED",
        }
    }
}

/// Evaluate one role-gate check. Pure, total, never panics.
pub fn evaluate_role_check(check: &RoleCheck<'_>) -> Result<(), RoleRefusal> {
    if check.role == Role::Lead && check.diff_adds_code {
        return Err(RoleRefusal::LeadWroteCode);
    }
    if check.role == Role::Verifier {
        if let (Some(builder), Some(verifier)) = (check.builder_model, check.verifier_model) {
            if builder == verifier {
                return Err(RoleRefusal::SelfVerified);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "role_check_tests.rs"]
mod tests;
