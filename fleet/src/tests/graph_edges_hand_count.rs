//! S3 (`docs/USER-JOURNEY-2.md`): `fleet graph --repo <r>` always reported `edges: 0`, even for a
//! repo where a hand count says call edges exist. Root cause: every call site in the journey's
//! repro was made *inside* a macro invocation (`println!`, `assert_eq!`) -- tree-sitter-rust
//! parses macro arguments as an opaque `token_tree`, not as `call_expression` nodes, so the
//! existing extractor (which only matches `call_expression`) never saw them. Fixed in
//! `fleet-context`'s `macro_body_call` (best-effort scan of a `token_tree`'s named children for
//! `<name> <"(...)">`). This drives the real binary against the exact repro shape and asserts the
//! reported count matches a hand count.

use std::fs;
mod support;
use support::cmd;

#[test]
fn graph_edges_match_hand_count_for_macro_nested_calls() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(
        root.path().join("src/main.rs"),
        "fn add(a: i32, b: i32) -> i32 { a + b }\n\
         fn subtract(a: i32, b: i32) -> i32 { a - b }\n\
         fn main() { println!(\"{}\", add(2, 3)); }\n\
         #[test]\n\
         fn test_add() { assert_eq!(add(2, 3), 5); }\n\
         #[test]\n\
         fn test_subtract() { assert_eq!(subtract(5, 3), 2); }\n",
    )
    .unwrap();

    let out = cmd().args(["graph", "--repo", root.path().to_str().unwrap(), "--json"]).output().expect("runs");
    assert!(out.status.success(), "graph failed: {}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    // Hand count: main->add (println!), test_add->add (assert_eq!), test_subtract->subtract
    // (assert_eq!) = 3. Symbols: add, subtract, main, test_add, test_subtract = 5.
    assert_eq!(v["symbols"], 5);
    assert_eq!(v["edges"], 3, "expected 3 hand-counted call edges, got {v}");
}
