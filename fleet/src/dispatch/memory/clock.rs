//! `now()` -- the one real clock read behind `sow`'s memory wiring. Deliberately impure and
//! deliberately NOT inside `fleet-memory` (that crate never reads an ambient clock): this plays
//! the same "real adapter" role `fleet-context::StdConventionFs` plays for filesystem access.

use fleet_memory::Timestamp;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> Timestamp {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    Timestamp::from_unix_secs(secs)
}
