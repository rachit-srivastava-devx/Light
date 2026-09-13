mod io_fault;
mod lock;

// `graph`/`kv`/`memory`/`retention`: KEEP-BUT-UNWIRED scaffolding matching `docs/LLD/LLD.md`
// §10/§11 (`DELTA.md` D10, timed P2/P3) -- no caller yet, so `#[allow(dead_code)]` here instead
// of at every item; `ledger` is the live, wired component and stays ungated.
#[allow(dead_code, unused_imports)]
pub mod graph;
#[allow(dead_code, unused_imports)]
pub mod kv;
pub mod ledger;
#[allow(dead_code, unused_imports)]
pub mod memory;
#[allow(dead_code, unused_imports)]
pub mod retention;

#[allow(unused_imports)]
pub use graph::GraphStore;
#[allow(unused_imports)]
pub use kv::KvStore;
pub use ledger::Ledger;
#[allow(unused_imports)]
pub use memory::MemoryStore;
#[allow(unused_imports)]
pub use retention::{PruneReport, RetentionError, RetentionPolicy, UsageReport};
