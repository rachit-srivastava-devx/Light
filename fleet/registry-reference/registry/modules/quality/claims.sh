#!/usr/bin/env bash
# claims.sh — execute every prover in docs/reports/CLAIM-LEDGER.tsv.
#
# PROVEN means the command exited 0. FAILED means it ran and contradicted the claim.
# UNPROVABLE is explicit evidence that the repository has no honest deterministic check for it;
# it is excluded from the denominator, exactly like NOT-MECH in bin/corpus.sh.
set -u
D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$D" || exit 3
LEDGER="${FLEET_CLAIM_LEDGER:-$D/docs/reports/CLAIM-LEDGER.tsv}"
FILTER=""
case "${1-}" in
  '') ;;
  --id) [ -n "${2-}" ] || { printf 'claims: --id needs a value\n' >&2; exit 2; }; FILTER="$2"; shift 2 ;;
  --ledger) [ -n "${2-}" ] || { printf 'claims: --ledger needs a value\n' >&2; exit 2; }; LEDGER="$2"; shift 2 ;;
  -h|--help) printf '%s\n' 'claims.sh [--id CLAIM-ID] [--ledger PATH]'; exit 0 ;;
  *) printf 'claims: unknown flag %s\n' "$1" >&2; exit 2 ;;
esac
[ "$#" -eq 0 ] || { printf 'claims: unexpected argument %s\n' "$1" >&2; exit 2; }
[ -r "$LEDGER" ] || { printf 'claims: ledger not readable: %s\n' "$LEDGER" >&2; exit 3; }

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fleet-claims.XXXXXX")"
cleanup() {
  "$D/registry/services/measure/telemetry.sh" stop >/dev/null 2>&1 || true
  rm -r "$TMP"
}
trap cleanup EXIT HUP INT TERM
export FLEET_LEDGER="$TMP/RECEIPTS.jsonl"
export FLEET_TELEMETRY_DIR="$TMP/telemetry"
export FLEET_CLAIM_TMP="$TMP"

proven=0; failed=0; unprovable=0; total=0
printf '%-24s %-12s %s\n' CLAIM RESULT SOURCE
while IFS=$'\t' read -r id source claim prover expected; do
  case "$id" in ''|\#*|id) continue ;; esac
  [ -z "$FILTER" ] || [ "$id" = "$FILTER" ] || continue
  total=$((total + 1))
  out="$TMP/$id.out"; err="$TMP/$id.err"
  set +e
  FLEET_LEDGER="$FLEET_LEDGER" FLEET_CLAIM_TMP="$TMP" bash -c "$prover" >"$out" 2>"$err"
  rc=$?
  set -e
  first="$(head -1 "$out" 2>/dev/null || true)"
  if [ "$rc" -eq 4 ] || [[ "$first" == UNPROVABLE:* ]]; then
    result=UNPROVABLE; unprovable=$((unprovable + 1))
  elif [ "$rc" -eq 0 ]; then
    result=PROVEN; proven=$((proven + 1))
  else
    result=FAILED; failed=$((failed + 1))
  fi
  printf '%-24s %-12s %s\n' "$id" "$result" "$source"
  printf '  claim: %s\n' "$claim"
  printf '  expected: %s\n' "$expected"
  if [ -s "$out" ]; then sed -n '1,3p' "$out" | sed 's/^/  evidence: /'; fi
  if [ "$result" = FAILED ] && [ -s "$err" ]; then sed -n '1,3p' "$err" | sed 's/^/  error: /'; fi
done <"$LEDGER"

printf '\nCLAIM totals\n'
printf '  PROVEN       : %s\n' "$proven"
printf '  FAILED       : %s\n' "$failed"
printf '  UNPROVABLE   : %s\n' "$unprovable"
printf '  denominator  : %s\n' "$((total - unprovable))"
[ "$failed" -eq 0 ]
