use ingest::{normalize, IncomingEvent, IngestError, SourceRegistration};
use serde_json::json;

fn reg(ns: &str) -> SourceRegistration { SourceRegistration { namespace: ns.into(), schema_version: 1 } }
fn ev(src: &str, payload: serde_json::Value) -> IncomingEvent {
    IncomingEvent { source: src.into(), delivery_id: "d1".into(), payload, attachments: vec![] }
}

#[test] fn unknown_source_refuses() {
    assert_eq!(normalize(ev("unregistered-ns", json!({})), &reg("github")).unwrap_err(), IngestError::UnknownSource);
}

#[test] fn unicode_homograph_source_refused() {
    assert_eq!(normalize(ev("\u{0261}itHub", json!({})), &reg("github")).unwrap_err(), IngestError::UnknownSource);
}

#[test] fn whitespace_source_normalized() {
    let r = normalize(ev("  github  ", json!({})), &reg("github")).unwrap();
    assert_eq!(r.source, "github");
}

#[test] fn oversized_source_refused() {
    let big = "a".repeat(257);
    assert_eq!(normalize(ev(&big, json!({})), &reg(&big)).unwrap_err(), IngestError::UnknownSource);
}

#[test] fn nfc_normalized_source_matches_nfd_input() {
    // NFD "café" must NFC-normalize to match registered NFC "café" (U+00E9).
    let r = normalize(
        IncomingEvent { source: "caf\u{0065}\u{0301}".into(), delivery_id: "d1".into(),
                        payload: json!({}), attachments: vec![] },
        &reg("caf\u{00e9}"),
    );
    assert!(r.is_ok(), "NFD input must match NFC namespace: {r:?}");
    assert_eq!(r.unwrap().source, "caf\u{00e9}");
}

#[test] fn oversized_payload_refused() {
    let big = "x".repeat(270_000); // 270_002 bytes > MAX_PAYLOAD_BYTES (262_144)
    assert_eq!(normalize(ev("github", json!(big)), &reg("github")).unwrap_err(), IngestError::OversizedPayload);
}
