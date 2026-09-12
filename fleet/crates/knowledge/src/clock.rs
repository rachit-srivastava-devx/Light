use std::time::{SystemTime, UNIX_EPOCH};

/// Injectable time source for expiry comparisons.
pub trait Clock: Send + Sync {
    /// Returns the current time as an ISO 8601 UTC string, e.g. `"2026-09-12T00:00:00Z"`.
    fn now_iso(&self) -> String;
}

/// Production clock: reads wall time via std::time::SystemTime.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_iso(&self) -> String {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let s = secs % 60;
        let m = (secs / 60) % 60;
        let h = (secs / 3600) % 24;
        let (y, mo, d) = epoch_secs_to_ymd(secs / 86400);
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
    }
}

fn leap(y: u64) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}

fn epoch_secs_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    loop {
        let dy = if leap(y) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let months = if leap(y) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 1u64;
    for dm in months {
        if days < dm { break; }
        days -= dm;
        mo += 1;
    }
    (y, mo, days + 1)
}

/// Fixed clock for tests.
pub struct FixedClock(pub String);

impl Clock for FixedClock {
    fn now_iso(&self) -> String { self.0.clone() }
}
