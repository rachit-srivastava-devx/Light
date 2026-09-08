//! `fleet mcp`: BLUEPRINT §5 cites `fleet_worker::sandbox::manifest::manifest_for_lease`, but
//! that module is private to `fleet-worker` (`mod sandbox;`) and not re-exported from `lib.rs` --
//! only whole-provision `resolve_hermetic_provision` is public, and it takes an `agent_id`, not
//! a bare lease expression. Flagged rather than silently faked: this subcommand parses but
//! refuses with a typed, named gap until the owning crate exposes the narrower fn.

use crate::cli::args_ctx::McpArgs;
use crate::dispatch::error::DispatchError;

pub fn mcp(_args: McpArgs) -> Result<(), DispatchError> {
    Err(DispatchError::NotYetImplemented(
        "fleet-worker::sandbox::manifest::manifest_for_lease is not pub from lib.rs",
    ))
}
