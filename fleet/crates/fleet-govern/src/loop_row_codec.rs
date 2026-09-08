//! Row codec for `FileLoopStore`'s one-line-per-plan format: `plan_id\tunit\x1funit\x1f...`
//! (units joined by `\x1f`, the ASCII unit separator, since a unit id may itself contain a tab).
//! Split out of `loop_store_file.rs` purely to hold the 80-line file cap.

use crate::loop_types::{LoopProgress, UnitId};

pub(crate) const SEP: char = '\u{1f}';

pub(crate) fn parse_row(line: &str) -> Option<(String, LoopProgress)> {
    let mut f = line.splitn(2, '\t');
    let id = f.next()?.to_string();
    let units = f.next().unwrap_or("");
    let completed = if units.is_empty() {
        Vec::new()
    } else {
        units.split(SEP).map(UnitId::new).collect()
    };
    Some((id, LoopProgress { completed }))
}

pub(crate) fn encode_row(id: &str, progress: &LoopProgress) -> String {
    let units: Vec<&str> = progress.completed.iter().map(UnitId::as_str).collect();
    format!("{id}\t{}\n", units.join(&SEP.to_string()))
}
