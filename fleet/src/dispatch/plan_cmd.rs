//! `fleet sow|plan`: parse -> `fleet_plan::validate_sow_text` (structural section checks) ->
//! `fleet_scan::assess` (content-level ambiguity probing over the raw text, wired via
//! `sow_probes`'s stub ports/runner -- §`sow_probes.rs`) -> `fleet_plan::assemble_*` -> print.
//! The structural check alone cannot distinguish a placeholder from a real spec (both lack the
//! same section headings); `assess` is what actually reads what the text says.

use crate::cli::args_core::{PlanArgs, SowArgs};
use crate::dispatch::error::DispatchError;
use crate::dispatch::memory::{record_accepted_sow, RealMemory};
use crate::dispatch::sow_probes::{SequentialRunner, StubCodebase, StubResearch};
use crate::print::human;
use fleet_scan::{assess, Assessment, BusinessProbe, MemoryProbe, ProbeSet, RequirementInput, ResearchProbe, TechnicalProbe};
use std::path::Path;

pub fn sow(state_dir: &Path, args: SowArgs) -> Result<(), DispatchError> {
    let mut messages: Vec<String> = fleet_plan::validate_sow_text(&args.text, &args.intent_hash)
        .into_iter()
        .map(|v| v.0)
        .collect();

    let (codebase, memory, research) = (StubCodebase, RealMemory::new(state_dir), StubResearch);
    let probes = ProbeSet {
        business: &BusinessProbe,
        technical: &TechnicalProbe { codebase: &codebase },
        memory: &MemoryProbe { memory: &memory },
        research: &ResearchProbe { research: &research },
    };
    let input = RequirementInput { text: args.text.clone(), task_id: None };
    if let Assessment::Open(open) = assess(&input, &probes, &SequentialRunner).result {
        for q in open.into_vec() {
            messages.push(format!("ambiguity ({:?}): {} -- {}", q.probe, q.text, q.why));
        }
    }

    if messages.is_empty() {
        // ONLY an accepted SOW is remembered. Writing refused submissions here poisoned the
        // next run: the corrected draft is textually similar to the rejected one, so `MemoryProbe`
        // flagged it as "a prior decision" and refused it a second time for colliding with the
        // caller's own rejected draft. Best-effort -- a memory-store fault must never turn an
        // otherwise-valid SOW into a refusal.
        if let Err(e) = record_accepted_sow(state_dir, &args.text) {
            eprintln!("note: sow memory not recorded: {e}");
        }
        human::ok("sow valid");
        Ok(())
    } else {
        for m in &messages {
            human::refused(m);
        }
        Err(DispatchError::Refusal(format!(
            "{} sow violation(s)\nsee fleet/docs/USING-FLEET.md#sow for a working template",
            messages.len()
        )))
    }
}

pub fn plan(args: PlanArgs) -> Result<(), DispatchError> {
    let doc = fleet_plan::assemble_acceptance_checks_draft(&args.model);
    println!("{doc}");
    Ok(())
}
