//! `discover_conventions`'s template/CONTRIBUTING side (`impl_/conventions/templates.rs`) had
//! zero test coverage before this suite -- goal item 7 ("templates fleet will enforce on the
//! repos it's working on") rests entirely on this code path actually finding what's on disk.
//! Real `StdConventionFs` over real `tempfile` dirs throughout; no fake filesystem.

use context::{discover_conventions, DocKind, StdConventionFs};
use std::path::Path;
use tempfile::tempdir;

fn kinds(root: &Path, work: &Path) -> Vec<(String, DocKind)> {
    discover_conventions(&StdConventionFs, root, work)
        .expect("work_dir under root")
        .docs
        .into_iter()
        .map(|d| (d.path, d.kind))
        .collect()
}

#[test]
fn a_bare_repo_discovers_nothing() {
    let root = tempdir().expect("tempdir");
    assert!(kinds(root.path(), root.path()).is_empty());
}

/// The enforcement half of goal item 7: a caller that wants to *require* templates (a future
/// gate, not built yet -- see `docs/LLD/LLD-META-L8-ADDENDUM-4.md`) needs this exact signal --
/// "no PrTemplate/Contributing doc kind present" -- to refuse. Pins that the absence is real
/// absence (empty result), not a silent partial miss, so a gate built on top can trust it.
#[test]
fn a_repo_missing_templates_is_unambiguously_detectable_for_enforcement() {
    let root = tempdir().expect("tempdir");
    std::fs::write(root.path().join("README.md"), "# a repo with no conventions").unwrap();
    let found = kinds(root.path(), root.path());
    let has_pr_template = found.iter().any(|(_, k)| *k == DocKind::PrTemplate);
    let has_contributing = found.iter().any(|(_, k)| *k == DocKind::Contributing);
    assert!(
        !has_pr_template && !has_contributing,
        "expected no enforceable template docs in a bare repo, got: {found:?}"
    );
}

#[test]
fn contributing_md_is_discovered_at_precedence_zero() {
    let root = tempdir().expect("tempdir");
    std::fs::write(root.path().join("CONTRIBUTING.md"), "how to contribute").unwrap();
    let set = discover_conventions(&StdConventionFs, root.path(), root.path()).unwrap();
    assert_eq!(set.docs.len(), 1);
    assert_eq!(set.docs[0].kind, DocKind::Contributing);
    assert_eq!(set.docs[0].precedence, 0);
    assert_eq!(set.docs[0].content, "how to contribute");
}

#[test]
fn single_file_pr_template_is_discovered() {
    let root = tempdir().expect("tempdir");
    std::fs::create_dir_all(root.path().join(".github")).unwrap();
    std::fs::write(
        root.path().join(".github/PULL_REQUEST_TEMPLATE.md"),
        "## Summary",
    )
    .unwrap();
    let found = kinds(root.path(), root.path());
    assert_eq!(
        found,
        vec![(
            ".github/PULL_REQUEST_TEMPLATE.md".to_string(),
            DocKind::PrTemplate
        )]
    );
}

#[test]
fn pr_template_directory_entries_are_discovered_sorted_by_name() {
    let root = tempdir().expect("tempdir");
    let dir = root.path().join(".github/PULL_REQUEST_TEMPLATE");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("feature.md"), "feature body").unwrap();
    std::fs::write(dir.join("bugfix.md"), "bugfix body").unwrap();
    let found = kinds(root.path(), root.path());
    assert_eq!(
        found,
        vec![
            (
                ".github/PULL_REQUEST_TEMPLATE/bugfix.md".to_string(),
                DocKind::PrTemplate
            ),
            (
                ".github/PULL_REQUEST_TEMPLATE/feature.md".to_string(),
                DocKind::PrTemplate
            ),
        ]
    );
}

#[test]
fn issue_template_directory_is_discovered_as_its_own_kind() {
    let root = tempdir().expect("tempdir");
    let dir = root.path().join(".github/ISSUE_TEMPLATE");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bug_report.md"), "bug body").unwrap();
    let found = kinds(root.path(), root.path());
    assert_eq!(
        found,
        vec![(
            ".github/ISSUE_TEMPLATE/bug_report.md".to_string(),
            DocKind::IssueTemplate
        )]
    );
}

/// The single-file template and the directory form are independent seams in the source --
/// nothing stops both existing on disk at once (a repo mid-migration between the two GitHub
/// conventions), so both must surface rather than one silently shadowing the other.
#[test]
fn single_file_and_directory_pr_templates_can_coexist() {
    let root = tempdir().expect("tempdir");
    std::fs::create_dir_all(root.path().join(".github/PULL_REQUEST_TEMPLATE")).unwrap();
    std::fs::write(root.path().join(".github/PULL_REQUEST_TEMPLATE.md"), "root").unwrap();
    std::fs::write(
        root.path().join(".github/PULL_REQUEST_TEMPLATE/extra.md"),
        "extra",
    )
    .unwrap();
    let found = kinds(root.path(), root.path());
    assert_eq!(found.len(), 2, "both forms must surface: {found:?}");
    assert!(found
        .iter()
        .all(|(_, k)| *k == DocKind::PrTemplate));
}

/// Non-`.md` files and unrelated files in `.github/` are not templates.
#[test]
fn unrelated_github_files_are_ignored() {
    let root = tempdir().expect("tempdir");
    std::fs::create_dir_all(root.path().join(".github/workflows")).unwrap();
    std::fs::write(root.path().join(".github/workflows/ci.yml"), "name: ci").unwrap();
    std::fs::write(root.path().join(".github/dependabot.yml"), "version: 2").unwrap();
    assert!(kinds(root.path(), root.path()).is_empty());
}

/// Templates carry precedence 0 -- the same value as the repo-root `AGENTS.md`/`CLAUDE.md` --
/// and the tie-break is alphabetical by path, so `.github/...` (a `.` byte) sorts before an
/// upper-case `AGENTS.md`/`CLAUDE.md`. This is a real, non-obvious consequence of the sort key,
/// not a hypothetical: pin it so a future reordering of that tie-break is a deliberate choice.
#[test]
fn templates_tie_break_alphabetically_against_root_level_docs() {
    let root = tempdir().expect("tempdir");
    std::fs::write(root.path().join("AGENTS.md"), "agents").unwrap();
    std::fs::create_dir_all(root.path().join(".github")).unwrap();
    std::fs::write(root.path().join(".github/PULL_REQUEST_TEMPLATE.md"), "pr").unwrap();
    let found = kinds(root.path(), root.path());
    assert_eq!(
        found,
        vec![
            (".github/PULL_REQUEST_TEMPLATE.md".to_string(), DocKind::PrTemplate),
            ("AGENTS.md".to_string(), DocKind::AgentsMd),
        ]
    );
}

/// End-to-end, not synthetic: this fleet repo's own `.github/PULL_REQUEST_TEMPLATE.md` (added for
/// goal item 7 -- "templates fleet will enforce") must actually be discoverable by fleet's own
/// mechanism, not just by a tempdir fixture that assumes the same shape.
#[test]
fn fleet_repos_own_pr_template_is_really_discovered() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/context -> repo root");
    let found = kinds(repo_root, repo_root);
    assert!(
        found.contains(&(
            ".github/PULL_REQUEST_TEMPLATE.md".to_string(),
            DocKind::PrTemplate
        )),
        "expected this repo's own PR template to be discovered: {found:?}"
    );
}

/// The full, realistic shape: a nested `AGENTS.md` layering chain (nearest-wins) plus templates,
/// folded into one discovery call -- the actual seam goal item 7's "fleet will enforce" rests on.
#[test]
fn nested_agents_md_layering_and_templates_combine_nearest_wins() {
    let root = tempdir().expect("tempdir");
    std::fs::write(root.path().join("AGENTS.md"), "root rules").unwrap();
    std::fs::create_dir_all(root.path().join("crates/approval")).unwrap();
    std::fs::write(root.path().join("crates/AGENTS.md"), "crates rules").unwrap();
    std::fs::write(
        root.path().join("crates/approval/AGENTS.md"),
        "approval rules",
    )
    .unwrap();
    std::fs::create_dir_all(root.path().join(".github")).unwrap();
    std::fs::write(root.path().join(".github/PULL_REQUEST_TEMPLATE.md"), "pr").unwrap();

    let work = root.path().join("crates/approval");
    let found = kinds(root.path(), &work);
    assert_eq!(
        found,
        vec![
            ("crates/approval/AGENTS.md".to_string(), DocKind::AgentsMd),
            ("crates/AGENTS.md".to_string(), DocKind::AgentsMd),
            (".github/PULL_REQUEST_TEMPLATE.md".to_string(), DocKind::PrTemplate),
            ("AGENTS.md".to_string(), DocKind::AgentsMd),
        ],
        "nearest AGENTS.md must sort first, root-precedence docs tie-break by path: {found:?}"
    );
}
