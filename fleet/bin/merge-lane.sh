#!/usr/bin/env bash
# D31 — merging a lane that produced nothing must FAIL, not succeed quietly.
# The e-meter lane's meter.rs was lost because the worktree commit was run with output suppressed,
# so its failure was invisible; `git merge` on the resulting zero-commit branch exited 0 and I
# reported "merged clean". A merge is only a success if it moved something.
set -u
[ $# -ge 2 ] || { echo "usage: merge-lane.sh <worktree-dir> <branch>"; exit 3; }
D="$1"; B="$2"
[ -d "$D" ] || { echo "merge-lane: REFUSE: no worktree at $D"; exit 3; }
( cd "$D" && git add -A ':!*.pyc' ':!*/target/*' ) || { echo "merge-lane: REFUSE: git add failed in $D"; exit 6; }
STAGED=$( cd "$D" && git diff --cached --name-only | wc -l | tr -d ' ' )
if [ "$STAGED" -eq 0 ]; then
  echo "merge-lane: REFUSE: lane $B staged 0 files -- it produced nothing"; exit 6
fi
( cd "$D" && git -c core.hooksPath=/dev/null commit -qm "lane $B: $STAGED files" ) || {
  echo "merge-lane: REFUSE: commit failed in $D (this is the failure that used to be silent)"; exit 6; }
BEFORE=$(git rev-parse HEAD)
git -c core.hooksPath=/dev/null merge --no-edit -q "$B" || { echo "merge-lane: CONFLICT merging $B"; exit 6; }
AFTER=$(git rev-parse HEAD)
[ "$BEFORE" != "$AFTER" ] || { echo "merge-lane: REFUSE: merging $B moved HEAD nowhere -- nothing was integrated"; exit 6; }
CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" | wc -l | tr -d ' ')
[ "$CHANGED" -gt 0 ] || { echo "merge-lane: REFUSE: $B changed 0 files"; exit 6; }
echo "merge-lane: $B integrated ($CHANGED files, staged $STAGED)"

# D34/M3: a shared CARGO_TARGET_DIR caches the artifact built inside the lane's worktree, and
# CARGO_MANIFEST_DIR is baked in at compile time. Once the worktree is removed the binary points at
# a path that no longer exists and every registry read fails at RUNTIME with a green build. Caught
# twice now (.worktrees/rub, then .worktrees/multi), so invalidate on every merge rather than
# remembering to.
if command -v cargo >/dev/null 2>&1 && [ -f keel/fleet/Cargo.toml ]; then
  cargo clean -q -p fleet --manifest-path keel/fleet/Cargo.toml 2>/dev/null || true
  echo "merge-lane: invalidated the cached fleet artifact (D34)"
fi
