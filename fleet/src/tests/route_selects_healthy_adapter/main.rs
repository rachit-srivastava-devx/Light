//! `fleet route --role <role> --json` used to refuse for EVERY role, unconditionally, on any
//! machine no matter what adapters were actually installed. Root cause: `route_cmd`'s own
//! `RuntimeState` builder collected candidate IDs ("codex","sonnet","opus","haiku","freelane")
//! into `capable`, but `filter_capability` checks `.adapter` ("codex","claude","freelane") --
//! so every `claude`-family candidate (sonnet/opus/haiku) silently dropped at stage 3 -- and left
//! `remaining` empty, which `filter_quota` treats as "never usable, never unlimited", dropping
//! everyone else at stage 4. `fleet run` never had this bug: its own `healthy_runtime()` builds
//! the same snapshot correctly and routes fine. This pins `route` to that same "everything
//! installed, generous quota" default so a role with an eligible tier and an installed adapter
//! actually gets routed, not refused.
//!
//! Scoped to `builder`/`lead` only. `verifier` hits a separate, later gate (stage 5, "verifier
//! independence": `route_cmd` always passes `None` for the builder's resolved model, and
//! `filter_verifier` deliberately clears every candidate when that is `None` -- a real gap, but a
//! distinct one from the `RuntimeState` bug this test pins, and a safety-relevant design
//! question this test does not decide.

#[path = "../support/mod.rs"]
mod support;
use serde_json::Value;
use support::cmd;

/// `builder`/`lead` both have an eligible tier and at least one adapter present in the committed
/// candidate table (`crates/router/src/table.rs::ORDER`) -- `fleet route --role <role> --json`
/// must select one, not refuse, on a machine with the eligible CLIs installed.
#[test]
fn route_selects_an_adapter_for_builder_and_lead() {
    let state_dir = tempfile::tempdir().unwrap();
    for role in ["builder", "lead"] {
        let out = cmd()
            .env("FLEET_STATE_DIR", state_dir.path())
            .args(["route", "--role", role, "--json"])
            .output()
            .expect("binary runs");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "role={role} must route to a healthy adapter, not refuse (exit {:?}). stdout={stdout} stderr={}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );

        let v: Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("role={role}: expected one JSON object, got err {e}: {stdout}"));
        let selected = v["selected_adapter"]
            .as_str()
            .unwrap_or_else(|| panic!("role={role}: missing selected_adapter: {v}"));
        assert!(
            selected.starts_with("Some("),
            "role={role}: expected a selected adapter, got {selected}"
        );
    }
}
