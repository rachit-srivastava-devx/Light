//! Parsing helpers for `macos.rs`'s `sysctl`/`vm_stat` output. Every failure is a typed
//! `ProbeError::ParseFailed`, never a silent zero.

use super::measurement::ProbeError;

pub fn parse_u64(text: &str, resource: &'static str) -> Result<u64, ProbeError> {
    text.trim().parse().map_err(|e| ProbeError::ParseFailed {
        resource,
        field: "value",
        detail: format!("{e} (raw: {text:?})"),
    })
}

pub fn parse_loadavg(text: &str) -> Result<f64, ProbeError> {
    text.split_whitespace()
        .find(|tok| tok.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .ok_or_else(|| err("sysctl vm.loadavg", "1-minute load", text))?
        .parse()
        .map_err(|_| err("sysctl vm.loadavg", "1-minute load", text))
}

/// `vm_stat`'s header reads `Mach Virtual Memory Statistics: (page size of 16384 bytes)`;
/// available memory is `(free + inactive + speculative) pages * page size`.
pub fn parse_vm_stat_available(text: &str) -> Result<u64, ProbeError> {
    let page_size = extract_page_size(text)?;
    let free = extract_pages(text, "Pages free")?;
    let inactive = extract_pages(text, "Pages inactive")?;
    let speculative = extract_pages(text, "Pages speculative")?;
    Ok((free + inactive + speculative) * page_size)
}

fn extract_page_size(text: &str) -> Result<u64, ProbeError> {
    let marker = "page size of";
    let start = text
        .find(marker)
        .ok_or_else(|| err("vm_stat", "page size", text))?;
    let digits: String = text[start + marker.len()..]
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits
        .parse()
        .map_err(|_| err("vm_stat", "page size", text))
}

fn extract_pages(text: &str, label: &'static str) -> Result<u64, ProbeError> {
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with(label))
        .ok_or_else(|| err("vm_stat", label, text))?;
    let digits: String = line.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse().map_err(|_| err("vm_stat", label, text))
}

fn err(resource: &'static str, field: &'static str, raw: &str) -> ProbeError {
    ProbeError::ParseFailed {
        resource,
        field,
        detail: format!("unexpected output: {raw:?}"),
    }
}

#[cfg(test)]
#[path = "macos_parse_tests.rs"]
mod tests;
