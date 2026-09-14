//! `fleet route`: parse -> `route::decide` -> print. `roles`/`role-check` live in
//! `role_cmd.rs` (≤80-line split).
//!
//! `--role verifier` with no `--builder-model` deliberately refuses at stage 5 ("verifier
//! independence"): a bare route query has no builder context, so it cannot know whether the
//! picked verifier would share the (unknown) builder's model -- see `verify_gate.rs`'s own doc
//! comment ("`None` builder model clears all candidates -- it does not skip the stage"). This is
//! not a bug to silently default around; pass `--builder-model` to get a real answer instead.

use cli::args_core::RouteArgs;
use crate::dispatch::error::DispatchError;
use crate::dispatch::runtime_snapshot::healthy_runtime;
use print::human;
use types::Role;

#[derive(serde::Serialize)]
struct RouteReport {
    selected_adapter: String,
}

pub fn route(args: RouteArgs) -> Result<(), DispatchError> {
    let json = args.json;
    let role = args
        .role
        .as_deref()
        .map(Role::parse)
        .transpose()
        .ok()
        .flatten();
    let runtime = healthy_runtime();
    let decision = route::decide(
        role,
        route::TaskClass::General,
        args.builder_model.as_deref(),
        &runtime,
    );
    match decision.refusal {
        None => {
            let adapter = format!("{:?}", decision.selected_adapter);
            if json {
                print::json::print_pretty(&RouteReport {
                    selected_adapter: adapter,
                });
            } else {
                human::ok(format!("selected {adapter}"));
            }
            Ok(())
        }
        Some(r) => Err(DispatchError::Router(r)),
    }
}
