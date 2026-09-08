//! The two routing-specific safety-gate rules. Re-homed from
//! `fleet/keel/fleet/src/roles.rs:83-110` as `RoleRefusal`/`evaluate_role_check` -- nothing
//! outside routing consumes these today, so they live here rather than in `fleet-types`.

use fleet_types::Role;

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
mod tests {
    use super::*;

    #[test]
    fn lead_code_gate_covers_both_directions() {
        let wrote_code = RoleCheck { role: Role::Lead, diff_adds_code: true, builder_model: None, verifier_model: None };
        assert_eq!(evaluate_role_check(&wrote_code), Err(RoleRefusal::LeadWroteCode));

        let no_code = RoleCheck { role: Role::Lead, diff_adds_code: false, builder_model: None, verifier_model: None };
        assert_eq!(evaluate_role_check(&no_code), Ok(()));
    }

    #[test]
    fn verifier_self_check_covers_both_directions() {
        let same = RoleCheck { role: Role::Verifier, diff_adds_code: false, builder_model: Some("sonnet"), verifier_model: Some("sonnet") };
        assert_eq!(evaluate_role_check(&same), Err(RoleRefusal::SelfVerified));

        let distinct = RoleCheck { role: Role::Verifier, diff_adds_code: false, builder_model: Some("sonnet"), verifier_model: Some("codex") };
        assert_eq!(evaluate_role_check(&distinct), Ok(()));
    }
}
