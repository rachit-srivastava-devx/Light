//! Reuses `fleet/keel/tests/graph-fixtures/*` verbatim (§9 of the blueprint): the tree-sitter
//! extraction ported from `graph.rs` must behave identically on these fixtures.

use fleet_context::{build_repo_map, Language, SourceFile};

const FIXTURES: &str = "../../keel/tests/graph-fixtures";

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}")).expect("fixture file present")
}

#[test]
fn known_callers_fixture_returns_non_zero_edges() {
    let source = read("known-callers.rs");
    let files = [SourceFile {
        path: "known-callers.rs".into(),
        language: Language::Rust,
        source,
    }];
    let map = build_repo_map(&files).expect("valid rust source parses");
    assert!(!map.edges.is_empty(), "expected at least one call edge");
}

#[test]
fn false_positive_control_has_zero_edges() {
    let source = read("false-positive.rs");
    let files = [SourceFile {
        path: "false-positive.rs".into(),
        language: Language::Rust,
        source,
    }];
    let map = build_repo_map(&files).expect("valid rust source parses");
    assert!(
        map.edges.is_empty(),
        "an unindexed external call must never become a graph edge"
    );
}

#[test]
fn all_required_languages_produce_symbols() {
    let py = SourceFile {
        path: "known-python.py".into(),
        language: Language::Python,
        source: read("known-python.py"),
    };
    let bash = SourceFile {
        path: "known-bash.sh".into(),
        language: Language::Bash,
        source: read("known-bash.sh"),
    };
    let py_map = build_repo_map(&[py]).expect("python parses");
    let bash_map = build_repo_map(&[bash]).expect("bash parses");
    assert!(!py_map.symbols.is_empty());
    assert!(!bash_map.symbols.is_empty());
}

#[test]
fn build_repo_map_is_idempotent() {
    let source = read("known-callers.rs");
    let files = [SourceFile {
        path: "known-callers.rs".into(),
        language: Language::Rust,
        source,
    }];
    let first = build_repo_map(&files).unwrap();
    let second = build_repo_map(&files).unwrap();
    let first_ids: Vec<_> = first.symbols.iter().map(|s| s.id.as_str().to_string()).collect();
    let second_ids: Vec<_> = second.symbols.iter().map(|s| s.id.as_str().to_string()).collect();
    assert_eq!(first_ids, second_ids);
}

#[test]
fn empty_files_yields_empty_map() {
    let map = build_repo_map(&[]).unwrap();
    assert!(map.symbols.is_empty());
    assert!(map.edges.is_empty());
    assert!(map.importance.is_empty());
}
