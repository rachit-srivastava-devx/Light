//! Complete-attestation and sample-request fixtures shared across the integration suites.

use fleet_lifecycle::{HumanApproval, ProposalRequest, Task, TaskId};
use serde_json::{json, Value};
use std::path::PathBuf;

use super::MemoryLedger;

/// Walks a fresh task from `Intake` to `Accepted` through the typed edges -- shared setup
/// for every `propose`-gate test, which only cares about behavior at `Accepted`.
pub fn accepted_task(id: &str, ledger: &MemoryLedger) -> Task<fleet_lifecycle::Accepted> {
    let task = Task::new(TaskId::new(id).unwrap());
    let task = task.specify("SOW", ledger).unwrap();
    let task = task.review(HumanApproval::recorded("approval").unwrap(), ledger).unwrap();
    let task = task.decompose("leaves", ledger).unwrap();
    let task = task.contract("contract", ledger).unwrap();
    let task = task.brief("brief", ledger).unwrap();
    let task = task.lease("lease", ledger).unwrap();
    let task = task.build("build", ledger).unwrap();
    let task = task.finish_build("diff", ledger).unwrap();
    let task = task.begin_verification("verify started", ledger).unwrap();
    let task = task.verify("reproduced", ledger).unwrap();
    let task = task.attest("attested", ledger).unwrap();
    task.accept(HumanApproval::recorded("accepted").unwrap(), ledger).unwrap()
}

pub fn complete_elements() -> Value {
    let o1_hash = "a".repeat(64);
    let o2_hash = "b".repeat(64);
    json!({
        "sow": {"task": "unit-test-task"},
        "blind_suite": {
            "in_worktree_tree": true, "in_object_store": false,
            "in_env": false, "on_any_fd": false, "suite_hash": "deadbeef"
        },
        "independent_verification": {
            "builder": "builder-agent", "verifier": "verifier-agent",
            "distinct": true, "reproduced": true, "verdict": "ACCEPT"
        },
        "adequacy": {"status": "no-measurable-surface"},
        "blast_radius": {"files": ["main.rs"], "count": 1, "source": "recorded-diff"},
        "rollback": {"executed": true, "suite_went_red": false, "reverted_files": 1},
        "cost": {
            "wall_ms": 1, "characters_in_diff": 32, "tokens": null,
            "tokenizer_generation": null, "source": "observed"
        },
        "oracle_independence": {
            "o1_author": "lead", "o2_author": "verifier",
            "o1_hash": o1_hash, "o2_hash": o2_hash, "distinct": true, "quadrant": "ACCEPT"
        }
    })
}

pub fn sample_request(head: &str) -> ProposalRequest {
    let diff = b"diff --git a/main.rs b/main.rs\nunit test fixture\n".to_vec();
    let artifact_id = blake3::hash(&diff).to_hex().to_string();
    ProposalRequest {
        repo: PathBuf::from("/nonexistent/lifecycle-unit-test"),
        base: "main".to_string(),
        head: head.to_string(),
        artifact_id,
        diff,
        title: format!("fleet: {head}"),
        body: "evidence bundle -- not to be self-merged".to_string(),
    }
}
