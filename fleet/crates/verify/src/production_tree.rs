#[path = "production_file_hash.rs"]
pub(crate) mod file_hash;
#[path = "production_tree_walk.rs"]
mod walk;

use super::acceptance::spec_hash::field;
use std::fs;
use std::path::Path;

pub(super) const MAX_FILES: usize = 4_096;
const MAX_BYTES_PER_FILE: u64 = 1_048_576;
const MAX_CONTENT_BYTES: u64 = 67_108_864;

pub fn tree_digest_for_repo(repo: &Path) -> String {
    let text = repo.to_string_lossy();
    if let Some(commit) = std::process::Command::new("git")
        .args(["-C", text.as_ref(), "rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return commit;
    }
    content_digest(repo)
}

/// Hash a directory's bounded, sorted contents without consulting its Git identity. This is
/// used for resolved gate assets because an override may itself be a Git checkout whose HEAD can
/// remain unchanged while a gate script or policy file changes.
pub(super) fn content_digest(root: &Path) -> String {
    let mut files = Vec::new();
    walk::collect_files(root, root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = blake3::Hasher::new();
    field(&mut hasher, b"fleet.tree.no-git.v1");
    field(&mut hasher, &(files.len() as u64).to_le_bytes());
    let mut content_bytes = 0u64;
    for (relative, path) in files {
        field(&mut hasher, relative.as_bytes());
        let size = match fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                field(&mut hasher, format!("metadata-error:{error}").as_bytes());
                continue;
            }
        };
        field(&mut hasher, &size.to_le_bytes());
        if content_bytes < MAX_CONTENT_BYTES {
            let limit = MAX_BYTES_PER_FILE.min(MAX_CONTENT_BYTES - content_bytes);
            match walk::read_bounded(&path, limit) {
                Ok(bytes) => {
                    content_bytes = content_bytes.saturating_add(bytes.len() as u64);
                    field(&mut hasher, &bytes);
                    field(
                        &mut hasher,
                        if size > limit {
                            &b"truncated"[..]
                        } else {
                            &b"complete"[..]
                        },
                    );
                }
                Err(error) => field(&mut hasher, format!("read-error:{error}").as_bytes()),
            }
        } else {
            field(&mut hasher, b"content-budget-exhausted");
        }
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}
