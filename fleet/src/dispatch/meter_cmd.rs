//! `fleet meter`: parse -> `fleet_govern::{admit,settle}` against a real `FileMeterStore` ->
//! print. The actual admission/settlement arithmetic is `fleet-govern`'s; this only wires args
//! to it and prints the result.

use crate::cli::args_core::MeterArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human;
use fleet_govern::FileMeterStore;
use fleet_types::{LaneId, Tokens};
use std::path::Path;

pub fn meter(state_dir: &Path, args: MeterArgs) -> Result<(), DispatchError> {
    let store = FileMeterStore::new(state_dir.join("meter.json"));
    let lane = LaneId::parse(args.lane.clone()).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let reservation = fleet_govern::admit(&store, &lane, Tokens::new(args.cost_est))?;
    human::line("reservation", reservation.id.get());

    if args.settle {
        fleet_govern::settle(&store, reservation, Tokens::new(args.cost_est))?;
        human::ok("settled");
    }
    Ok(())
}
