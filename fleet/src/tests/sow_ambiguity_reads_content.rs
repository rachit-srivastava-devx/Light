//! `fleet sow`'s violation list used to come only from structural section-heading checks, so a
//! one-line vague prompt and a fully-specified spec produced byte-for-byte identical output --
//! the check never actually read what the text said. `plan_cmd::sow` now also runs
//! `fleet_scan::assess` over the raw text; this drives the real binary and asserts a detailed
//! spec yields strictly fewer ambiguity findings (and different output overall) than a vague one.

mod support;
use support::cmd;

const VAGUE: &str = "fix the flaky pagination test";
const DETAILED: &str = "Add a rate limiter to POST /api/auth/login using a token-bucket keyed by \
client IP, 5 req/min, return 429 with Retry-After header on exceed";

fn run(text: &str) -> String {
    // Each call gets its own throwaway `FLEET_STATE_DIR` -- `sow` now writes a memory row on
    // refusal (see `dispatch::memory_write`); without isolation, two calls in this file (or a
    // parallel test binary) could observe each other's recorded memory and make the ambiguity
    // counts below flaky. This test is about text content, not memory recall, so each call
    // starts from empty memory.
    let state_dir = tempfile::tempdir().expect("tempdir");
    let out = cmd()
        .env("FLEET_STATE_DIR", state_dir.path())
        .args(["sow", "--text", text, "--intent-hash", "x"])
        .output()
        .expect("binary runs");
    assert!(!out.status.success(), "a bare --text with no SOW sections should still be refused");
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn ambiguity_lines(stderr: &str) -> usize {
    stderr.lines().filter(|l| l.contains("ambiguity (")).count()
}

#[test]
fn detailed_spec_produces_strictly_fewer_ambiguity_findings_than_vague_prompt() {
    let vague_out = run(VAGUE);
    let detailed_out = run(DETAILED);

    assert_ne!(vague_out, detailed_out, "vague and detailed sow output must not be identical");

    let vague_n = ambiguity_lines(&vague_out);
    let detailed_n = ambiguity_lines(&detailed_out);
    assert!(
        detailed_n < vague_n,
        "expected detailed spec to raise fewer ambiguity findings than the vague prompt \
         (vague={vague_n}, detailed={detailed_n})\nvague:\n{vague_out}\ndetailed:\n{detailed_out}"
    );
}

#[test]
fn vague_prompt_flags_missing_audience() {
    let out = run(VAGUE);
    assert!(out.contains("Who is this requirement for?"), "vague output: {out}");
}

#[test]
fn detailed_spec_does_not_flag_missing_audience() {
    let out = run(DETAILED);
    assert!(!out.contains("Who is this requirement for?"), "detailed output: {out}");
}
