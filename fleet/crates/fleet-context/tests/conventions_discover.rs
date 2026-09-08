//! §9 integration tests for the `conventions` capability: nearest-wins precedence, missing docs
//! as a normal empty result, budget trimming with a recorded trim, and unicode round-tripping.
//! All IO goes through `StdConventionFs` over a real `tempfile::tempdir()` tree -- never the repo.

use fleet_context::{discover_conventions, fold_conventions, StdConventionFs, TokenModel};
use fleet_types::Tokens;
use std::fs;
use tempfile::tempdir;

#[test]
fn nearest_wins_over_ancestor() {
    let root = tempdir().unwrap();
    let work = root.path().join("apps/web");
    fs::create_dir_all(&work).unwrap();
    fs::write(root.path().join("AGENTS.md"), "root rules").unwrap();
    fs::write(root.path().join("apps/AGENTS.md"), "layer rules").unwrap();
    fs::write(work.join("AGENTS.md"), "unit rules").unwrap();

    let set = discover_conventions(&StdConventionFs, root.path(), &work).unwrap();
    let agents: Vec<_> = set.docs.iter().filter(|d| d.path.ends_with("AGENTS.md")).collect();
    assert_eq!(agents.len(), 3);
    // Sorted nearest-first: the unit doc's precedence beats the layer's, which beats the root's.
    assert_eq!(agents[0].content, "unit rules");
    assert!(agents[0].precedence > agents[1].precedence);
    assert!(agents[1].precedence > agents[2].precedence);
}

#[test]
fn missing_convention_files_are_a_normal_empty_result() {
    let root = tempdir().unwrap();
    fs::create_dir_all(root.path().join("apps/web")).unwrap();
    let work = root.path().join("apps/web");

    let set = discover_conventions(&StdConventionFs, root.path(), &work).unwrap();
    assert!(set.is_empty());
}

#[test]
fn huge_doc_is_trimmed_to_budget_and_recorded() {
    let root = tempdir().unwrap();
    fs::create_dir_all(root.path().join("unit")).unwrap();
    fs::write(root.path().join("AGENTS.md"), "word ".repeat(2000)).unwrap();
    fs::write(root.path().join("unit/AGENTS.md"), "short unit rule").unwrap();
    let work = root.path().join("unit");

    let set = discover_conventions(&StdConventionFs, root.path(), &work).unwrap();
    let fold = fold_conventions(&set, Tokens::new(20), TokenModel::Cl100kBase).unwrap();

    assert!(fold.tokens_used.get() <= fold.tokens_budget.get());
    assert!(fold.docs.iter().any(|d| d.content == "short unit rule"));
    assert!(fold.trimmed.iter().any(|d| d.path == "AGENTS.md"));
}

#[test]
fn unicode_content_round_trips() {
    let root = tempdir().unwrap();
    let content = "конвенции проекта — 规约 — 🚀";
    fs::write(root.path().join("AGENTS.md"), content).unwrap();

    let set = discover_conventions(&StdConventionFs, root.path(), root.path()).unwrap();
    assert_eq!(set.docs.len(), 1);
    assert_eq!(set.docs[0].content, content);
}
