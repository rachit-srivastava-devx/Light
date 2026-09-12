#!/usr/bin/env bash
# M8 — every command cited as IMPLEMENTED in docs/BLUEPRINT-COHERENCE.md must still exist in the
# built binary's dispatch surface. Coherence measured once (B1/B2) decays the moment a command
# named as evidence for an IMPLEMENTED claim is renamed or removed and nobody re-reads the audit
# doc. This is the B1/B2 drift detector called for by B3.
#
# BLUEPRINT-COHERENCE.md does not use M6's `fleet <cmd>` backtick convention uniformly -- most
# command citations are bare (`sow`, `sow accept`, `ledger verify`, `route`), matching how the
# audit prose reads ("`sow` creates a ready record..."). Extracting every bare backticked
# lowercase token would false-positive on Rust identifiers (`run_with_evidence`, underscored --
# excluded by the char class below), JSON field names (`seq`), and POSIX primitives named in
# passing (`setsid`, `wait`) that are not fleet subcommands at all. Those three specific false
# positives are excluded by DENY_RE below (found by first running this extraction unfiltered
# against the current doc and reading every match by hand -- not a general NLP solver, the same
# "kept deliberately narrow" trade M6 made for negation).
#
# Scoped to rows whose result column is literally IMPLEMENTED: PARTIAL/ABSENT rows may legitimately
# name a command that does not fully work (`fleet impact` at 09.1 is PARTIAL) or does not exist at
# all (`recur` at 08.5 is ABSENT, named specifically because it is missing) -- those are not claims
# this detector should hold to "still exists".
set -u
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
# Same override chain as M9/M3/M10/install.sh's fixed form: FLEET_BIN wins, else CARGO_TARGET_DIR's
# debug/fleet, else the in-repo default. A hardcoded $ROOT/keel/target/debug/fleet path was a real
# bug found this session in M3/M9/M10/install.sh -- silently exit-77'ing (excluded, not checked)
# whenever the build moved to an external CARGO_TARGET_DIR, the exact AGENTS.md-recommended
# worktree convention. Not repeating it here.
B="${FLEET_BIN:-${CARGO_TARGET_DIR:-$ROOT/keel/target}/debug/fleet}"
[ -x "$B" ] || exit 77
DOC="$ROOT/docs/BLUEPRINT-COHERENCE.md"
[ -f "$DOC" ] || exit 77

CMD_RE='^[a-z][a-z-]*( [a-z][a-z-]*)?( --[a-z-]+)?$'
DENY_RE='^(seq|wait|setsid|main|crew|keel)$'

CMDS=""
while IFS= read -r line; do
  case "$line" in *"| IMPLEMENTED |"*) ;; *) continue ;; esac
  while IFS= read -r tok; do
    [ -n "$tok" ] || continue
    case "$tok" in
      "fleet "*) tok="${tok#fleet }" ;;
    esac
    [[ "$tok" =~ $CMD_RE ]] || continue
    [[ "$tok" =~ $DENY_RE ]] && continue
    CMDS="$CMDS
$tok"
  done < <(grep -oE '`[^`]*`' <<<"$line" | tr -d '`')
done < "$DOC"
CMDS=$(sort -u <<<"$CMDS" | grep -v '^$')
[ -n "$CMDS" ] || { echo "M8: found no IMPLEMENTED command citations to check -- measuring nothing is a failure"; exit 1; }

N=0; BAD=0
export FLEET_STATE="${FLEET_STATE:-$(mktemp -d "${TMPDIR:-/tmp}/fleet-m8.XXXXXX")}"
while IFS= read -r c; do
  [ -n "$c" ] || continue
  N=$((N+1))
  # shellcheck disable=SC2086
  if "$B" $c 2>&1 | grep -q 'unknown command'; then
    echo "M8: cited as IMPLEMENTED but missing from dispatch: fleet $c"
    BAD=$((BAD+1))
  fi
done <<< "$CMDS"
echo "M8: $((N-BAD)) of $N IMPLEMENTED-cited commands still exist in the dispatch surface (denominator: $N)"
[ "$BAD" -eq 0 ]
