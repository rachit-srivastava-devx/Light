//! Typed refusal reasons for `apply::apply`. Every variant is honest about exactly what was
//! wrong -- never a generic "could not apply" -- so a refusal tells the operator (or a test)
//! precisely which fence and which rule it tripped.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    /// The reply had no fenced code block at all -- prose, not a change. Mirrors the predecessor
    /// `has_fence` check (`git show HEAD:fleet/keel/fleet/src/main.rs:3403`).
    #[error("freelane apply: reply contained no fenced code block to apply")]
    NoFence,

    /// A fence's target file could not be determined from either the fence's own info string
    /// (` ```path:<file> `) or a `path:`/`file:` line immediately above it. Refusing rather than
    /// guessing a filename -- see module doc on why a default target is never invented.
    #[error(
        "freelane apply: fence #{index} has no declared target (use ```path:<file> or a \
         `path:`/`file:` line immediately above the fence) -- refusing rather than guessing a \
         filename"
    )]
    AmbiguousTarget { index: usize },

    /// The model handed back an absolute path -- untrusted input, treated as hostile.
    #[error("freelane apply: fence #{index} names an absolute path {path:?} -- refusing")]
    AbsolutePath { index: usize, path: String },

    /// The declared path has a `..` (or other non-normal) component.
    #[error("freelane apply: fence #{index} path {path:?} contains a `..` component -- refusing")]
    PathTraversal { index: usize, path: String },

    /// The resolved, canonicalised target (or a symlinked ancestor of it) falls outside the
    /// worktree -- covers both `..` disguised via a symlink and a pre-existing symlinked target.
    #[error("freelane apply: fence #{index} target {path:?} resolves outside the worktree -- refusing")]
    EscapesWorktree { index: usize, path: String },

    /// The worktree root itself could not be canonicalised -- should not happen for a real lane,
    /// but this is untrusted-input handling, not an assumption.
    #[error("freelane apply: could not resolve the worktree root: {0}")]
    WorktreeUnresolvable(String),

    /// A real IO failure while writing a validated target.
    #[error("freelane apply: failed to write {path}: {source}", path = path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
