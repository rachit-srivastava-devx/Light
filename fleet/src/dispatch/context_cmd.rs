//! `fleet graph|impact`: walk the repo into `fleet_context::SourceFile`s, call
//! `fleet_context::build_repo_map`, and print. Parsing/symbol-graph/PageRank logic is entirely
//! `fleet-context`'s; this only reads files off disk (a real, named IO touch this outermost
//! layer is allowed -- BLUEPRINT §4).

use crate::cli::args_ctx::{GraphArgs, ImpactArgs};
use crate::dispatch::error::DispatchError;
use crate::print::human;
use fleet_context::{build_repo_map, language_for, SourceFile};
use std::path::Path;

fn read_source_files(repo: &Path) -> Vec<SourceFile> {
    let mut out = Vec::new();
    collect(repo, repo, &mut out);
    out
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<SourceFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) != Some("target") {
                collect(root, &path, out);
            }
        } else if let Some(language) = language_for(&path) {
            if let Ok(source) = std::fs::read_to_string(&path) {
                let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                out.push(SourceFile { path: rel, language, source });
            }
        }
    }
}

pub fn graph(args: GraphArgs) -> Result<(), DispatchError> {
    let files = read_source_files(Path::new(&args.repo));
    let map = build_repo_map(&files).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    human::line("symbols", map.symbols.len());
    human::line("edges", map.edges.len());
    Ok(())
}

pub fn impact(args: ImpactArgs) -> Result<(), DispatchError> {
    let files = read_source_files(Path::new("."));
    let map = build_repo_map(&files).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let hits = map.symbols.iter().filter(|s| s.name == args.symbol).count();
    human::line("matching_symbols", hits);
    Ok(())
}
