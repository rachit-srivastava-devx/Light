//! The committed candidate table. Re-homed from `fleet/keel/fleet/src/route.rs:15-52`.

/// The tier a role maps to. `role_allows` (in `allow.rs`) is the only place tier <-> role is
/// decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tier {
    Lead,
    Worker,
    Cheap,
}

/// One entry in the committed, order-sensitive candidate table. Order changes are a policy
/// change and require review (see `ORDER`'s doc comment for the D50 history this preserves).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateSpec {
    /// Stable identity used in `Decision`, `RuntimeState.preference`, and receipts. Never renamed
    /// once shipped -- receipts and dashboards key on it.
    pub id: &'static str,
    /// The adapter family this candidate resolves through (`"claude"`, `"codex"`, `"freelane"`).
    /// Multiple candidates may share an adapter (e.g. opus/sonnet/haiku all resolve via "claude").
    pub adapter: &'static str,
    /// The model alias as requested of the adapter.
    pub requested: &'static str,
    /// The model identity actually used once resolved -- may differ from `requested` (aliases).
    pub resolved: &'static str,
    pub tier: Tier,
}

/// The committed candidate order and the deterministic tie-breaker. Stage 6 walks this table
/// top-to-bottom and picks the first id that both survived every prior filter AND appears in
/// `RuntimeState.preference` -- so changing this table's order changes routing outcomes;
/// changing it is a reviewed policy change, not a code change like any other.
///
/// D50: `freelane` was a runnable adapter the router could never select. Found by running a
/// whole session: the plan said "routed lane: UNAVAILABLE" while the one lane that was up and
/// measured sat outside the table. Ordered LAST deliberately -- it is the keyless subprocess
/// fallback, preferred only when every authenticated CLI is unavailable or out of quota, never
/// over them.
pub const ORDER: &[CandidateSpec] = &[
    CandidateSpec { id: "codex",    adapter: "codex",    requested: "codex-worker",    resolved: "codex",  tier: Tier::Worker },
    CandidateSpec { id: "sonnet",   adapter: "claude",   requested: "claude-sonnet",   resolved: "sonnet", tier: Tier::Worker },
    CandidateSpec { id: "opus",     adapter: "claude",   requested: "opus",            resolved: "opus",   tier: Tier::Lead },
    CandidateSpec { id: "haiku",    adapter: "claude",   requested: "haiku",           resolved: "haiku",  tier: Tier::Cheap },
    CandidateSpec { id: "freelane", adapter: "freelane", requested: "codestral-latest", resolved: "codestral-latest", tier: Tier::Worker },
];

/// Coarse task classification the safety-policy stage (stage 2) reasons over.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskClass {
    General,
    Implementation,
    HumanOnly,
}

#[cfg(test)]
mod tests {
    use super::ORDER;

    #[test]
    fn order_ids_are_pairwise_distinct() {
        for (i, a) in ORDER.iter().enumerate() {
            for b in &ORDER[i + 1..] {
                assert_ne!(a.id, b.id, "duplicate candidate id {}", a.id);
            }
        }
    }
}
