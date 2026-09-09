//! S3 (`docs/USER-JOURNEY-2.md`): `main` calls `add` only as `println!("{}", add(2, 3))`, and
//! `test_add`/`test_subtract` call `add`/`subtract` only inside `assert_eq!(...)` -- every call
//! site is nested inside a macro invocation's argument list, which tree-sitter-rust parses as an
//! opaque `token_tree`, not as `call_expression` nodes. Before the macro-body scan, this fixture
//! hand-counted to 3 edges but `build_repo_map` reported 0. Pinned to the exact hand count: 3.
//! Split out of `repomap_fixtures.rs` to keep both files under the 80-line gate.

use fleet_context::{build_repo_map, Language, SourceFile};

#[test]
fn macro_nested_callers_fixture_matches_hand_count() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/macro-nested-callers.rs");
    let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("missing fixture {path}: {e}"));
    let files = [SourceFile { path: "macro-nested-callers.rs".into(), language: Language::Rust, source }];
    let map = build_repo_map(&files).expect("valid rust source parses");
    assert_eq!(map.edges.len(), 3, "hand count: main->add, test_add->add, test_subtract->subtract");
}
