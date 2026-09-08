//! `advance_any` walks the canonical edge from every resumable state, except `Accepted`
//! (needs real PR-emit evidence) and `Refused` (no forward edge at all).

mod common;

use common::{variant_name, MemoryLedger};
use fleet_lifecycle::{advance_any, resume, TaskId};

#[test]
fn advance_any_walks_the_canonical_edge_from_every_resumable_state() {
    let expected: [(&str, &str); 14] = [
        ("Intake", "Specified"), ("Specified", "Reviewed"), ("Reviewed", "Decomposed"),
        ("Decomposed", "Contracted"), ("Contracted", "Briefed"), ("Briefed", "Leased"),
        ("Leased", "Building"), ("Building", "Built"), ("Built", "Verifying"),
        ("Verifying", "Verified"), ("Verified", "Attested"), ("Attested", "Accepted"),
        ("Proposed", "Observed"), ("Observed", "Intake"),
    ];
    for (from, to) in expected {
        let ledger = MemoryLedger::default();
        let id = TaskId::new("advance-task").unwrap();
        let any = resume(from, id, 0).unwrap();
        let advanced = advance_any(any, "evidence", &ledger).unwrap();
        assert_eq!(variant_name(&advanced), to);
    }
}

#[test]
fn advance_any_refuses_accepted_naming_the_real_entry_point() {
    let ledger = MemoryLedger::default();
    let id = TaskId::new("advance-accepted").unwrap();
    let any = resume("Accepted", id, 0).unwrap();
    let refusal = advance_any(any, "evidence", &ledger).unwrap_err();
    assert_eq!(refusal.code(), "PR_EMIT_REQUIRES_EVIDENCE");
}

#[test]
fn advance_any_refuses_from_refused() {
    let ledger = MemoryLedger::default();
    let id = TaskId::new("advance-refused").unwrap();
    let any = resume("Refused", id, 0).unwrap();
    let refusal = advance_any(any, "evidence", &ledger).unwrap_err();
    assert_eq!(refusal.code(), "ILLEGAL_LIFECYCLE_TRANSITION");
}
