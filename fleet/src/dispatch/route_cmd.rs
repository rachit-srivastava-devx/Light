//! `fleet route`: parse -> `fleet_router::decide` -> print. `roles`/`role-check` live in
//! `role_cmd.rs` (≤80-line split).

use crate::cli::args_core::RouteArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human;
use fleet_types::Role;
use std::collections::{BTreeMap, BTreeSet};

fn default_runtime() -> fleet_router::RuntimeState {
    fleet_router::RuntimeState {
        capable: fleet_router::ORDER.iter().map(|c| c.id).collect::<BTreeSet<_>>(),
        remaining: BTreeMap::new(),
        cooldown: BTreeSet::new(),
        required_tokens: 0,
        preference: fleet_router::ORDER.iter().map(|c| c.id).collect(),
    }
}

#[derive(serde::Serialize)]
struct RouteReport {
    selected_adapter: String,
}

pub fn route(args: RouteArgs) -> Result<(), DispatchError> {
    let json = args.json;
    let role = args.role.as_deref().map(Role::parse).transpose().ok().flatten();
    let runtime = default_runtime();
    let decision = fleet_router::decide(role, fleet_router::TaskClass::General, None, &runtime);
    match decision.refusal {
        None => {
            let adapter = format!("{:?}", decision.selected_adapter);
            if json {
                crate::print::json::print_pretty(&RouteReport { selected_adapter: adapter });
            } else {
                human::ok(format!("selected {adapter}"));
            }
            Ok(())
        }
        Some(r) => Err(DispatchError::Router(r)),
    }
}
