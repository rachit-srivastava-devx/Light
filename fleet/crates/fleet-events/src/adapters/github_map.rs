//! Maps a GitHub REST events-API `type` field to our closed `EventKind` vocabulary. Split out of
//! `github.rs` to keep both files under the 80-line gate.

use crate::event_kind::EventKind;

pub(super) fn map_kind(type_name: &str) -> Option<EventKind> {
    match type_name {
        "PullRequestEvent" => Some(EventKind::GithubPullRequestOpened),
        "PullRequestReviewEvent" => Some(EventKind::GithubPullRequestUpdated),
        "IssueCommentEvent" => Some(EventKind::GithubIssueComment),
        "PushEvent" => Some(EventKind::GithubPush),
        _ => None,
    }
}
