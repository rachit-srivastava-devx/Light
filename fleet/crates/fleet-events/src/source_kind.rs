//! `SourceKind` -- which ingress source produced an envelope.

/// Which ingress source produced this envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    GithubWebhook,
    Gmail,
    FsWatch,
    Cli,
}

impl SourceKind {
    /// Stable, hash-input-only tag -- never a display string, never parsed back.
    pub(crate) fn wire_tag(self) -> &'static str {
        match self {
            SourceKind::GithubWebhook => "github_webhook",
            SourceKind::Gmail => "gmail",
            SourceKind::FsWatch => "fs_watch",
            SourceKind::Cli => "cli",
        }
    }
}
