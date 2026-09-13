//! `fleet meter`: parse -> `control::{admit,settle}` against a real `FileMeterStore` ->
//! print. The actual admission/settlement arithmetic is `fleet-govern`'s; this only wires args
//! to it and prints the result.

use crate::cli::args_core::MeterArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human;
use control::FileMeterStore;
use std::path::Path;
use types::{LaneId, Tokens};

#[derive(serde::Serialize)]
struct MeterReport {
    reservation: String,
    settled: bool,
}

pub fn meter(state_dir: &Path, args: MeterArgs) -> Result<(), DispatchError> {
    let store = FileMeterStore::new(state_dir.join("meter.json"));
    let lane =
        LaneId::parse(args.lane.clone()).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let reservation = control::admit(&store, &lane, Tokens::new(args.cost_est))?;
    let reservation_id = reservation.id.get().to_string();

    let settled = if args.settle {
        control::settle(&store, reservation, Tokens::new(args.cost_est))?;
        true
    } else {
        false
    };

    if args.json {
        crate::print::json::print_pretty(&MeterReport {
            reservation: reservation_id,
            settled,
        });
    } else {
        human::line("reservation", &reservation_id);
        if settled {
            human::ok("settled");
        }
    }
    Ok(())
}
