//! `Classify`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Turns the
//! `--task` text into a `fleet_router::TaskClass` (the one piece of decision logic this
//! composition layer legitimately owns -- `fleet-router` takes a `TaskClass` as input, it does
//! not derive one from free text itself) and runs it through the real `fleet_router::decide`
//! path so the result is an auditable `Decision`, not a discarded `Ok(())`.

use fleet_router::{Decision, TaskClass};

/// `HumanOnly` keywords mirror the A15/D4 doctrine already named in this repo's `CLAUDE.md`:
/// contracts, migrations, and money moves are human-merge always. Anything else that names a
/// concrete code change is `Implementation`; an empty/vague task falls back to `General`.
fn task_class(task_text: &str) -> TaskClass {
    let lower = task_text.to_lowercase();
    let human_only = ["contract", "migration", "money", "payment", "credential", "secret"];
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
pub fn classify(task_text: &str, runtime: &fleet_router::RuntimeState) -> Decision {
    fleet_router::decide(None, task_class(task_text), None, runtime)
}
