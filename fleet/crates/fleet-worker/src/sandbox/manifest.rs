//! Lease-scoped MCP tool manifest -- ported from `mcp.rs`'s `Lease`/`ToolManifest`/
//! `manifest_for_lease` (manifest construction only; running the MCP server is out of scope,
//! see BLUEPRINT.md §5's `mcp.rs:148-259` row).

use crate::ProvisionError;
use serde::Serialize;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Serialize, PartialEq)]
struct ManifestTool {
    name: String,
    scope: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct ToolManifest {
    lease: String,
    tools: Vec<ManifestTool>,
}

/// Path-traversal-safe relative-path normalization, ported from `mcp.rs::normalize_relative`.
fn normalize_relative(text: &str) -> Result<PathBuf, ()> {
    let mut out = PathBuf::new();
    for component in Path::new(text).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err(()),
        }
    }
    Ok(out)
}

/// Build the lease-scoped tool manifest for one lane's own worktree prefix
/// (`<prefix>/**`), plus the always-on `impact`/`lesson_recall`/`ledger_read` tools.
pub fn manifest_for_lease(expression: &str) -> Result<serde_json::Value, ProvisionError> {
    let prefix_text = expression
        .strip_suffix("/**")
        .filter(|p| !p.is_empty())
        .ok_or(ProvisionError::MissingFleetFile("lease expression"))?;
    let prefix =
        normalize_relative(prefix_text).map_err(|_| ProvisionError::MissingFleetFile("lease"))?;
    if prefix.as_os_str().is_empty() {
        return Err(ProvisionError::MissingFleetFile("lease"));
    }
    let scope = expression.to_string();
    let manifest = ToolManifest {
        lease: expression.to_string(),
        tools: [
            ("file_read", scope.clone()),
            ("file_write", scope.clone()),
            ("file_list", scope),
            ("impact", "fleet".to_string()),
            ("lesson_recall", "fleet".to_string()),
            ("ledger_read", "fleet".to_string()),
        ]
        .into_iter()
        .map(|(name, scope)| ManifestTool { name: name.to_string(), scope })
        .collect(),
    };
    serde_json::to_value(manifest).map_err(|_| ProvisionError::MissingFleetFile("manifest"))
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
