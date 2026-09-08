//! `fleet route|roles|role-check`: parse -> `fleet_router::{decide,evaluate_role_check}` -> print.

use crate::cli::args_core::{RoleCheckArgs, RouteArgs};
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

pub fn route(args: RouteArgs) -> Result<(), DispatchError> {
    let role = args.role.as_deref().map(Role::parse).transpose().ok().flatten();
    let runtime = default_runtime();
    let decision = fleet_router::decide(role, fleet_router::TaskClass::General, None, &runtime);
    match decision.refusal {
        None => {
            human::ok(format!("selected {:?}", decision.selected_adapter));
            Ok(())
        }
        Some(r) => Err(DispatchError::Router(r)),
    }
}

pub fn roles() -> Result<(), DispatchError> {
    for role in Role::ALL {
        human::line(role.name(), format!("bandwidth={} gate={}", role.bandwidth(), role.owned_gate()));
    }
    Ok(())
}

pub fn role_check(args: RoleCheckArgs) -> Result<(), DispatchError> {
    let role = Role::parse(&args.role).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let check = fleet_router::RoleCheck {
        role,
        diff_adds_code: false,
        builder_model: None,
        verifier_model: None,
    };
    fleet_router::evaluate_role_check(&check).map_err(|e| DispatchError::RoleCheck(e.reason()))?;
    human::ok("role check passed");
    Ok(())
}
