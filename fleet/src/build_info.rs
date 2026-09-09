//! Build identity (S2, `docs/USER-JOURNEY-2.md`): the git sha, dirty marker, build timestamp,
//! and crate version this binary was compiled from. All captured at COMPILE time by `build.rs`
//! via `cargo:rustc-env` and read here with `env!` -- never a runtime `git` shell-out, which
//! would report the CWD's repo instead of the build's (an installed binary runs far from any
//! checkout). Surfaced by `fleet version` and `fleet doctor`, human and `--json` alike.

/// Everything a build report needs, gathered once so `version`/`doctor` share one source.
#[derive(serde::Serialize, Clone)]
pub struct BuildIdentity {
    pub version: &'static str,
    /// Short commit sha the binary was built from, or `"unknown"` if `git` was unavailable or
    /// the source tree wasn't a checkout at build time -- never a fabricated sha.
    pub commit_sha: &'static str,
    /// `"dirty"` if the working tree had uncommitted changes at build time, `"clean"` if not,
    /// `"unknown"` if git dirtiness could not be determined (folded into the sha's own unknown).
    pub tree_state: &'static str,
    /// RFC3339 UTC build timestamp, e.g. `2026-09-09T07:30:00Z`.
    pub build_time: &'static str,
}

pub const IDENTITY: BuildIdentity = BuildIdentity {
    version: env!("CARGO_PKG_VERSION"),
    commit_sha: env!("FLEET_BUILD_SHA"),
    tree_state: env!("FLEET_BUILD_DIRTY"),
    build_time: env!("FLEET_BUILD_TIME"),
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins that every identity field is compiled in and non-empty -- catches the field
    /// silently going back to `unknown`/empty if `build.rs` regresses.
    #[test]
    fn identity_fields_present_and_non_empty() {
        assert!(!IDENTITY.version.is_empty());
        assert!(!IDENTITY.commit_sha.is_empty());
        assert!(matches!(IDENTITY.tree_state, "clean" | "dirty" | "unknown"));
        assert!(!IDENTITY.build_time.is_empty());
        assert!(IDENTITY.build_time.contains('T') && IDENTITY.build_time.ends_with('Z'));
    }
}
