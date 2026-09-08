//! Hand-rolled TSV codec for the ledger file, mirroring `meter.rs`'s "empty field means unknown,
//! never zero" rule. Row-level (dis)assembly lives in `row_codec.rs` to hold the 80-line rule.

use std::collections::BTreeMap;

use crate::row_codec::{decode_row, encode_row};
use crate::types::{LaneState, MeterIoError};

const HEADER: &str = "fleet-govern-meter-v1";

pub(crate) fn encode_lanes(lanes: &BTreeMap<String, LaneState>) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for (name, state) in lanes {
        out.push_str(&encode_row(name, state));
        out.push('\n');
    }
    out
}

pub(crate) fn decode_lanes(text: &str) -> Result<BTreeMap<String, LaneState>, MeterIoError> {
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default();
    if !header.is_empty() && header != HEADER {
        return Err(MeterIoError(format!("unsupported meter state header {header:?}")));
    }
    let mut lanes = BTreeMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6 {
            return Err(MeterIoError(format!("invalid meter state line: {line:?}")));
        }
        lanes.insert(fields[0].to_string(), decode_row(&fields)?);
    }
    Ok(lanes)
}
