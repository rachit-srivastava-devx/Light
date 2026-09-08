//! `EventId` determinism and collision-avoidance -- BLUEPRINT.md §9.

use fleet_events::{EventId, EventKind, SourceKind};

#[test]
fn same_external_event_yields_same_id() {
    let a = EventId::derive(SourceKind::GithubWebhook, EventKind::GithubPush, "delivery-1");
    let b = EventId::derive(SourceKind::GithubWebhook, EventKind::GithubPush, "delivery-1");
    assert_eq!(a, b);
}

#[test]
fn distinct_sources_never_collide_on_shared_external_id() {
    let a = EventId::derive(SourceKind::GithubWebhook, EventKind::GithubPush, "shared-id");
    let b = EventId::derive(SourceKind::Gmail, EventKind::GmailMessageReceived, "shared-id");
    assert_ne!(a, b);
}

#[test]
fn distinct_kinds_never_collide_on_shared_external_id_and_source() {
    let a = EventId::derive(SourceKind::GithubWebhook, EventKind::GithubPush, "shared-id");
    let b = EventId::derive(SourceKind::GithubWebhook, EventKind::GithubPullRequestOpened, "shared-id");
    assert_ne!(a, b);
}

#[test]
fn empty_external_id_still_derives_deterministically() {
    let a = EventId::derive(SourceKind::Cli, EventKind::CliInvoked, "");
    let b = EventId::derive(SourceKind::Cli, EventKind::CliInvoked, "");
    assert_eq!(a, b);
}
