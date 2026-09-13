use super::*;
use crate::pipeline::stage::PipelineStage;

#[test]
fn a_marked_stage_survives_a_fresh_handle_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let log = StepLog::open(dir.path(), "t1");
    assert!(!log.is_done(PipelineStage::Scan));
    log.mark_done(PipelineStage::Scan).unwrap();

    let reopened = StepLog::open(dir.path(), "t1");
    assert!(reopened.is_done(PipelineStage::Scan));
    assert!(!reopened.is_done(PipelineStage::Plan));
}

#[test]
fn module_step_log_isolated_per_module() {
    let dir = tempfile::tempdir().unwrap();
    let log = StepLog::open(dir.path(), "task-1");

    // Mark module-a as done
    log.mark_module_done("module-a", PipelineStage::Plan)
        .unwrap();

    // Module-a should be done
    assert!(log.is_module_done("module-a", PipelineStage::Plan));

    // Module-b should not be done yet
    assert!(!log.is_module_done("module-b", PipelineStage::Plan));

    // Mark module-b as done
    log.mark_module_done("module-b", PipelineStage::Plan)
        .unwrap();

    // Both should be done
    assert!(log.is_module_done("module-a", PipelineStage::Plan));
    assert!(log.is_module_done("module-b", PipelineStage::Plan));
}

#[test]
fn module_id_validation() {
    assert!(ModuleId::parse("valid-module").is_ok());
    assert!(ModuleId::parse("").is_err());
    assert!(ModuleId::parse("   ").is_err());
}
