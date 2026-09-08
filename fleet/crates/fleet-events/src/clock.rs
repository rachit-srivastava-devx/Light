//! Injected wall-clock source -- no logic in this crate reads the clock ambiently.

use std::time::{SystemTime, UNIX_EPOCH};

/// Injected wall-clock source. Every RFC3339 timestamp any adapter stamps goes through this.
pub trait Clock {
    fn now_rfc3339(&self) -> String;
}

/// The real clock. Constructing and using this is the composition root's job (`src/`).
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_rfc3339(&self) -> String {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        format_rfc3339(secs)
    }
}

/// Hand-rolled RFC3339 (UTC, second precision) formatting from a Unix timestamp -- avoids adding
/// a chrono/time/humantime dependency for one call site. Civil-from-days conversion after Howard
/// Hinnant's `civil_from_days` algorithm (public domain).
fn format_rfc3339(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m_num = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m_num <= 2 { y + 1 } else { y };
    format!("{y:04}-{m_num:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}
