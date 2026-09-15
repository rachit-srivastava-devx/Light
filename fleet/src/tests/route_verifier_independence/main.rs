//! Drives the REAL compiled `fleet` binary (a proxy is not the property) to pin two decisions
//! made about `fleet route --role verifier`:
//!
//! 1. With no `--builder-model`, it refuses at stage 5 ("verifier independence") BY DESIGN -- a
//!    bare route query has no builder context, so it cannot know whether the picked verifier
//!    would share the (unknown) builder's model. This is not the D-class bug it looks like; see
//!    `router::verify_gate::filter_verifier`'s own doc comment. Pinned here so nobody "fixes" it
//!    into a silent, unsafe default.
//! 2. `--builder-model` is real per-candidate filtering, not a bypass: passing the top-preference
//!    candidate's own resolved model must skip past it to the next distinct one, not clear the
//!    whole stage and not ignore the collision.
//!
//! Also guards the regression this uncovered: before the fix, `route_cmd.rs`'s runtime default
//! built `capable` from candidate `id`s instead of `adapter`s and left `remaining` empty, so
//! EVERY role refused at stage 3 or 4 regardless of `--builder-model` -- masking the stage-5
//! design question entirely. `builder_role_still_routes_when_independent` would fail on that bug
//! too (stage 3/4, not a clean route), so it doubles as the regression pin.

#[path = "../support/mod.rs"]
mod support;
use support::cmd;

const REFUSAL: i32 = 7; // types::ExitCode::Refusal

fn route(args: &[&str]) -> (i32, String, String) {
    let out = cmd()
        .env("FLEET_STATE_DIR", std::env::temp_dir())
        .arg("route")
        .args(args)
        .output()
        .expect("binary runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// No builder context given -- refuses, and names stage 5 specifically (not stage 3/4, which
/// would mean the unrelated capability/quota bug is masking the real design question again).
#[test]
fn bare_role_verifier_refuses_at_stage_five_by_design() {
    let (code, out, err) = route(&["--role", "verifier", "--json"]);
    assert_eq!(code, REFUSAL, "stdout: {out}\nstderr: {err}");
    assert!(
        out.is_empty(),
        "refusal must not also print a report: {out}"
    );
    assert!(err.contains("stage: 5"), "wrong stage: {err}");
    assert!(
        err.contains("verifier independence"),
        "wrong stage name: {err}"
    );
}

/// A builder model that collides with nothing lets the verifier route succeed for real.
#[test]
fn builder_model_distinct_from_every_candidate_routes_successfully() {
    let (code, out, err) = route(&[
        "--role",
        "verifier",
        "--builder-model",
        "some-unrelated-external-model",
        "--json",
    ]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(
        out.contains("codex"),
        "expected the top-preference verifier candidate: {out}"
    );
}

/// Passing the model of the candidate that would otherwise be picked first must skip only THAT
/// candidate, not clear the whole stage (proves real filtering, not a bypass flag) -- and must
/// not silently keep picking it either (proves the check actually ran).
#[test]
fn builder_model_matching_top_candidate_skips_only_that_one() {
    let (code, out, err) = route(&["--role", "verifier", "--builder-model", "codex", "--json"]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(
        !out.contains("\"codex\""),
        "still selected the colliding candidate: {out}"
    );
    assert!(
        out.contains("claude"),
        "expected the next distinct verifier candidate (sonnet, adapter claude): {out}"
    );
}
