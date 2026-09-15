//! The structured events the human render path draws from. The pipeline/dispatch layers emit
//! these; `print::renderer` decides how they look. No formatting or ANSI lives here -- this is
//! data only, so it stays trivial to construct in a test with a fixed value instead of a live
//! clock or terminal.

use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    Skip,
    /// Still running / not yet resolved -- distinct from every terminal outcome above.
    Progress,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    StageStarted {
        stage: String,
        /// The docs/LLD §3 node(s) this stage's real wiring exercises today -- named so a CLI
        /// run can be read against the design graph, not just this composition root's own
        /// 8-stage names. Never claims a node the wiring does not actually reach (see
        /// `PipelineStage::lld_nodes`'s doc comment for the honesty rule behind this field).
        lld_node: String,
    },
    StageFinished {
        stage: String,
        outcome: Outcome,
        elapsed: Duration,
    },
    LldPathStep {
        stage: String,
        index: usize,
        total: usize,
        node: String,
    },
    /// One gate's (or gate-shaped subprocess's) verdict, with its published denominator when one
    /// exists -- `checked`/`total` are `None` together, never independently absent.
    GateVerdict {
        id: String,
        outcome: Outcome,
        checked: Option<u64>,
        total: Option<u64>,
        detail: Option<String>,
    },
    /// One line of output attributed to a specific worker lane, so N workers' output never blurs
    /// together into one unattributed stream.
    Worker {
        lane: String,
        text: String,
    },
    Refusal {
        source: String,
        reason: String,
    },
    /// Process-level narration below a gate's own verdict -- e.g. "this one subprocess timed out"
    /// or "this one subprocess never got to run, the budget was already spent". Deliberately NOT
    /// `GateVerdict`/`Refusal`-shaped: those two render as terminal verdicts (`FAIL gate ...`,
    /// `REFUSED ...`), and a gate already gets exactly one such verdict from its registry-id
    /// `GateVerdict` once the run finishes. Rendering process narration the same way would read as
    /// a second, contradictory verdict for the same gate.
    Note {
        source: String,
        text: String,
    },
}
