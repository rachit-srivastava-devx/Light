//! One node in the fixed 8-stage pipeline graph. A stage may only hand off to the next one in
//! this list, or to `Teach` from any stage on failure -- enforced by an exhaustive match with no
//! wildcard arm (BLUEPRINT §4).

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum PipelineStage {
    Event,
    Classify,
    Scan,
    Plan,
    Dispatch,
    Verify,
    Merge,
    Teach,
}

impl PipelineStage {
    pub const ALL: [PipelineStage; 8] = [
        PipelineStage::Event,
        PipelineStage::Classify,
        PipelineStage::Scan,
        PipelineStage::Plan,
        PipelineStage::Dispatch,
        PipelineStage::Verify,
        PipelineStage::Merge,
        PipelineStage::Teach,
    ];

    /// The one successor this stage advances to on success. `Teach` is terminal: `None`.
    pub fn next(self) -> Option<PipelineStage> {
        match self {
            PipelineStage::Event => Some(PipelineStage::Classify),
            PipelineStage::Classify => Some(PipelineStage::Scan),
            PipelineStage::Scan => Some(PipelineStage::Plan),
            PipelineStage::Plan => Some(PipelineStage::Dispatch),
            PipelineStage::Dispatch => Some(PipelineStage::Verify),
            PipelineStage::Verify => Some(PipelineStage::Merge),
            PipelineStage::Merge => Some(PipelineStage::Teach),
            PipelineStage::Teach => None,
        }
    }

    /// Every legal successor on failure: any stage may fail forward into `Teach`, which itself
    /// has no successor at all (a required trailer, not a conditional stage).
    pub fn allowed_successors(self) -> Vec<PipelineStage> {
        match (self, self.next()) {
            (PipelineStage::Teach, _) => vec![],
            (_, Some(n)) => vec![n, PipelineStage::Teach],
            (_, None) => vec![PipelineStage::Teach],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_stage_edges_are_exhaustive_and_directional() {
        for (i, stage) in PipelineStage::ALL.iter().enumerate() {
            let succ = stage.allowed_successors();
            if *stage == PipelineStage::Teach {
                assert!(succ.is_empty(), "Teach must be terminal");
            } else {
                let expected_next = PipelineStage::ALL[i + 1];
                assert_eq!(succ, vec![expected_next, PipelineStage::Teach]);
            }
        }
    }
}
