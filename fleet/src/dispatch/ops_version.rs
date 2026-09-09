//! `fleet version`: crate version + build identity (S2, `docs/USER-JOURNEY-2.md`). Split out of
//! `ops_cmd.rs` to keep it under the 80-line gate.

use crate::dispatch::error::DispatchError;
use crate::print::human;

pub fn version(json: bool) -> Result<(), DispatchError> {
    let id = crate::build_info::IDENTITY;
    if json {
        crate::print::json::print_pretty(&id);
    } else {
        human::line("fleet-cli", id.version);
        human::line("commit_sha", id.commit_sha);
        human::line("tree_state", id.tree_state);
        human::line("build_time", id.build_time);
    }
    Ok(())
}
