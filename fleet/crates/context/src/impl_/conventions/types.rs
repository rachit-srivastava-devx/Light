//! Shared types for the `conventions` capability: discovered convention docs and the result of
//! folding them into a token budget. See `discover.rs`/`fold.rs`.

/// What kind of convention document this is -- lets a caller render each differently (e.g. a PR
/// template goes into a PR body, an `AGENTS.md` goes into a system prompt).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DocKind {
    AgentsMd,
    ClaudeMd,
    Contributing,
    PrTemplate,
    IssueTemplate,
}

/// One loaded convention document. `precedence` records WHY this doc applies: for `AgentsMd`/
/// `ClaudeMd`, higher means nearer the working directory (root = 0, each directory level below it
/// +1); `Contributing`/`PrTemplate`/`IssueTemplate` are not part of that layering and always carry
/// precedence `0`.
#[derive(Clone, Debug)]
pub struct ConventionDoc {
    /// Repo-relative, forward-slash-normalized path.
    pub path: String,
    pub kind: DocKind,
    pub precedence: u32,
    pub content: String,
}

/// Every convention doc discovered under a repo root for one working directory, nearest-first.
#[derive(Clone, Debug, Default)]
pub struct ConventionSet {
    /// Sorted by `precedence` descending (nearest/most-specific first), so a caller folding this
    /// into a token budget can take a simple prefix and get "nearest wins" for free.
    pub docs: Vec<ConventionDoc>,
}

impl ConventionSet {
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }
}
