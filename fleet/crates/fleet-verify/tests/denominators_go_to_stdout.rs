//! A gate's published denominator MUST reach stdout, because that is the only stream the
//! `parse_denominator` fns read.
//!
//! `recur-gate.sh` printed its `checked=/flagged=` line to `/dev/stderr`, so `parsers::recur`
//! saw an empty string and returned `Unparseable` on EVERY run since the gate was written -- it
//! never published a count once. Nothing noticed, because the shared-deadline bug in
//! `verify_ports.rs` starved that gate into a timeout 124 before its output mattered. Two
//! independent defects hiding each other is why this is a test and not a comment.
//!
//! The behavioural half lives in `recur_publishes_denominator.rs` (80-line cap).

use std::path::{Path, PathBuf};

pub fn gates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("gates")
}

/// No gate script may redirect a denominator-bearing line to stderr. Cheap, needs no external
/// tool, and catches the whole class rather than the one instance that bit us.
#[test]
fn no_gate_script_writes_its_denominator_to_stderr() {
    let mut checked = 0usize;
    let mut offenders = Vec::new();
    let entries = std::fs::read_dir(gates_dir()).expect("gates dir must exist");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sh") {
            continue;
        }
        let body = std::fs::read_to_string(&path).expect("gate script must be readable");
        checked += 1;
        for (n, line) in body.lines().enumerate() {
            let publishes = line.contains("checked=")
                || line.contains("denominator:")
                || line.contains("files scanned");
            if publishes && line.contains("/dev/stderr") {
                offenders.push(format!("{}:{}", path.display(), n + 1));
            }
        }
    }
    // Publish the denominator for this check itself -- passing on an empty input set is a defect.
    assert!(checked >= 4, "expected several gate scripts to scan, saw {checked}");
    assert!(offenders.is_empty(), "denominator written to stderr at: {offenders:?}");
}
