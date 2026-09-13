//! `fleet route`: parse -> `route::decide` -> print. `roles`/`role-check` live in
//! `role_cmd.rs` (≤80-line split).

use cli::args_core::RouteArgs;
use crate::dispatch::error::DispatchError;
use print::human;
use std::collections::{BTreeMap, BTreeSet};
use types::Role;

fn default_runtime() -> route::RuntimeState {
    route::RuntimeState {
        capable: route::ORDER.iter().map(|c| c.id).collect::<BTreeSet<_>>(),
        remaining: BTreeMap::new(),
        cooldown: BTreeSet::new(),
        required_tokens: 0,
        preference: route::ORDER.iter().map(|c| c.id).collect(),
    }
}

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
    let runtime = default_runtime();
    let decision = route::decide(role, route::TaskClass::General, None, &runtime);
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
