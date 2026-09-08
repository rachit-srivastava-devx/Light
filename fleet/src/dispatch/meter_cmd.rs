//! `fleet meter`: parse -> `fleet_govern::{admit,settle}` against a real `FileMeterStore` ->
//! print. The actual admission/settlement arithmetic is `fleet-govern`'s; this only wires args
//! to it and prints the result.

use crate::cli::args_core::MeterArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human;
use fleet_govern::FileMeterStore;
use fleet_types::{LaneId, Tokens};
use std::path::Path;

#[derive(serde::Serialize)]
struct MeterReport {
    reservation: String,
    settled: bool,
}

pub fn meter(state_dir: &Path, args: MeterArgs) -> Result<(), DispatchError> {
    let store = FileMeterStore::new(state_dir.join("meter.json"));
    let lane = LaneId::parse(args.lane.clone()).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let reservation = fleet_govern::admit(&store, &lane, Tokens::new(args.cost_est))?;
    let reservation_id = reservation.id.get().to_string();

    let settled = if args.settle {
        fleet_govern::settle(&store, reservation, Tokens::new(args.cost_est))?;
        true
    } else {
        false
    };

    if args.json {
        crate::print::json::print_pretty(&MeterReport { reservation: reservation_id, settled });
    } else {
        human::line("reservation", &reservation_id);
        if settled {
            human::ok("settled");
        }
    }
    Ok(())
}
