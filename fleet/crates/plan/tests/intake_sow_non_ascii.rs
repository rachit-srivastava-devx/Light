//! Regression: `has_line_prefix_ci` sliced by BYTE index (`l[..prefix.len()]`), so any SOW line
//! whose first `prefix.len()` bytes ended inside a multi-byte char aborted the whole process --
//! `end byte index 8 is not a char boundary; it is inside '—'`. A non-ASCII SOW must produce a
//! normal verdict, never a panic. Every body below carries an em-dash, curly quotes, an accented
//! character and an emoji, so the crash is reachable from more than one line shape.

use fleet_plan::validate_sow_text;

/// The exact shape that crashed: a heading whose 8th byte falls inside U+2014.
const EM_DASH_HEADING: &str = "# SOW — Shopify customer order history";

fn non_ascii_sow() -> String {
    format!(
        "{EM_DASH_HEADING}\n\
source_intent_hash: abc123\n\
request: constrúir the “thing” — end to end 🚀\n\
## Request restatement\nBuild the “thing” — café edition 🚀\n\
## Built for\nUtilisateurs — the naïve ones 🙂\n\
## Must do\nLivrer le “résumé” — chaque jour\n\
## Explicitly will not do\nNot delete data — out of scope: façturation 🚫\n\
## Done when\n95% des requêtes passent — vérifié\n\
## Acceptance threshold\nexit 0 — p95 < 200ms ✅\n"
    )
}

#[test]
fn a_non_ascii_sow_validates_instead_of_panicking() {
    let v = validate_sow_text(&non_ascii_sow(), "abc123");
    assert!(v.is_empty(), "a well-formed non-ASCII SOW must be clean, got {v:?}");
}

#[test]
fn the_em_dash_heading_alone_does_not_panic() {
    // Shorter than `request:` in chars but not in bytes, and byte 8 is mid-em-dash.
    let v = validate_sow_text(EM_DASH_HEADING, "abc123");
    assert!(v.iter().any(|e| e.0.contains("request:")), "expected the request: violation, got {v:?}");
}

#[test]
fn a_line_whose_prefix_boundary_splits_every_multibyte_kind_does_not_panic() {
    for line in ["reque—st: x", "reques“t: x", "requesé: x", "reques🚀t: x", "—", "🚀", "é", "“"] {
        let _ = validate_sow_text(line, "abc123");
    }
}

#[test]
fn a_non_ascii_request_line_still_satisfies_the_request_check() {
    let v = validate_sow_text(&non_ascii_sow(), "abc123");
    assert!(!v.iter().any(|e| e.0.contains("non-empty request")), "{v:?}");
}
