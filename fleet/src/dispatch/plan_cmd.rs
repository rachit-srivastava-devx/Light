//! `fleet sow|plan`: parse -> `fleet_plan::{validate_sow_text,assemble_*}` -> print.

use crate::cli::args_core::{PlanArgs, SowArgs};
use crate::dispatch::error::DispatchError;
use crate::print::human;

pub fn sow(args: SowArgs) -> Result<(), DispatchError> {
    let violations = fleet_plan::validate_sow_text(&args.text, &args.intent_hash);
    if violations.is_empty() {
        human::ok("sow valid");
        Ok(())
    } else {
        for v in &violations {
            human::refused(&v.0);
        }
        Err(DispatchError::Refusal(format!("{} sow violation(s)", violations.len())))
    }
}

pub fn plan(args: PlanArgs) -> Result<(), DispatchError> {
    let doc = fleet_plan::assemble_acceptance_checks_draft(&args.model);
    println!("{doc}");
    Ok(())
}
