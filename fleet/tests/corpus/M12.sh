#!/usr/bin/env bash
# M12 — a script in bin/ that nothing calls is dead code no other detector can see.
# B11 found `bin/perf-gate.sh` claimed (BACKLOG.md) to exist and be uncalled by `verify.sh`, and
# asked why M4/M5 didn't already catch it. Answer, verified by reading both: M4 only looks at
# `*_command` functions inside `keel/fleet/src/main.rs` (Rust dispatch dead code) -- a shell
# script in `bin/` is not that shape at all. M5 only checks tools listed **ADOPTED** in
# `docs/ADOPTION.md` (an external binary someone installed) -- a script this repo itself wrote and
# committed to `bin/` was never a candidate for that list; `bin/perf-gate.sh` isn't in
# `docs/ADOPTION.md` under any verdict. Neither detector's schema covers "a committed script with
# no caller." This one does, directly, for the `bin/` directory specifically (PRINCIPLES.md #1:
# an installed/committed thing is not the same claim as a thing something actually invokes).
#
# A caller is any OTHER tracked file (not itself) that names the script's basename OR its
# `bin/<name>` path. Documents count as callers here (unlike M5) because the concrete failure mode
# this closes is "committed and invoked by nothing at all", not "invoked only by a document" --
# M5 already owns that stricter bar for ADOPTED tools; duplicating it for every bin/ script would
# make an honest "just mentioned in a README, not actually run yet" script indistinguishable from
# a fully dead one.
set -u
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
BIN="$ROOT/bin"
[ -d "$BIN" ] || exit 77

# git ls-files, not `find` -- an uncommitted scratch script in a live worktree is not this
# detector's concern, and using git keeps the result identical for every checkout. A plain
# `while read` loop, not `mapfile` -- macOS ships bash 3.2 (no `mapfile`), and `verify.sh` targets
# that shell (see verify.sh's own D22 PATH-normalisation comment: "This is a mac-only tool").
scripts=$(cd "$ROOT" && git ls-files 'bin/*.sh' 2>/dev/null)
[ -n "$scripts" ] || { echo "M12: found no bin/*.sh scripts -- measuring nothing is a failure"; exit 1; }

TOTAL=0
# Every tracked text file except the scripts themselves is a candidate caller. `git grep` searches
# the committed tree, so this matches M4/M5's "the committed state is what's real" convention.
FAILED=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  TOTAL=$((TOTAL + 1))
  name="$(basename "$f")"
  # A script is its own trivial "caller" (its own shebang line, self-reference in comments) --
  # exclude it from its own search.
  hits=$(cd "$ROOT" && git grep -l --fixed-strings -e "$name" -- ':!'"$f" 2>/dev/null | wc -l | tr -d ' ')
  if [ "$hits" -eq 0 ]; then
    echo "M12: '$f' is committed but no other tracked file names it — dead code no other gate can see."
    FAILED=$((FAILED + 1))
  fi
done <<EOF
$scripts
EOF

echo "M12: $((TOTAL - FAILED)) of $TOTAL committed bin/*.sh scripts have at least one real caller (denominator: $TOTAL)"
[ "$FAILED" -eq 0 ]
