//! The four concrete `Adapter` implementations -- one file per ingress source.

mod cli;
mod fs_watch;
mod github;
mod github_map;
mod gmail;

pub use cli::{CliAdapter, CliAdapterError};
pub use fs_watch::{FsAdapter, FsAdapterError};
pub use github::{GithubAdapter, GithubAdapterError};
pub use gmail::{GmailAdapter, GmailAdapterError};
