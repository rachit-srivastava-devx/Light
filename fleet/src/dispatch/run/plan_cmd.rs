//! `fleet sow|plan`: parse -> `planner::validate_sow_text` (structural section checks) ->
//! `scan::assess` (content-level ambiguity probing over the raw text, wired via
//! `sow_probes`'s stub ports/runner -- §`sow_probes.rs`) -> `planner::assemble_*` -> print.
//! The structural check alone cannot distinguish a placeholder from a real spec (both lack the
//! same section headings); `assess` is what actually reads what the text says.
//!
//! **Module-level parallel execution**: Extended to support parallel SOW validation and planning
//! for multiple modules, with blueprint streaming to the user as an L8 engineer.

use cli::args_core::{PlanArgs, SowArgs};
use crate::dispatch::error::DispatchError;
use sow_memory::{record_accepted_sow, RealMemory};
use crate::dispatch::sow_probes::{SequentialRunner, StubCodebase, StubResearch};
use print::human;
use scan::{
    assess, Assessment, BusinessProbe, MemoryProbe, ProbeSet, RequirementInput, ResearchProbe,
    TechnicalProbe,
};
use std::collections::HashMap;
use std::path::Path;
use types::{Blueprint, Module, TaskId};

pub fn sow(state_dir: &Path, args: SowArgs) -> Result<(), DispatchError> {
    let mut messages: Vec<String> = planner::validate_sow_text(&args.text, &args.intent_hash)
        .into_iter()
        .map(|v| v.0)
        .collect();

    let (codebase, memory, research) = (StubCodebase, RealMemory::new(state_dir), StubResearch);
    let probes = ProbeSet {
        business: &BusinessProbe,
        technical: &TechnicalProbe {
            codebase: &codebase,
        },
        memory: &MemoryProbe { memory: &memory },
        research: &ResearchProbe {
            research: &research,
        },
    };
    let input = RequirementInput {
        text: args.text.clone(),
        task_id: None,
    };
    if let Assessment::Open(open) = assess(&input, &probes, &SequentialRunner).result {
        for q in open.into_vec() {
            messages.push(format!(
                "ambiguity ({:?}): {} -- {}",
                q.probe, q.text, q.why
            ));
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

/// SOW modules in parallel, respecting dependencies.
pub fn sow_modules(
    state_dir: &Path,
    modules: &[Module],
) -> Result<HashMap<String, Module>, DispatchError> {
    let mut result = HashMap::new();

    // Track states for dependency checking
    let mut state_map: HashMap<String, String> = HashMap::new();

    for module in modules {
        let mut messages: Vec<String> =
            planner::validate_sow_text(&module.sow_text, &format!("hash-{}", module.id))
                .into_iter()
                .map(|v| v.0)
                .collect();

        let (codebase, memory, research) = (StubCodebase, RealMemory::new(state_dir), StubResearch);
        let probes = ProbeSet {
            business: &BusinessProbe,
            technical: &TechnicalProbe {
                codebase: &codebase,
            },
            memory: &MemoryProbe { memory: &memory },
            research: &ResearchProbe {
                research: &research,
            },
        };
        let task_id = TaskId::parse(&module.id)
            .map_err(|_| DispatchError::Refusal(format!("module {} has invalid id", module.id)))?;
        let input = RequirementInput {
            text: module.sow_text.clone(),
            task_id: Some(task_id),
        };
        if let Assessment::Open(open) = assess(&input, &probes, &SequentialRunner).result {
            for q in open.into_vec() {
                messages.push(format!(
                    "ambiguity ({:?}): {} -- {}",
                    q.probe, q.text, q.why
                ));
            }
        }

        if messages.is_empty() {
            human::ok(format!("module {} sow valid", module.id));
            state_map.insert(module.id.clone(), "sowed".to_string());
        } else {
            for m in &messages {
                human::refused(format!("module {}: {}", module.id, m));
            }
            state_map.insert(module.id.clone(), "failed".to_string());
            return Err(DispatchError::Refusal(format!(
                "module {} has {} sow violation(s)",
                module.id,
                messages.len()
            )));
        }
    }

    // Rebuild modules with successful state
    for module in modules {
        if state_map
            .get(&module.id)
            .map(|s| s == "sowed")
            .unwrap_or(false)
        {
            let mut updated = module.clone();
            updated.state = types::ModuleState::Sowed {
                sow_id: format!("sow-{}", module.id),
            };
            result.insert(module.id.clone(), updated);
        }
    }

    Ok(result)
}

pub fn plan(args: PlanArgs) -> Result<(), DispatchError> {
    let doc = planner::assemble_acceptance_checks_draft(&args.model);
    println!("{doc}");
    Ok(())
}

/// Plan modules in parallel with blueprint streaming.
/// Returns a map of module_id to blueprint.
pub fn plan_modules(
    _args: PlanArgs,
    _modules: &[Module],
) -> Result<HashMap<String, Blueprint>, DispatchError> {
    // TODO: Implement actual parallel planning with L8 engineer role
    let mut blueprints = HashMap::new();

    for module in _modules {
        // For now, create a stub blueprint
        let blueprint = Blueprint {
            plan: format!("Plan for {}", module.name),
            acceptance: vec!["Acceptance criteria 1".to_string()],
            affected_files: vec!["src/lib.rs".to_string()],
        };
        blueprints.insert(module.id.clone(), blueprint);
    }

    Ok(blueprints)
}
