//! The `conventions` capability: discover a repo's `AGENTS.md`/`CLAUDE.md` layering, PR/issue
//! templates, and `CONTRIBUTING.md`, then fold them into this crate's existing token-budget
//! compactor. IO is injected via `ConventionFs`; this module never reads an ambient path.

mod discover;
mod fold;
mod fold_types;
mod fs_port;
mod templates;
mod types;

pub use discover::discover_conventions;
pub use fold::fold_conventions;
pub use fold_types::{ConventionFold, PlacedConventionDoc, TrimmedConventionDoc};
pub use fs_port::{ConventionFs, StdConventionFs};
pub use types::{ConventionDoc, ConventionSet, DocKind};
