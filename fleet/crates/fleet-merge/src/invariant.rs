//! The D31 gate, factored out as pure fns so it is unit-testable without a real git repo. Each
//! fn mirrors one guard in `fleet/bin/merge-lane.sh` line-for-line.

use crate::error::MergeRefusal;

/// `merge-lane.sh:12-14`'s guard: `staged_files == 0` is a refusal, any positive count passes.
pub fn check_stage_nonempty(staged_files: usize, branch: &str) -> Result<(), MergeRefusal> {
    if staged_files == 0 {
        return Err(MergeRefusal::EmptyStage {
            branch: branch.to_string(),
        });
    }
    Ok(())
}

/// `merge-lane.sh:20`'s guard: `before == after` (exact string equality of the two sha hex
/// strings) is a refusal. Never treats a shorter/abbreviated sha as equal to a full one.
pub fn check_head_moved(before: &str, after: &str, branch: &str) -> Result<(), MergeRefusal> {
    if before == after {
        return Err(MergeRefusal::HeadUnmoved {
            branch: branch.to_string(),
        });
    }
    Ok(())
}

/// `merge-lane.sh:22`'s guard: `changed_files == 0` is a refusal even though `HEAD` moved.
pub fn check_files_changed(changed_files: usize, branch: &str) -> Result<(), MergeRefusal> {
    if changed_files == 0 {
        return Err(MergeRefusal::NoFilesChanged {
            branch: branch.to_string(),
        });
    }
    Ok(())
}
