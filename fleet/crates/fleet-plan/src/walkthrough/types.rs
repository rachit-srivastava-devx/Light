//! The `Walkthrough` shape: ordered, ranked sections an owner with ADHD can skim in seconds --
//! no walls of text, every item traceable to one module brief field.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildItem {
    pub node_id: String,
    pub purpose: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkOrderItem {
    pub position: u32,
    pub node_id: String,
    pub because: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionItem {
    pub node_id: String,
    pub claim: String,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskItem {
    pub node_id: String,
    pub risk: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptancePreviewItem {
    pub node_id: String,
    pub given: String,
    pub when: String,
    pub then: String,
    pub oracle_kind: String,
}

/// Six ordered sections: what will be built, the work order and why, key design decisions with
/// rationale, risks, what the acceptance checks will prove, and what to look at closely.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Walkthrough {
    pub title: String,
    pub what_will_be_built: Vec<BuildItem>,
    pub work_order: Vec<WorkOrderItem>,
    pub key_decisions: Vec<DecisionItem>,
    pub risks: Vec<RiskItem>,
    pub acceptance_preview: Vec<AcceptancePreviewItem>,
    /// node_ids ranked highest-attention-first; never empty for a non-refused walkthrough.
    pub owner_focus: Vec<String>,
}
