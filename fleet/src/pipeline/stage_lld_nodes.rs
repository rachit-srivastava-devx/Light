//! `PipelineStage::lld_nodes` -- split out of `stage.rs` to stay under the 80-line file gate.
//!
//! Maps each of the 8 real composition-root stages to the docs/LLD §3 node(s) its *current call
//! chain actually reaches*, for CLI-mode narration (`stage_report::started`). This is deliberately
//! not a 1:1 label lookup against the full §3 graph: most of that graph (`Discovery`/`Catalog`,
//! the `Business`/`Tech`/`Learn`/`Research` ambiguity fan-out, `Ideation`, `PlanReview`,
//! `QualityLedger`, `Approval`, `Broker`, `CentralSync`, `Offline`) has no caller in this binary
//! yet -- `route`'s own `admit`/`CatalogSnapshot` pipeline is committed scaffolding with "no
//! caller yet" per its own module doc, and `Scan`/`Plan` (see `stages.rs`) call their crate with a
//! placeholder input, not a real per-task one. Naming an unwired node here would be exactly the
//! confabulation L8 forbids, so each stage names only the node(s) its current call chain reaches,
//! and the two stub stages say so plainly rather than borrowing a fuller node name.

use super::stage::PipelineStage;

impl PipelineStage {
    pub fn lld_nodes(self) -> &'static str {
        match self {
            PipelineStage::Event => "Ingest -> Store (ledger: RunStart)",
            PipelineStage::Classify => {
                "Intent -> Route (6-stage decide; Discovery/Catalog not wired)"
            }
            PipelineStage::Scan => "Scan (stub: merges zero questions, no ambiguity agents run)",
            PipelineStage::Plan => "DAG -> Planner (stub: fixed draft, not per-task)",
            PipelineStage::Dispatch => {
                "Route -> Ready -> build-queue handoff (no Builder process spawned here)"
            }
            PipelineStage::Verify => "Verify (real gate table, ledgered; QualityLedger not wired)",
            PipelineStage::Merge => {
                "Integrate (stage-nonempty guard only, no commit performed here)"
            }
            PipelineStage::Teach => "Teach (lesson -> sow-memory on failure only)",
        }
    }
}
