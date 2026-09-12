//! Plain substring digit-extraction helpers shared by every `parse_denominator` fn in
//! `parsers.rs`. No regex dependency needed for any marker in the committed registry.

/// Digits immediately following `key` (e.g. `"caught="` -> the `40` in `"caught=40"`).
pub(crate) fn after(text: &str, key: &str) -> Option<u64> {
    let after = text.split_once(key)?.1;
    after
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

/// Digits immediately preceding `marker` (e.g. the `5` in `"5 files scanned"`).
pub(crate) fn before(text: &str, marker: &str) -> Option<u64> {
    let prefix = &text[..text.find(marker)?];
    prefix
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .parse()
        .ok()
}
