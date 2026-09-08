//! `resume` reconstructs every one of the 16 states from its wire name, and refuses an
//! unknown one.

mod common;

use common::variant_name;
use fleet_lifecycle::{resume, TaskId};

const ALL_STATES: [&str; 16] = [
    "Intake", "Specified", "Reviewed", "Decomposed", "Contracted", "Briefed", "Leased",
    "Building", "Built", "Verifying", "Verified", "Attested", "Accepted", "Proposed",
    "Observed", "Refused",
];

#[test]
fn resume_reconstructs_every_one_of_the_16_states() {
    for name in ALL_STATES {
        let id = TaskId::new("resume-task").unwrap();
        let any = resume(name, id, 0).unwrap();
        assert_eq!(variant_name(&any), name);
    }
}

#[test]
fn resume_refuses_an_unknown_state_name() {
    let id = TaskId::new("resume-task").unwrap();
    let refusal = resume("Bogus", id, 0).unwrap_err();
    assert_eq!(refusal.code(), "UNKNOWN_LIFECYCLE_STATE");
}
