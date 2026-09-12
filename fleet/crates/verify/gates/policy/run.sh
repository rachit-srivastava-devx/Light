#!/usr/bin/env bash
# Policy gate. Exit: 0 all policies proven BOTH directions · 3 conftest absent · 6 a policy is
# vacuous or wrong.
# NOTE: conftest looks for `deny` in package `main` unless --namespace is given. Without it, it finds
# NO rules and PASSES EVERYTHING — a vacuous gate. That shipped here and was caught only by running
# the BAD fixtures and noticing they passed. Proving one direction proves nothing.
set -u
cd "$(dirname "$0")/.."
command -v conftest >/dev/null || { echo "policy: conftest absent"; exit 3; }
P=0; F=0
for pol in policy/*.rego; do
  n="$(basename "$pol" .rego)"; ns="fleet.policy.$n"
  bad="policy/fixtures/$n.bad.json"; good="policy/fixtures/$n.good.json"
  [ -f "$bad" ] && [ -f "$good" ] || { echo "  FAIL $n: missing bad/good fixture pair"; F=$((F+1)); continue; }
  if conftest test --policy policy --namespace "$ns" "$bad" >/dev/null 2>&1; then
    echo "  FAIL $n: BAD fixture was ACCEPTED (vacuous policy)"; F=$((F+1)); continue
  fi
  if conftest test --policy policy --namespace "$ns" "$good" >/dev/null 2>&1; then
    echo "  ok   $n: rejects bad, accepts good"; P=$((P+1))
  else
    echo "  FAIL $n: GOOD control was REJECTED (over-broad policy)"; F=$((F+1))
  fi
done
echo "-- $P passed, $F failed (denominator: $((P+F)) policies) --"
[ "$((P+F))" -gt 0 ] || { echo "REFUSE: zero policies checked"; exit 6; }
[ "$F" -eq 0 ] || exit 6
