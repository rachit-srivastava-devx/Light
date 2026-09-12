//! Non-layered convention docs: PR/issue templates and `CONTRIBUTING.md`. These are not part of
//! the `AGENTS.md`/`CLAUDE.md` nearest-wins chain (§ discover.rs), so they always carry
//! precedence `0` -- present or absent, never overridden by a deeper directory.

use super::discover::relative_slash_path;
use super::fs_port::ConventionFs;
use super::types::{ConventionDoc, DocKind};
use std::path::Path;

pub fn discover_templates(fs: &dyn ConventionFs, repo_root: &Path) -> Vec<ConventionDoc> {
    let mut docs = Vec::new();

    if let Some(content) = fs.read_file(&repo_root.join("CONTRIBUTING.md")) {
        docs.push(doc("CONTRIBUTING.md", DocKind::Contributing, content));
    }

    let pr_single = repo_root.join(".github/PULL_REQUEST_TEMPLATE.md");
    if let Some(content) = fs.read_file(&pr_single) {
        docs.push(doc(
            ".github/PULL_REQUEST_TEMPLATE.md",
            DocKind::PrTemplate,
            content,
        ));
    }
    docs.extend(dir_docs(
        fs,
        repo_root,
        ".github/PULL_REQUEST_TEMPLATE",
        DocKind::PrTemplate,
    ));
    docs.extend(dir_docs(
        fs,
        repo_root,
        ".github/ISSUE_TEMPLATE",
        DocKind::IssueTemplate,
    ));
    docs
}

fn dir_docs(
    fs: &dyn ConventionFs,
    repo_root: &Path,
    rel_dir: &str,
    kind: DocKind,
) -> Vec<ConventionDoc> {
    let dir = repo_root.join(rel_dir);
    let mut names = fs.list_dir(&dir);
    names.sort();
    names
        .into_iter()
        .filter_map(|name| {
            let full = dir.join(&name);
            let content = fs.read_file(&full)?;
            Some(doc(&relative_slash_path(repo_root, &full), kind, content))
        })
        .collect()
}

fn doc(rel_path: &str, kind: DocKind, content: String) -> ConventionDoc {
    ConventionDoc {
        path: rel_path.to_string(),
        kind,
        precedence: 0,
        content,
    }
}
