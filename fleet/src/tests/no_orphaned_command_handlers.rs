//! Reimplements keel's `tests/corpus/M4.sh` ("a command function with no dispatch arm is dead
//! code that every gate passes over" -- its own comment named `__agent` as the example this
//! catches). The original stopped firing after the `keel/` migration: it pointed at
//! `keel/fleet/src/main.rs` and did `[ -r "$SRC" ] || exit 77`, which SKIPPED silently forever
//! once that path stopped existing, instead of failing (PRINCIPLES: a check cheaper to fake
//! than to satisfy will be faked -- and a check nobody watches fail is worse: it fakes itself).
//! This version reads the real, current sources and panics loudly if it cannot find them --
//! never skips. Homed in `src/tests/` rather than `fleet-verify` because what it checks --
//! `src/dispatch/*_cmd.rs` modules vs. `src/cli/root.rs`'s `Commands` enum -- is intrinsic to
//! this package's own module layout, not a generic cross-repo gate `fleet-verify` owns. Parsing
//! helpers live in `support/m4.rs`; the variant-side check lives in
//! `no_orphaned_commands_variants.rs` (≤80-line split).

mod support;

use support::m4::{command_module_names, read_or_fail_loudly};

#[test]
fn every_cmd_module_is_called_from_the_dispatch_match() {
    let mod_rs = read_or_fail_loudly("dispatch/mod.rs");
    let modules = command_module_names(&mod_rs);
    let dead: Vec<&String> =
        modules.iter().filter(|m| !mod_rs.contains(&format!("{m}::"))).collect();
    assert!(dead.is_empty(), "dead command module(s), never called from dispatch::run: {dead:?}");
}
