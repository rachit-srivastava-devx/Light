#!/usr/bin/env bash
# readme.sh — the README's quickstart must actually run.
#
# It did not. `D39` put a planning gate in front of `run`, and the 60-second quickstart -- the
# first thing any reviewer copy-pastes -- kept calling `fleet run` directly and failed with "task
# has no accepted SOW". Nothing tested it, so nothing noticed. Fourth instance of one class: a
# later feature breaking an earlier flow that no assertion covered (D41, D51, D55, this).
#
# This EXTRACTS the shell block from README.md and runs it. It cannot drift from the document,
# because it IS the document.
set -u
if [ -n "${GIT_DIR:-}" ] || [ -n "${GIT_INDEX_FILE:-}" ]; then
  echo "FATAL: refusing to run with an inherited git context (D23)." >&2; exit 3
fi
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FLEET="${FLEET_BIN:-$ROOT/keel/target/debug/fleet}"
[ -x "$FLEET" ] || { echo "  FATAL: $FLEET not built"; exit 3; }

BLOCK="$(awk '/^## 60-second quickstart/{f=1} f&&/^```sh$/{c=1;next} c&&/^```$/{exit} c' "$ROOT/README.md")"
[ -n "$BLOCK" ] || { echo "  FAIL: no quickstart block found in README.md"; exit 1; }
LINES=$(printf '%s\n' "$BLOCK" | grep -c .)
[ "$LINES" -ge 10 ] || { echo "  FAIL: quickstart is only $LINES lines — did it get truncated?"; exit 1; }

WORK="$(mktemp -d)"
# `fleet` must resolve to the built binary, exactly as an installed one would.
printf '#!/usr/bin/env bash\nexec "%s" "$@"\n' "$FLEET" > "$WORK/fleet"
chmod +x "$WORK/fleet"
OUT="$(cd "$WORK" && PATH="$WORK:$PATH" FLEET_STATE="$WORK/state" bash -c "$BLOCK" 2>&1)"
RC=$?
if [ $RC -eq 0 ]; then
  echo "  ok   the README quickstart runs end to end (denominator: $LINES lines)"
  exit 0
fi
echo "  FAIL the README quickstart does not run (rc=$RC)"
printf '%s\n' "$OUT" | grep -viE 'sow bypass' | tail -4 | sed 's/^/       /'
exit 1
