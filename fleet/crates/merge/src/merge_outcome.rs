/// What a successful, verified merge actually moved -- measured after the fact from real git
/// output, never assumed from a `git merge` exit code alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeOutcome {
    pub branch: String,
    /// Files staged inside the worktree before the lane's own commit (`merge-lane.sh:11`).
    pub staged_files: usize,
    /// Files differing between pre-/post-merge `HEAD` in the target repo (`merge-lane.sh:21`).
    pub changed_files: usize,
    /// Target repo's `HEAD` sha before the merge (`merge-lane.sh:17`).
    pub before: String,
    /// Target repo's `HEAD` sha after the merge (`merge-lane.sh:19`).
    pub after: String,
}
