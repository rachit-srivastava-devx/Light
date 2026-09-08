//! One lane's TSV row: `lane\twindow\tused\treservations\tresolved_model\tunknown_observed`.
//! Split out of `meter_codec.rs` to hold the 80-line-per-file rule.

use fleet_types::Tokens;

use crate::reservation_codec::parse_reservation;
use crate::types::{LaneState, MeterIoError};

pub(crate) fn encode_row(name: &str, state: &LaneState) -> String {
    let window = state.window.map(|t| t.get().to_string()).unwrap_or_default();
    let used = state.used.map(|t| t.get().to_string()).unwrap_or_default();
    let reservations = state
        .reservations
        .iter()
        .map(|r| format!("{}:{}", r.id.get(), r.estimated.get()))
        .collect::<Vec<_>>()
        .join(",");
    let model = state.resolved_model.clone().unwrap_or_default();
    let unknown = if state.unknown_observed { "1" } else { "0" };
    format!("{name}\t{window}\t{used}\t{reservations}\t{model}\t{unknown}")
}

fn parse_tokens(s: &str) -> Result<Option<Tokens>, MeterIoError> {
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<u64>()
        .map(Tokens::new)
        .map(Some)
        .map_err(|e| MeterIoError(format!("bad token count {s:?}: {e}")))
}

pub(crate) fn decode_row(fields: &[&str]) -> Result<LaneState, MeterIoError> {
    let window = parse_tokens(fields[1])?;
    let used = parse_tokens(fields[2])?;
    let reservations = if fields[3].is_empty() {
        Vec::new()
    } else {
        fields[3]
            .split(',')
            .map(|entry| parse_reservation(fields[0], entry))
            .collect::<Result<Vec<_>, MeterIoError>>()?
    };
    let resolved_model = if fields[4].is_empty() { None } else { Some(fields[4].to_string()) };
    let unknown_observed = fields[5] == "1";
    Ok(LaneState { window, used, reservations, resolved_model, unknown_observed })
}
