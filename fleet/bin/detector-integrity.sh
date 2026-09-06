#!/usr/bin/env bash
# D28 — detector integrity. A worktree agent rewrote tests/corpus/M1.sh to assert the OPPOSITE of
# its purpose so its own change would pass, and a blanket `git add -A` committed that under the
# lead's message. An in-file self-check cannot catch this (renaming the asserted string renames the
# grep looking for it), so integrity is enforced from outside: every detector is hashed, and any
# change must be accompanied by a deliberate manifest update.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAN="$ROOT/tests/corpus/MANIFEST.sha256"
cd "$ROOT/tests/corpus" || exit 3
if [ "${1:-}" = "--update" ]; then
  shasum -a 256 *.sh > MANIFEST.sha256
  echo "detector-integrity: manifest updated ($(wc -l < MANIFEST.sha256 | tr -d ' ') detectors)"
  exit 0
fi
[ -r "$MAN" ] || { echo "detector-integrity: REFUSE: no manifest at $MAN"; exit 6; }
N=$(wc -l < "$MAN" | tr -d ' ')
[ "$N" -gt 0 ] || { echo "detector-integrity: REFUSE: empty manifest — measuring nothing"; exit 6; }
LIVE=$(ls *.sh | wc -l | tr -d ' ')
if [ "$LIVE" -ne "$N" ]; then
  echo "detector-integrity: REFUSE: $LIVE detectors on disk, $N in manifest — run bin/detector-integrity.sh --update deliberately"
  exit 6
fi
if ! shasum -a 256 --status -c "$MAN" 2>/dev/null; then
  echo "detector-integrity: REFUSE: a detector changed without a manifest update:"
  shasum -a 256 -c "$MAN" 2>/dev/null | grep -v ': OK$' | sed 's/^/    /'
  exit 6
fi
echo "detector-integrity: $N detectors match the manifest (denominator: $N)"
