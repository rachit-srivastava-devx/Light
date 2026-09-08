//! `guard()`'s full BLUEPRINT.md §6 behavior table.

use fleet_events::{guard, EventEnvelope, EventId, EventKind, SourceKind, MAX_QUOTE_BYTES};
use serde_json::{json, Value};

fn envelope(kind: EventKind, payload: Value) -> EventEnvelope {
    let id = EventId::derive(SourceKind::Cli, kind, "ext");
    EventEnvelope::new(id, SourceKind::Cli, kind, "2026-01-01T00:00:00Z".to_string(), payload)
}

const ALL_KINDS: [EventKind; 9] = [
    EventKind::GithubPullRequestOpened,
    EventKind::GithubPullRequestUpdated,
    EventKind::GithubIssueComment,
    EventKind::GithubPush,
    EventKind::GmailMessageReceived,
    EventKind::FsFileCreated,
    EventKind::FsFileModified,
    EventKind::FsFileDeleted,
    EventKind::CliInvoked,
];

#[test]
fn guard_quote_is_verbatim_and_capped() {
    let small = envelope(EventKind::CliInvoked, json!({"a": 1}));
    let action = guard(&small);
    assert_eq!(action.quote, serde_json::to_string(&small.payload).unwrap());

    let huge_string = "x".repeat(MAX_QUOTE_BYTES * 2);
    let huge = envelope(EventKind::CliInvoked, json!(huge_string));
    let action = guard(&huge);
    assert_eq!(action.quote.len(), MAX_QUOTE_BYTES + "…[truncated]".len());
    assert!(action.quote.is_char_boundary(action.quote.len()));
    assert!(std::str::from_utf8(action.quote.as_bytes()).is_ok());
}

#[test]
fn guard_requires_confirmation_matches_side_effecting_exactly() {
    for kind in ALL_KINDS {
        let env = envelope(kind, json!({"x": "y"}));
        let action = guard(&env);
        assert_eq!(action.requires_confirmation, kind.side_effecting());
    }
}

#[test]
fn guard_handles_empty_and_null_payload_without_panicking() {
    let empty = envelope(EventKind::CliInvoked, json!({}));
    let action = guard(&empty);
    assert_eq!(action.quote, "{}");

    let null = envelope(EventKind::CliInvoked, Value::Null);
    let action = guard(&null);
    assert_eq!(action.quote, "null");
}

#[test]
fn guard_is_pure_same_input_same_output() {
    let env = envelope(EventKind::GithubIssueComment, json!({"body": "hello"}));
    let first = guard(&env);
    let second = guard(&env);
    assert_eq!(first.quote, second.quote);
    assert_eq!(first.requires_confirmation, second.requires_confirmation);
}

#[test]
fn guard_quote_is_byte_for_byte_unicode_passthrough() {
    let env = envelope(EventKind::CliInvoked, json!({"text": "héllo \u{202e}world"}));
    let action = guard(&env);
    assert!(action.quote.contains("héllo"));
    assert!(action.quote.contains('\u{202e}'));
}
