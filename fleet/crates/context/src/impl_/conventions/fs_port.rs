//! `ConventionFs` -- the injected IO port for `discover_conventions`. This crate never touches
//! `std::env`/an ambient cwd; every path this module reads is caller-supplied. `StdConventionFs`
//! is a real adapter over `std::fs`, mirroring `TantivyIndex::open`'s `IndexLocation::Path`
//! precedent elsewhere in this crate -- tests use their own in-memory impl over a tempdir instead.

use std::fs;
use std::path::Path;

/// A filesystem seam for convention discovery: read one file (missing is `None`, not an error)
/// and list one directory's entries (missing/non-dir is `[]`, not an error).
pub trait ConventionFs {
    fn read_file(&self, path: &Path) -> Option<String>;
    fn list_dir(&self, path: &Path) -> Vec<String>;
}

/// Real `std::fs`-backed adapter. Every path passed to it is caller-supplied (via
/// `discover_conventions`'s `repo_root`/`work_dir` and the fixed relative sub-paths this module
/// computes from them) -- this struct never reads an ambient path on its own.
pub struct StdConventionFs;

impl ConventionFs for StdConventionFs {
    fn read_file(&self, path: &Path) -> Option<String> {
        fs::read_to_string(path).ok()
    }

    fn list_dir(&self, path: &Path) -> Vec<String> {
        let Ok(entries) = fs::read_dir(path) else {
            return Vec::new();
        };
        entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect()
    }
}
