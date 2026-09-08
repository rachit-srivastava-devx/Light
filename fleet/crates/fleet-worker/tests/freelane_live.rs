//! Pins the real behaviour restored into `crates/fleet-worker/src/freelane/`: `freelane.sh` was
//! deleted from `fleet/bin/` by a cleanup pass and never migrated, so `run_freelane()` in
//! `src/dispatch/agent_cmd_run.rs` used to return a hardcoded refusal unconditionally. This test
//! calls the SAME `fleet_worker::freelane::run` that call site now uses -- the real embedded
//! `freelane.sh` (`crates/fleet-worker/assets/freelane.sh`), materialized to a real temp dir and
//! actually executed, hitting the real keyless network lane -- and asserts it comes back `Ok`
//! with a genuine, non-empty model reply, not a fabricated or hardcoded result.
//!
//! Requires outbound internet access to the keyless lane (`api.llm7.io` by default, or
//! `$FREELANE_URL`/`$FREELANE_LANES` if overridden), so both tests are `#[ignore]`d: the free
//! lanes rate-limit, and a suite that goes red because an external endpoint throttled is
//! non-deterministic -- the same defect class as the machine-load dependence fixed on 2026-09-09.
//! These still exist and still prove the keyless thesis; they are opt-in, not deleted.
//! Run explicitly: `cargo test -p fleet-worker --test freelane_live -- --ignored --nocapture`

use std::path::Path;

#[test]
#[ignore = "hits the real keyless network; free lanes rate-limit -- run with --ignored"]
fn freelane_run_produces_a_genuine_response_over_the_real_network() {
    let repo = tempfile::tempdir().expect("tempdir");
    let worktree: &Path = repo.path();

    let result = fleet_worker::freelane::run(worktree, "Reply with exactly one word: pong", None);

    let out = result.unwrap_or_else(|err| {
        panic!(
            "freelane::run refused instead of answering -- either the keyless lane is genuinely \
             unavailable in this environment, or the asset restoration regressed: {err}"
        )
    });

    assert!(
        !out.response.trim().is_empty(),
        "a successful run must carry a real, non-empty reply, not an empty stub"
    );
    assert!(
        out.resolved_model.is_some(),
        "a real lane reports which model actually answered; got log={:?}",
        out.log
    );
}

/// Proves the restored asset is genuinely embedded (not read relative to cwd or
/// `CARGO_MANIFEST_DIR`) by running from a cwd that shares no ancestry with this crate's source.
#[test]
#[ignore = "hits the real keyless network; free lanes rate-limit -- run with --ignored"]
fn freelane_run_works_from_a_foreign_cwd() {
    let repo = tempfile::tempdir().expect("tempdir");
    let foreign_cwd = std::env::temp_dir();
    let original = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&foreign_cwd).expect("chdir to foreign cwd");

    let result = fleet_worker::freelane::run(repo.path(), "Reply with exactly one word: pong", None);

    std::env::set_current_dir(&original).expect("restore cwd");

    let out = result.unwrap_or_else(|err| {
        panic!("freelane::run failed from a foreign cwd -- location-independence regressed: {err}")
    });
    assert!(!out.response.trim().is_empty());
}
