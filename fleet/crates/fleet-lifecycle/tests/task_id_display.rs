//! `TaskId`'s `Display` writes the raw id verbatim -- guards against a `fmt` implementation
//! that silently writes nothing (or a default) instead of the id.

use fleet_lifecycle::TaskId;

#[test]
fn display_writes_the_raw_id() {
    let id = TaskId::new("task-displayed-verbatim").unwrap();
    assert_eq!(id.to_string(), "task-displayed-verbatim");
    assert_eq!(format!("{id}"), "task-displayed-verbatim");
}
