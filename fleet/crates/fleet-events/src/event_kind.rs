//! `EventKind` -- the typed, closed vocabulary the kernel dispatches on.

/// See `lib.rs` for the injection-proofing argument this type exists to enforce: `kind` is chosen
/// by trusted adapter code from protocol-level metadata only, never derived by parsing `payload`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    GithubPullRequestOpened,
    GithubPullRequestUpdated,
    GithubIssueComment,
    GithubPush,
    GmailMessageReceived,
    FsFileCreated,
    FsFileModified,
    FsFileDeleted,
    CliInvoked,
}

impl EventKind {
    pub(crate) fn wire_tag(self) -> &'static str {
        match self {
            EventKind::GithubPullRequestOpened => "github_pull_request_opened",
            EventKind::GithubPullRequestUpdated => "github_pull_request_updated",
            EventKind::GithubIssueComment => "github_issue_comment",
            EventKind::GithubPush => "github_push",
            EventKind::GmailMessageReceived => "gmail_message_received",
            EventKind::FsFileCreated => "fs_file_created",
            EventKind::FsFileModified => "fs_file_modified",
            EventKind::FsFileDeleted => "fs_file_deleted",
            EventKind::CliInvoked => "cli_invoked",
        }
    }

    /// Fixed per variant, never payload-derived -- see the injection-proofing invariant this
    /// crate exists to enforce (BLUEPRINT.md §4). No wildcard arm: adding a variant without
    /// deciding this is a compile error, not a silent default.
    pub fn side_effecting(self) -> bool {
        match self {
            EventKind::GithubPullRequestOpened => true,
            EventKind::GithubPullRequestUpdated => true,
            EventKind::GithubIssueComment => true,
            EventKind::GithubPush => true,
            EventKind::GmailMessageReceived => true,
            EventKind::FsFileCreated => true,
            EventKind::FsFileModified => true,
            EventKind::FsFileDeleted => true,
            EventKind::CliInvoked => true,
        }
    }
}
