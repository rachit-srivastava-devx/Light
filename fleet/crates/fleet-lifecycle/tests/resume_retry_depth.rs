//! `resume` carries a caller-supplied `retry_depth` through to `Task::retry_depth()` -- guards
//! against a stubbed getter that always reports `0` regardless of what was resumed.

use fleet_lifecycle::{resume, AnyTask, TaskId};

#[test]
fn resumed_task_reports_the_supplied_retry_depth() {
    let id = TaskId::new("retry-depth-task").unwrap();
    let any = resume("Built", id, 7).unwrap();
    let AnyTask::Built(task) = any else { panic!("expected Built") };
    assert_eq!(task.retry_depth(), 7);
}
