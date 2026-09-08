//! `Role` -- fleet's five swarm roles. Re-homed from `fleet/keel/fleet/src/roles.rs:1-65`.

/// One of fleet's five swarm roles.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Lead,
    Builder,
    Verifier,
    Designer,
    Meter,
}

/// `Role::parse` could not match `value` against any of the five lowercase wire names.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown role {0:?}: expected one of lead, builder, verifier, designer, meter")]
pub struct UnknownRole(pub String);

/// Per-role fixed facts: (name, bandwidth, allocation_fitness, owned_gate, may_write_code).
const FACTS: [(&str, u64, u64, &str, bool); 5] = [
    ("lead", 2, 3, "contract", false),
    ("builder", 4, 5, "implementation", true),
    ("verifier", 3, 4, "independent-verification", false),
    ("designer", 2, 3, "design-a11y", false),
    ("meter", 2, 2, "budget", false),
];

impl Role {
    /// All five roles, in the fixed dashboard/receipt order.
    pub const ALL: [Role; 5] =
        [Role::Lead, Role::Builder, Role::Verifier, Role::Designer, Role::Meter];

    fn facts(self) -> (&'static str, u64, u64, &'static str, bool) {
        FACTS[self as usize]
    }

    /// Parse a role from its lowercase wire name. Never case-folds or trims.
    pub fn parse(value: &str) -> Result<Self, UnknownRole> {
        Self::ALL
            .into_iter()
            .find(|role| role.name() == value)
            .ok_or_else(|| UnknownRole(value.to_string()))
    }

    /// The lowercase wire name.
    pub fn name(self) -> &'static str {
        self.facts().0
    }

    /// Fixed scheduling weight consumed by `fleet-govern`'s allocator.
    pub fn bandwidth(self) -> u64 {
        self.facts().1
    }

    /// Fixed scheduling fitness score consumed by `fleet-govern`'s allocator.
    pub fn allocation_fitness(self) -> u64 {
        self.facts().2
    }

    /// The gate name this role has sole authority over.
    pub fn owned_gate(self) -> &'static str {
        self.facts().3
    }

    /// Whether this role may submit a diff that adds implementation code.
    pub fn may_write_code(self) -> bool {
        self.facts().4
    }
}
