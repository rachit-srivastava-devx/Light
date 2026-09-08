//! Parsing helpers for the M4 dead-command-handler checks (`no_orphaned_command_handlers*.rs`).
//! Split out of the test files themselves to keep each ≤80 lines.

use std::fs;
use std::path::PathBuf;

pub fn read_or_fail_loudly(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {path:?}: {e} -- this check measures nothing if it cannot find its \
             own sources, so it must fail here rather than silently pass (never `exit 77`)"
        )
    })
}

/// Module names declared `[pub] mod <name>_cmd;` in `dispatch/mod.rs` -- the actual command
/// handler modules, distinct from support modules like `agent_cmd_error`/`verify_ports`.
pub fn command_module_names(mod_rs: &str) -> Vec<String> {
    let mut names: Vec<String> = mod_rs
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("pub mod ").or_else(|| line.strip_prefix("mod "))?;
            let name = rest.strip_suffix(';')?;
            name.ends_with("_cmd").then(|| name.to_string())
        })
        .collect();
    assert!(!names.is_empty(), "found no `*_cmd` module declarations -- measuring nothing");
    names.sort();
    names.dedup();
    names
}

/// `Commands` enum variant names declared in `root.rs`, in declaration order. Tracks `{`/`}`
/// depth line by line so a struct-shaped variant's own fields (e.g. `Skills { check: bool }`)
/// are never mistaken for sibling top-level variants.
pub fn commands_variants(root_rs: &str) -> Vec<String> {
    let enum_src = &root_rs[root_rs.find("pub enum Commands {").expect("Commands enum present")..];
    let mut depth = 0i32;
    let mut names = Vec::new();
    for line in enum_src.lines() {
        let trimmed = line.trim();
        if depth == 1 && trimmed.chars().next().is_some_and(char::is_uppercase) {
            if let Some(name) = trimmed.split(['(', ' ', '{', ',']).next() {
                names.push(name.to_string());
            }
        }
        depth += trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
        if depth <= 0 && !names.is_empty() {
            break;
        }
    }
    assert!(!names.is_empty(), "found no `Commands` variants -- measuring nothing");
    names
}

/// Whether `Commands::<name>` appears in `haystack` as a whole variant reference -- not merely
/// as a prefix of a longer variant's name (`"Commands::Agent"` is a substring of
/// `"Commands::Agents(a)"`, so a plain `str::contains` would wrongly call `Agent` covered).
pub fn mentions_variant(haystack: &str, name: &str) -> bool {
    let needle = format!("Commands::{name}");
    haystack.match_indices(&needle).any(|(i, _)| {
        haystack[i + needle.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_')
    })
}
