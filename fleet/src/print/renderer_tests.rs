//! Golden tests for `render`: a fixed `Event` + `Style` must always produce byte-identical text.

use super::render;
use crate::print::render_event::{Event, Outcome};
use crate::print::style::Style;
use std::time::Duration;

const PLAIN: Style = Style::new(false);
const COLOR: Style = Style::new(true);

#[test]
fn stage_started_plain() {
    let e = Event::StageStarted { stage: "Verify".into() };
    assert_eq!(render(&e, &PLAIN), "\u{25b6} stage Verify");
}

#[test]
fn stage_finished_plain_pass() {
    let elapsed = Duration::from_millis(1230);
    let e = Event::StageFinished { stage: "Verify".into(), outcome: Outcome::Pass, elapsed };
    assert_eq!(render(&e, &PLAIN), "  PASS stage Verify (1.23s)");
}

#[test]
fn stage_finished_plain_fail() {
    let e = Event::StageFinished { stage: "Merge".into(), outcome: Outcome::Fail, elapsed: Duration::from_secs(4) };
    assert_eq!(render(&e, &PLAIN), "  FAIL stage Merge (4.00s)");
}

#[test]
fn gate_verdict_plain_with_denominator() {
    let e = Event::GateVerdict { id: "unit tests".into(), outcome: Outcome::Pass, checked: Some(422), total: Some(422), detail: None };
    assert_eq!(render(&e, &PLAIN), "    PASS gate unit tests 422/422");
}

#[test]
fn gate_verdict_plain_fail_with_detail_no_denominator() {
    let e = Event::GateVerdict { id: "semgrep".into(), outcome: Outcome::Fail, checked: None, total: None, detail: Some("Unparseable".into()) };
    assert_eq!(render(&e, &PLAIN), "    FAIL gate semgrep -- Unparseable");
}

#[test]
fn worker_line_plain_is_lane_attributed() {
    let e = Event::Worker { lane: "lane-builder-1".into(), text: "compiling...".into() };
    assert_eq!(render(&e, &PLAIN), "    [lane-builder-1] compiling...");
}

#[test]
fn refusal_plain() {
    let e = Event::Refusal { source: "cargo".into(), reason: "verify budget already spent".into() };
    assert_eq!(render(&e, &PLAIN), "REFUSED cargo: verify budget already spent");
}

#[test]
fn note_plain_is_not_shaped_like_a_verdict() {
    // Never `FAIL`/`REFUSED`-shaped: narration below a gate's own registry-id verdict, not a
    // second verdict for the same gate.
    let e = Event::Note { source: "policy/run.sh".into(), text: "verify budget already spent".into() };
    let out = render(&e, &PLAIN);
    assert_eq!(out, "    note: policy/run.sh: verify budget already spent");
    assert!(!out.contains("FAIL") && !out.contains("REFUSED"), "{out}");
}

#[test]
fn colour_wraps_in_ansi_and_plain_does_not() {
    let e = Event::StageFinished { stage: "Verify".into(), outcome: Outcome::Pass, elapsed: Duration::ZERO };
    let coloured = render(&e, &COLOR);
    let plain = render(&e, &PLAIN);
    assert_ne!(coloured, plain);
    assert!(coloured.contains("\x1b["), "coloured render should carry an ANSI escape: {coloured}");
    assert!(!plain.contains('\x1b'), "plain render must carry no ANSI bytes: {plain:?}");
}
