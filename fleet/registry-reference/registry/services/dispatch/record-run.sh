#!/usr/bin/env bash
# record-run.sh — make a spawned agent VISIBLE to fleet.
#
# The gap this closes, stated plainly: nine agents were fanned out to build fleet's fan-out
# visualiser, every one launched with `nohup codex exec` or the Agent tool directly, and fleet
# recorded NONE of them. The dashboard showed 25 briefs from `var/runs/*/brief.md` — historical paperwork —
# while the actual work of the night was invisible. A factory that cannot see its own floor is not
# a factory.
#
# The console builds its map from exactly two artifacts (see console/server/collect.mjs):
#   var/runs/<id>/brief.md  the prompt the agent received -> the `brief` node and its decomposes edge
#   var/runs/<id>/meta.txt  the measured outcome         -> the `agent` node, its model and lifecycle
# So anything that spawns an agent must write both, or the agent does not exist as far as fleet is
# concerned. That is what this does.
#
#   ./fleet record-run --id NAME --brief FILE --out FILE [--harness H] [--model M] [--role R]
#                     [--exit N] [--started ISO] [--ended ISO] [--target PATH]
#
# HONESTY RULE, load-bearing: a field that cannot be sourced is written as the empty value and
# rendered `unknown`, never guessed. Token counts are not invented — `ccusage` reports per-session
# totals, not per-spawn, so `tokens=` stays empty unless a real number is passed. A plausible number
# here would corrupt every cost metric downstream, which is exactly how `meter.sh` came to report a
# budget cap as if it were spend.
set -uo pipefail

D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
# shellcheck source=../registry/lib/err.sh
. "$D/registry/lib/err.sh" 2>/dev/null || { echo "record-run: registry/lib/err.sh missing" >&2; exit 3; }

ID=""; BRIEF=""; OUT=""; HARNESS=""; MODEL=""; ROLE=""; EXITC=""; STARTED=""; ENDED=""; TARGET=""
TOKENS=""; COST_MICRO=""; RAW_EXIT=""; GATE_EXIT=""; ELAPSED_OVERRIDE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --id)      need_val --id "${2-}";      ID="$2";      shift 2 ;;
    --brief)   need_val --brief "${2-}";   BRIEF="$2";   shift 2 ;;
    --out)     need_val --out "${2-}";     OUT="$2";     shift 2 ;;
    --harness) need_val --harness "${2-}"; HARNESS="$2"; shift 2 ;;
    --model)   need_val --model "${2-}";   MODEL="$2";   shift 2 ;;
    --role)    need_val --role "${2-}";    ROLE="$2";    shift 2 ;;
    --exit)    need_val --exit "${2-}";    EXITC="$2";   shift 2 ;;
    --started) need_val --started "${2-}"; STARTED="$2"; shift 2 ;;
    --ended)   need_val --ended "${2-}";   ENDED="$2";   shift 2 ;;
    --target)  need_val --target "${2-}";  TARGET="$2";  shift 2 ;;
    --tokens)  need_val --tokens "${2-}";  TOKENS="$2";  shift 2 ;;
    --cost-micro-usd) need_val --cost-micro-usd "${2-}"; COST_MICRO="$2"; shift 2 ;;
    --raw-exit) need_val --raw-exit "${2-}"; RAW_EXIT="$2"; shift 2 ;;
    --gate-exit) need_val --gate-exit "${2-}"; GATE_EXIT="$2"; shift 2 ;;
    --elapsed) need_val --elapsed "${2-}"; ELAPSED_OVERRIDE="$2"; shift 2 ;;
    -h|--help) sed -n '2,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "$ERR_USAGE" unknown_flag "unknown flag '$1'" "see --help" ;;
  esac
done
[ -n "$ID" ] || die "$ERR_USAGE" no_id "--id is required" 'record-run.sh --id NAME --brief FILE --out FILE'
[ -n "$BRIEF" ] && [ -f "$BRIEF" ] || die "$ERR_USAGE" no_brief "--brief must name a readable file" "the prompt the agent actually received"
case "$EXITC" in ''|*[!0-9]*) [ -z "$EXITC" ] || die "$ERR_USAGE" bad_exit "--exit must be an integer" "I3: counts are integers" ;; esac
case "$TOKENS:$COST_MICRO:$RAW_EXIT:$GATE_EXIT:$ELAPSED_OVERRIDE" in
  *[!0-9:]* ) die "$ERR_USAGE" bad_metric 'dispatch metrics must be non-negative integers' 'pass integer metric values' ;;
esac

# --- timestamp guard -------------------------------------------------------
# `stat -f '%Sm'` and a bare `date` return LOCAL time. Appending a Z relabels local as UTC and shifts
# the value by the tz offset — that put 41 run records up to 5.5h in the FUTURE (caught by the
# console, not by any test), and every consumer doing arithmetic on them got nonsense.
# Refuse rather than store: a clock that lies is worse than one that is absent, because absence is
# visibly unknown while a wrong number is silently trusted.
_reject_future() {
  [ -n "${1-}" ] || return 0
  python3 -c '
import sys, datetime
try:
    t = datetime.datetime.fromisoformat(sys.argv[1].replace("Z", "+00:00")).timestamp()
except Exception:
    sys.exit(0)
if t > datetime.datetime.now(datetime.timezone.utc).timestamp() + 60:
    print("record-run: refusing future timestamp " + sys.argv[1] + " -- use `date -u`, not local time", file=sys.stderr)
    sys.exit(1)
' "$1"
}

_reject_future "$STARTED" || exit 6
_reject_future "$ENDED"   || exit 6

mkdir -p "$D/var/runs/$ID"

# --- the brief -------------------------------------------------------------
# collect.mjs reads `# COMPONENT: <title>` (or the first `#` heading) plus optional `role:`,
# `owns:` and `requirements:` header lines. Write the header, then the VERBATIM prompt — the owner
# asked to see each agent's real prompt, so a summary here would defeat the point.
BRIEF_DEST="$D/var/runs/$ID/brief.md"
if [ -f "$BRIEF_DEST" ]; then
  echo "record-run: var/runs/$ID/brief.md already exists, not overwriting" >&2
else
  {
    printf '# COMPONENT: %s\n' "$ID"
    [ -n "$ROLE" ] && printf 'role: %s\n' "$ROLE"
    printf 'owns: %s\n' "var/runs/$ID"
    printf '\n'
    cat "$BRIEF"
  } >"$BRIEF_DEST"
fi

# --- the measured outcome ---------------------------------------------------
# `state` is derived from the real exit code, never asserted: 0 -> ok, anything else -> failed,
# absent -> unknown. The board has shown only `ok` and `unknown` for most of this build precisely
# because nothing was writing a real failure.
if [ -n "$EXITC" ]; then
  [ "$EXITC" -eq 0 ] && STATE=ok || STATE=failed
else
  STATE=unknown
fi
ELAPSED=""
if [ -n "$OUT" ] && [ -f "$OUT" ] && [ -n "$STARTED" ]; then
  ELAPSED="$(python3 - "$STARTED" "$OUT" <<'PY' 2>/dev/null || true
import sys, os, datetime
try:
    t0 = datetime.datetime.fromisoformat(sys.argv[1].replace('Z', '+00:00')).timestamp()
    print(max(0, int(os.path.getmtime(sys.argv[2]) - t0)))
except Exception:
    pass
PY
)"
fi
[ -n "$ELAPSED_OVERRIDE" ] && ELAPSED="$ELAPSED_OVERRIDE"

{
  printf 'id=%s\n' "$ID"
  printf 'harness=%s\n' "$HARNESS"
  printf 'model=%s\n' "$MODEL"
  printf 'role=%s\n' "$ROLE"
  printf 'exit=%s\n' "$EXITC"
  printf 'state=%s\n' "$STATE"
  printf 'elapsed_sec=%s\n' "$ELAPSED"
  printf 'tokens=%s\n' "$TOKENS"
  [ -n "$COST_MICRO" ] && printf 'cost_micro_usd=%s\n' "$COST_MICRO"
  [ -n "$RAW_EXIT" ] && printf 'raw_exit=%s\n' "$RAW_EXIT"
  [ -n "$GATE_EXIT" ] && printf 'gate_exit=%s\n' "$GATE_EXIT"
  printf 'started=%s\n' "$STARTED"
  printf 'ended=%s\n' "$ENDED"
  [ -n "$TARGET" ] && printf 'target=%s\n' "$TARGET"
  [ -n "$OUT" ] && printf 'output=%s\n' "$OUT"
} >"$D/var/runs/$ID/meta.txt"

if [ -n "$OUT" ] && [ -f "$OUT" ] && [ "$OUT" != "$D/var/runs/$ID/out.log" ]; then
  cp "$OUT" "$D/var/runs/$ID/out.log"
fi

# --- the receipt ------------------------------------------------------------
# A spawn is an operator action, not a generation, so both model fields are the sentinel. Writing
# `<component>`/`<component>-verifier` here is exactly the fabrication that let 447 of 521 receipts
# pass invariant I1 without ever comparing two models.
if [ -n "${FLEET_LEDGER:-}" ] || [ -f "$D/ledger/RECEIPTS.jsonl" ]; then
  FLEET_LEDGER="${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}" \
    receipt_append "$(date -u +%Y-%m-%dT%H:%M:%SZ)" spawn record-run "$ID" \
      not-applicable not-applicable "${EXITC:-0}" 0 0 0 >/dev/null 2>&1 || true
fi

printf 'recorded[1]{id,state,brief,meta}:\n  %s,%s,var/runs/%s/brief.md,var/runs/%s/meta.txt\n' \
  "$ID" "$STATE" "$ID" "$ID"
