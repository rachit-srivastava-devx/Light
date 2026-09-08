//! Pins the `run.sh` collision this file's `display_name` exists to fix: two script gates
//! materialized under different subdirectories both end in `run.sh`; showing the bare file name
//! makes their process-level narration lines textually identical even though they're genuinely
//! different gates.

use super::display_name;

#[test]
fn distinguishes_two_scripts_sharing_a_basename() {
    let policy = display_name("/tmp/fleet-gates-xyz/policy/run.sh");
    let corpus = display_name("/tmp/fleet-gates-xyz/corpus/run.sh");
    assert_ne!(policy, corpus, "policy={policy} corpus={corpus}");
    assert_eq!(policy, "policy/run.sh");
    assert_eq!(corpus, "corpus/run.sh");
}

#[test]
fn bare_tool_name_is_left_alone() {
    assert_eq!(display_name("cargo"), "cargo");
}
