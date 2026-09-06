#!/usr/bin/env bash
# M6 — every backticked `fleet <cmd>` in the docs must be a real command.
# docs/SECURITY.md described `fleet attest self` and `fleet sbom`, neither of which exists. A
# security document claiming capabilities the tool lacks is the surface a reviewer trusts without
# re-checking, and nothing read the docs, so it would have survived indefinitely (D59).
#
# B12: a doc line can legitimately backtick a command name while explicitly saying it does NOT
# exist, e.g. "`fleet arch` is not a command" or "correctly reports that `fleet arch` does NOT
# exist" — a truthful absent-claim, not a claim of capability. That is not a punctuation defect
# to fix in the doc (D59's backtick-means-exists convention still holds for every OTHER mention);
# it is a real sentence the detector must read. This is the SIXTH detector to fire on documentation
# *about* what it watches (T6, B10, T20, A8, M6) — a design smell in the detectors, not a discipline
# problem in the writers, so the fix lives here: per LINE, not per corpus-wide command name, so a
# command negated in one place is still checked if it is claimed positively somewhere else. Kept
# deliberately narrow (a fixed negation-phrase list near the mention on the same line) — not a
# general NLP negation solver, and mutation-tested below (docs/delta.d/B12.md) so this stays a
# real check and not a rule quietly taught to ignore things.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
B="${FLEET_BIN:-${CARGO_TARGET_DIR:-$ROOT/keel/target}/debug/fleet}"
[ -x "$B" ] || exit 77
# DELTA.md is excluded BY SCOPE, not by suppression: it is a historical record of defects and
# necessarily quotes commands that were broken or withdrawn. M6 checks documents that describe
# CURRENT capability -- README, SECURITY, ADOPTION, ROUTING and the rest. Scoping a check to the
# documents whose claims must be true is different from exempting a document that fails it.
# An ARRAY, not a string: the repo path contains a space, so an unquoted $DOCS split on it and
# grep read nothing -- M6 then correctly refused ("measuring nothing is a failure") rather than
# passing vacuously, which is the only reason the bug was visible.
DOCS=("$ROOT/README.md")
for d in "$ROOT"/docs/*.md; do
  case "$d" in */DELTA.md) continue ;; esac
  DOCS+=("$d")
done
# A negation phrase anywhere on the SAME line as a backticked mention means that mention is a
# truthful absent-claim, not a claim the command exists -- exclude only that line's mentions from
# the check. Deliberately a fixed phrase list, case-insensitive, not a general negation solver.
NEG_RE='does[[:space:]]+not[[:space:]]+exist|do[[:space:]]+not[[:space:]]+exist|does[[:space:]]+NOT[[:space:]]+exist|is not a (real )?command|not a real command|no such command|(was|were)[[:space:]]+never[[:space:]]+built'
POS_CMDS=""
NEG_COUNT=0
while IFS= read -r line; do
  [ -n "$line" ] || continue
  line_cmds=$(grep -oE '`fleet [a-z][a-z-]*( [a-z][a-z-]*)?`' <<<"$line" | tr -d '`' | sed 's/^fleet //')
  [ -n "$line_cmds" ] || continue
  if grep -qiE "$NEG_RE" <<<"$line"; then
    NEG_COUNT=$((NEG_COUNT + $(wc -l <<<"$line_cmds")))
    continue
  fi
  POS_CMDS="$POS_CMDS
$line_cmds"
done < <(grep -rhE '`fleet [a-z][a-z-]*( [a-z][a-z-]*)?`' "${DOCS[@]}" 2>/dev/null)
CMDS=$(sort -u <<<"$POS_CMDS" | grep -v '^totallybogus' | grep -v '^$')
[ -n "$CMDS" ] || { echo "M6: found no documented commands to check -- measuring nothing is a failure"; exit 1; }
N=0; BAD=0
export FLEET_STATE="${FLEET_STATE:-$(mktemp -d)}"
while IFS= read -r c; do
  [ -n "$c" ] || continue
  N=$((N+1))
  # shellcheck disable=SC2086
  if "$B" $c 2>&1 | grep -q 'unknown command'; then
    echo "M6: documented but missing: fleet $c"
    BAD=$((BAD+1))
  fi
done <<< "$CMDS"
echo "M6: $((N-BAD)) of $N documented commands exist (denominator: $N, excluded as truthfully-negated: $NEG_COUNT)"
[ "$BAD" -eq 0 ]
