#!/usr/bin/env bash
# M7 — every top-level command must appear in `--help`.
# `fleet plan` -- the natural-language front door, the headline capability -- was reachable and
# INVISIBLE. So were meter, lifecycle, graph and completions: 5 real commands a user could not
# discover (D60). M6 catches documented-but-missing; this catches the inverse, present-but-hidden.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
B="${FLEET_BIN:-${CARGO_TARGET_DIR:-$ROOT/keel/target}/debug/fleet}"
[ -x "$B" ] || exit 77
HELP="$("$B" --help 2>&1)"
[ -n "$HELP" ] || { echo "M7: --help produced nothing"; exit 1; }
# Top-level commands worth surfacing. Internal ones (prefixed __) and worker-protocol verbs are
# deliberately absent from help and listed here as exempt, with the reason.
EXEMPT="agent append count done dump git-diff list manifest note observed ok recorded-diff refuse requested-model served-model unit-suite verify version dispatch no-measurable-surface"
N=0; BAD=0
# The dispatch surface lives above the single `#[cfg(test)]` module (main.rs:4166). Module-internal
# literals are argument values to test calls (resolve_run_agent(Some("builder"), ...)), not commands,
# so extraction stops at the first `#[cfg(test)]` line. Predicate and exempt set are unchanged.
for c in $(awk '/^#\[cfg\(test\)\]/{exit} {print}' "$ROOT/keel/fleet/src/main.rs" \
           | grep -oE 'Some\("[a-z][a-z-]*"\)' | sed 's/Some("//; s/")//' | sort -u); do
  case " $EXEMPT " in *" $c "*) continue ;; esac
  case "$c" in -*|h|help) continue ;; esac
  N=$((N+1))
  printf '%s\n' "$HELP" | grep -qE "^  $c\b" || { echo "M7: reachable but absent from --help: fleet $c"; BAD=$((BAD+1)); }
done
[ "$N" -gt 0 ] || { echo "M7: no commands checked -- measuring nothing is a failure"; exit 1; }
echo "M7: $((N-BAD)) of $N top-level commands appear in --help (denominator: $N)"
[ "$BAD" -eq 0 ]
