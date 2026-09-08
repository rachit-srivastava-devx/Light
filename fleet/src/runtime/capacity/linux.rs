//! Linux `CapacityProbe` backend: `/proc/meminfo`'s `MemAvailable` (kernel's own estimate of
//! "usable without swapping", not raw free) and `/proc/loadavg`'s first field.

use super::measurement::{Measurement, ProbeError};
use std::fs;

pub fn measure(logical_cores: usize) -> Result<Measurement, ProbeError> {
    let meminfo = read("/proc/meminfo")?;
    let total_memory_bytes = extract_meminfo_kb(&meminfo, "MemTotal")? * 1024;
    let available_memory_bytes = extract_meminfo_kb(&meminfo, "MemAvailable")? * 1024;
    let load_avg_1m = parse_loadavg(&read("/proc/loadavg")?)?;
    Ok(Measurement { total_memory_bytes, available_memory_bytes, load_avg_1m, logical_cores })
}

fn read(path: &'static str) -> Result<String, ProbeError> {
    fs::read_to_string(path).map_err(|e| ProbeError::SourceUnavailable { resource: path, detail: e.to_string() })
}

fn extract_meminfo_kb(text: &str, key: &'static str) -> Result<u64, ProbeError> {
    let line = text
        .lines()
        .find(|l| l.starts_with(key))
        .ok_or_else(|| parse_err("/proc/meminfo", key, text))?;
    line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).ok_or_else(|| parse_err("/proc/meminfo", key, line))
}

fn parse_loadavg(text: &str) -> Result<f64, ProbeError> {
    text.split_whitespace()
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| parse_err("/proc/loadavg", "load1", text))
}

fn parse_err(resource: &'static str, field: &'static str, raw: &str) -> ProbeError {
    ProbeError::ParseFailed { resource, field, detail: format!("unexpected content: {raw:?}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo_and_loadavg_shapes() {
        let mem = "MemTotal:       16777216 kB\nMemAvailable:    8388608 kB\n";
        assert_eq!(extract_meminfo_kb(mem, "MemAvailable").unwrap(), 8388608);
        assert!((parse_loadavg("1.50 1.20 0.90 2/456 12345\n").unwrap() - 1.50).abs() < 1e-9);
    }
}
