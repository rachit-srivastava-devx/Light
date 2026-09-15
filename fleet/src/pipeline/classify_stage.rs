//! `Classify`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Turns the
//! `--task` text into a `route::TaskClass` (the one piece of decision logic this
//! composition layer legitimately owns -- `fleet-router` takes a `TaskClass` as input, it does
//! not derive one from free text itself) and runs it through the real `route::decide`
//! path so the result is an auditable `Decision`, not a discarded `Ok(())`.

use print::human_stream::emit;
use print::render_event::Event;
use print::style::Style;
use route::{Decision, TaskClass};

/// `HumanOnly` keywords mirror the A15/D4 doctrine already named in this repo's `CLAUDE.md`:
/// contracts, migrations, and money moves are human-merge always. Anything else that names a
/// concrete code change is `Implementation`; an empty/vague task falls back to `General`.
fn task_class(task_text: &str) -> TaskClass {
    let lower = task_text.to_lowercase();
    let human_only = [
        "contract",
        "migration",
        "money",
        "payment",
        "credential",
        "secret",
    ];
    if human_only.iter().any(|k| lower.contains(k)) {
        TaskClass::HumanOnly
    } else if lower.trim().is_empty() {
        TaskClass::General
    } else {
        TaskClass::Implementation
    }
}

/// Classification never fails the pipeline on its own -- `decide`'s `refusal` field (no capable
/// adapter, no role assigned yet) is part of the returned `Decision`, not an error here; `Dispatch`
/// is the stage that actually needs a selected adapter and owns failing on that.
pub fn classify(task_text: &str, runtime: &route::RuntimeState) -> Decision {
    let class = task_class(task_text);
    let decision = route::decide(None, class, None, runtime);
    let text = match &decision.refusal {
        Some(r) => format!(
            "task_class={class:?} -- refused at stage {} ({}): {}",
            r.stage, r.stage_name, r.reason
        ),
        None => format!(
            "task_class={class:?} adapter={:?} resolved_model={:?}",
            decision.selected_adapter, decision.resolved_model
        ),
    };
    emit(
        &Event::Note {
            source: "classify".into(),
            text,
        },
        &Style::detect(),
    );
    decision
}
