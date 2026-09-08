//! `fleet graph|impact`: walk the repo into `fleet_context::SourceFile`s (bounded -- see
//! `walk.rs`, which skips build/VCS/vendor dirs and enforces a file/byte/deadline budget so a
//! real multi-hundred-MB repo finishes instead of hanging), call `fleet_context::build_repo_map`,
//! and print. Parsing/symbol-graph/PageRank logic is entirely `fleet-context`'s; this only reads
//! files off disk (a real, named IO touch this outermost layer is allowed -- BLUEPRINT §4).

use crate::cli::args_ctx::{GraphArgs, ImpactArgs};
use crate::dispatch::error::DispatchError;
use crate::dispatch::walk::read_source_files_bounded;
use crate::print::human;
use fleet_context::build_repo_map;
use std::path::Path;

#[derive(serde::Serialize)]
struct GraphReport {
    files_scanned: usize,
    symbols: usize,
    edges: usize,
}

pub fn graph(args: GraphArgs) -> Result<(), DispatchError> {
    let files = read_source_files_bounded(Path::new(&args.repo))?;
    let map = build_repo_map(&files).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let report = GraphReport { files_scanned: files.len(), symbols: map.symbols.len(), edges: map.edges.len() };
    if args.json {
        crate::print::json::print_pretty(&report);
    } else {
        human::line("files_scanned", report.files_scanned);
        human::line("symbols", report.symbols);
        human::line("edges", report.edges);
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct ImpactReport {
    matching_symbols: usize,
}

pub fn impact(args: ImpactArgs) -> Result<(), DispatchError> {
    let files = read_source_files_bounded(Path::new(&args.repo))?;
    let map = build_repo_map(&files).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let hits = map.symbols.iter().filter(|s| s.name == args.symbol).count();
    if args.json {
        crate::print::json::print_pretty(&ImpactReport { matching_symbols: hits });
    } else {
        human::line("matching_symbols", hits);
    }
    Ok(())
}
