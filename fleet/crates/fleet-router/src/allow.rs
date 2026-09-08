//! Role -> tier eligibility. Re-homed from `fleet/keel/fleet/src/route.rs:118-124`.

use crate::table::{CandidateSpec, Tier};
use fleet_types::Role;

/// Whether `candidate` is in the tier `role` is eligible to be routed to. One match arm per
/// `Role` variant, no wildcard `_ => ...` arm -- the compiler forces every `Role` to have an
/// explicit tier mapping.
pub(crate) fn role_allows(role: Role, candidate: CandidateSpec) -> bool {
    match role {
        Role::Lead | Role::Designer => candidate.tier == Tier::Lead,
        Role::Builder | Role::Verifier => candidate.tier == Tier::Worker,
        Role::Meter => candidate.tier == Tier::Cheap,
    }
}

#[cfg(test)]
mod tests {
    use super::role_allows;
    use crate::table::ORDER;
    use fleet_types::Role;

    #[test]
    fn role_allows_covers_every_role_exhaustively() {
        for role in Role::ALL {
            for candidate in ORDER {
                let _ = role_allows(role, *candidate); // must not panic for any pairing
            }
        }
    }
}
