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
