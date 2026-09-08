//! The `Commands`-variant half of the M4 reimplementation (see `no_orphaned_command_handlers.rs`
//! for the full rationale) -- split into its own file to keep both ≤80 lines.

mod support;

use support::m4::{commands_variants, mentions_variant, read_or_fail_loudly};

#[test]
fn every_commands_variant_is_named_somewhere_in_the_dispatch_match() {
    let root_rs = read_or_fail_loudly("cli/root.rs");
    let mod_rs = read_or_fail_loudly("dispatch/mod.rs");
    let orphaned: Vec<String> = commands_variants(&root_rs)
        .into_iter()
        .filter(|v| !mentions_variant(&mod_rs, v))
        .collect();
    assert!(orphaned.is_empty(), "Commands variant(s) with no dispatch arm at all: {orphaned:?}");
}
