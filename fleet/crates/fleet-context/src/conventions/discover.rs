//! `discover_conventions` -- walks the `AGENTS.md`/`CLAUDE.md` layering chain from `repo_root`
//! down to `work_dir`, nearest-wins by construction (each level's precedence is its depth), then
//! adds the non-layered templates/`CONTRIBUTING.md` via `discover_templates`.

use super::fs_port::ConventionFs;
use super::templates::discover_templates;
use super::types::{ConventionDoc, ConventionSet, DocKind};
use crate::error::ContextError;
use std::path::{Path, PathBuf};

pub fn discover_conventions(
    fs: &dyn ConventionFs,
    repo_root: &Path,
    work_dir: &Path,
) -> Result<ConventionSet, ContextError> {
    let rel = work_dir
        .strip_prefix(repo_root)
        .map_err(|_| ContextError::WorkDirOutsideRoot {
            root: repo_root.display().to_string(),
            work_dir: work_dir.display().to_string(),
        })?;

    let mut docs = Vec::new();
    let mut current = PathBuf::from(repo_root);
    for (rank, component) in std::iter::once(None)
        .chain(rel.components().map(Some))
        .enumerate()
    {
        if let Some(component) = component {
            current.push(component);
        }
        push_layer_doc(fs, &current, repo_root, DocKind::AgentsMd, "AGENTS.md", rank as u32, &mut docs);
        push_layer_doc(fs, &current, repo_root, DocKind::ClaudeMd, "CLAUDE.md", rank as u32, &mut docs);
    }

    docs.extend(discover_templates(fs, repo_root));
    docs.sort_by(|a, b| b.precedence.cmp(&a.precedence).then_with(|| a.path.cmp(&b.path)));
    Ok(ConventionSet { docs })
}

fn push_layer_doc(
    fs: &dyn ConventionFs,
    dir: &Path,
    repo_root: &Path,
    kind: DocKind,
    file_name: &str,
    precedence: u32,
    docs: &mut Vec<ConventionDoc>,
) {
    let full = dir.join(file_name);
    let Some(content) = fs.read_file(&full) else {
        return;
    };
    docs.push(ConventionDoc {
        path: relative_slash_path(repo_root, &full),
        kind,
        precedence,
        content,
    });
}

/// `full`'s path relative to `repo_root`, forward-slash-normalized (matches this crate's existing
/// `SourceFile.path` convention).
pub(super) fn relative_slash_path(repo_root: &Path, full: &Path) -> String {
    let rel = full.strip_prefix(repo_root).unwrap_or(full);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
