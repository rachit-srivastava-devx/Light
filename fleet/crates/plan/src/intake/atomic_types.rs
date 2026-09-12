//! `AtomicTier`/`AtomicRow` -- the atomic-decomposition row shape `validate_atomic` reads
//! (`intake.sh:203-227`), already TSV-parsed by the caller.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicTier {
    Feature,
    Service,
    Module,
}

impl AtomicTier {
    pub(crate) fn name(self) -> &'static str {
        match self {
            AtomicTier::Feature => "feature",
            AtomicTier::Service => "service",
            AtomicTier::Module => "module",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicRow {
    pub id: String,
    pub tier: AtomicTier,
    pub parents: Vec<String>,
    pub description: String,
    pub inputs: String,
    pub outputs: String,
    pub acceptance: String,
    pub design_decision: String,
}
