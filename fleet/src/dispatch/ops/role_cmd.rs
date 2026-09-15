//! `fleet roles|role-check`: split out of `route_cmd.rs` to keep both files ≤80 lines.

use crate::dispatch::error::DispatchError;
use cli::args_core::RoleCheckArgs;
use print::human;
use types::Role;

#[derive(serde::Serialize)]
struct RoleReport {
    name: &'static str,
    bandwidth: u64,
    owned_gate: &'static str,
}

pub fn roles(json: bool) -> Result<(), DispatchError> {
    if json {
        let report: Vec<RoleReport> = Role::ALL
            .iter()
            .map(|r| RoleReport {
                name: r.name(),
                bandwidth: r.bandwidth(),
                owned_gate: r.owned_gate(),
            })
            .collect();
        print::json::print_pretty(&report);
        return Ok(());
    }
    for role in Role::ALL {
        human::line(
            role.name(),
            format!("bandwidth={} gate={}", role.bandwidth(), role.owned_gate()),
        );
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct RoleCheckReport {
    role: String,
    passed: bool,
}

pub fn role_check(args: RoleCheckArgs) -> Result<(), DispatchError> {
    let role = Role::parse(&args.role).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let check = route::RoleCheck {
        role,
        diff_adds_code: false,
        builder_model: None,
        verifier_model: None,
    };
    route::evaluate_role_check(&check).map_err(|e| DispatchError::RoleCheck(e.reason()))?;
    if args.json {
        print::json::print_pretty(&RoleCheckReport {
            role: args.role,
            passed: true,
        });
    } else {
        human::ok("role check passed");
    }
    Ok(())
}
