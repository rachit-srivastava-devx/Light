//! `sow`'s memory wiring, split into small files to keep each comfortably under the ≤80-line
//! rule (see `../plan_cmd.rs` for how these compose): `embed` (pure text -> vector), `store`
//! (JSON persistence), `ports` (search over loaded items), `clock` (the one real clock read),
//! `adapter` (read side: `fleet_scan::MemoryPort`), `write` (write side: dedup-on-write).

mod adapter;
mod clock;
mod embed;
mod ports;
mod store;
mod write;

pub use adapter::RealMemory;
pub use write::record_sow_refusal;
