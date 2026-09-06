#!/usr/bin/env bash
# telemetry.sh — the ADOPT.md "agent telemetry" row: Arize Phoenix owns traces/tokens/cost/
# latency; tiktoken owns exact token counts; DuckDB owns the analytic queries (percentiles
# included). This file is integration glue ONLY -- no quantile math, no cost table, no
# estimate ever labelled as a measurement. Replaces process-plane.sh, perf.sh, metrics.sh, and
# meter.sh's token/cost reporting (meter.sh keeps only the gate/quota budget boundary, a
# different capability with its own dependents in route.sh/dispatch.sh/detect.sh).
#
# LOOPBACK CAVEAT (read before relying on this for anything internet-facing): phoenix serve's
# HTTP/UI server honours PHOENIX_HOST, forced to 127.0.0.1 below. Its OTLP gRPC receiver does
# NOT -- grpc_server.py hardcodes `add_insecure_port("[::]:<port>")` with no host override in
# this phoenix version (verified by reading site-packages/phoenix/server/grpc_server.py). This
# script never uses gRPC (protocol=http/protobuf only, enforced in telemetry_otel.py), so
# fleet's own traffic never needs that port. phoenix_loopback.py disables only that receiver because
# this Phoenix release does not expose a switch to disable it and hardcodes the wildcard host.
#
# ATTEMPT COUNTING: retry_depth/first_pass_yield were BLOCKED in process-plane.sh because
# nothing counted attempts. Fixed here: every `span` call carries a fleet.attempt integer.
# When the caller doesn't pass --attempt, it is derived by counting prior RECEIPTS.jsonl rows
# for the same --task-id and adding one -- real counting from the tamper-evident ledger, not an
# estimate, and it works even when Phoenix itself is down. `query retry-depth` /
# `first-pass-yield` then compute the two measures over span attempts via DuckDB. For aggregate
# per-model token and cost figures, `telemetry.sh usage` delegates to keyless ccusage, which reads
# local Claude/Codex transcripts and never receives an API key. It is deliberately separate from
# a task span: aggregate transcript usage must not be misattributed to one task.
set -u
D="$(cd "$(dirname "$0")/../../.." && pwd)"
# shellcheck source=../../registry/lib/toon.sh
. "$D/registry/lib/toon.sh"
# shellcheck source=../../registry/lib/receipt.sh
. "$D/registry/lib/receipt.sh"
# shellcheck source=../../registry/lib/err.sh
. "$D/registry/lib/err.sh"

PYBIN="${FLEET_PYTHON:-$D/var/venv/bin/python}"
HELPER="$D/registry/features/telemetry/telemetry_otel.py"
PHOENIX_BIN="${FLEET_PHOENIX_BIN:-$D/var/venv/bin/phoenix}"
PHOENIX_LOOPBACK="$D/registry/features/telemetry/phoenix_loopback.py"
PORT="${FLEET_PHOENIX_PORT:-6006}"
DIR="${FLEET_TELEMETRY_DIR:-$D/var/telemetry}"
PROJECT="${FLEET_TELEMETRY_PROJECT:-fleet}"
ENDPOINT="http://127.0.0.1:$PORT"
NOW="${FLEET_NOW:-}"
JSON=0

now_iso() { [ -n "$NOW" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }
_is_int() { case "${1-}" in ''|*[!0-9-]*|-) return 1 ;; *) return 0 ;; esac; }
_pidfile() { printf '%s/phoenix.pid' "$DIR"; }
_probe() { curl -sf -o /dev/null --max-time 2 "$ENDPOINT/healthz" 2>/dev/null; }
_alive() { local pid; pid="$(cat "$(_pidfile)" 2>/dev/null || true)"; [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; }

usage() {
  printf '%s\n' \
    'telemetry.sh [--json] [start|stop|status]' \
    'telemetry.sh span --event NAME --component NAME --task-id ID [--attempt N] [--model NAME]' \
    '  [--generator-model M] [--verifier-model M] [--exit N] [--tokens-in N] [--tokens-out N]' \
    '  [--token-source counted|reported] [--duration-ms N] [--cost-micro N]' \
    'telemetry.sh tokens --model NAME (--text S | --file PATH | --stdin) [--reported N]' \
    '  exact tiktoken count ("counted"), vs a harness-declared ("reported") value -- never' \
    '  averaged or conflated; both may be printed side by side.' \
    'telemetry.sh query {retry-depth|first-pass-yield|latency|SQL}' \
    '  runs DuckDB quantile_cont/aggregates over spans exported live from Phoenix.' \
    'telemetry.sh usage [daily|weekly|monthly|session|blocks]  # ccusage JSON, keyless local transcripts' \
    'telemetry.sh prove | prove-p50                       # queried trace / P50 proofs'
}

# _attempt_for <task_id> -> next attempt number for this task, by counting real ledger rows.
_attempt_for() {
  local tid="$1" ledger n
  ledger="${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}"
  [ -f "$ledger" ] || { printf '1'; return 0; }
  n="$(grep -cF "\"task_id\":\"$tid\"" "$ledger" 2>/dev/null || true)"
  case "$n" in ''|*[!0-9]*) n=0 ;; esac
  printf '%s' "$((n + 1))"
}

cmd_start() {
  mkdir -p "$DIR"
  if _probe; then cmd_status; return 0; fi
  [ -x "$PHOENIX_BIN" ] || die "$ERR_NOTOOL" tool_missing "phoenix not found at $PHOENIX_BIN" "./fleet setup --only arize-phoenix"
  [ -r "$PHOENIX_LOOPBACK" ] || die "$ERR_NOTOOL" phoenix_wrapper_missing "missing $PHOENIX_LOOPBACK" 'restore the telemetry adapter'
  PHOENIX_HOST=127.0.0.1 PHOENIX_PORT="$PORT" PHOENIX_WORKING_DIR="$DIR" \
    nohup "$PYBIN" "$PHOENIX_LOOPBACK" serve >"$DIR/phoenix.log" 2>&1 &
  echo $! >"$(_pidfile)"
  local n=0
  while [ "$n" -lt 30 ] && ! _probe; do
    _alive || break
    n=$((n + 1)); sleep 1
  done
  _probe || die "$ERR_GENERIC" phoenix_start_failed "phoenix did not become healthy within 30s" "inspect $DIR/phoenix.log"
  cmd_status
}

cmd_stop() {
  if _alive; then
    kill "$(cat "$(_pidfile)")" 2>/dev/null || true
    local n=0
    while [ "$n" -lt 10 ] && _alive; do n=$((n + 1)); sleep 1; done
    _alive && kill -9 "$(cat "$(_pidfile)")" 2>/dev/null || true
  fi
  rm -f "$(_pidfile)"
  if [ "$JSON" -eq 1 ]; then printf '{"state":"stopped"}\n'; else
    toon_preamble telemetry 'phoenix stopped' "$(now_iso)"; toon_open status 1 'state'; toon_row stopped
  fi
}

cmd_status() {
  local pid state=stopped
  pid="$(cat "$(_pidfile)" 2>/dev/null || true)"
  if _alive && _probe; then state=running; elif _alive; then state=starting; fi
  if [ "$JSON" -eq 1 ]; then
    jq -cn --arg state "$state" --arg url "$ENDPOINT" --arg pid "${pid:-}" \
      '{state:$state,url:$url,pid:(if $pid=="" then null else ($pid|tonumber) end)}'
  else
    toon_preamble telemetry 'phoenix lifecycle status (loopback only; see header for the gRPC caveat)' "$(now_iso)"
    toon_open status 1 'state,url,pid'
    toon_row "$state" "$ENDPOINT" "${pid:-none}"
  fi
}

cmd_span() {
  local event="" component="" task_id="" attempt="" model="" provider="" gen="" ver=""
  local exit_code=0 tin="" tout="" tsrc="unknown" dur=0 cost=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --event) need_val --event "${2-}"; event="$2"; shift 2 ;;
      --component) need_val --component "${2-}"; component="$2"; shift 2 ;;
      --task-id) need_val --task-id "${2-}"; task_id="$2"; shift 2 ;;
      --attempt) need_val --attempt "${2-}"; attempt="$2"; shift 2 ;;
      --model) need_val --model "${2-}"; model="$2"; shift 2 ;;
      --provider) need_val --provider "${2-}"; provider="$2"; shift 2 ;;
      --generator-model) need_val --generator-model "${2-}"; gen="$2"; shift 2 ;;
      --verifier-model) need_val --verifier-model "${2-}"; ver="$2"; shift 2 ;;
      --exit) need_val --exit "${2-}"; exit_code="$2"; shift 2 ;;
      --tokens-in) need_val --tokens-in "${2-}"; tin="$2"; shift 2 ;;
      --tokens-out) need_val --tokens-out "${2-}"; tout="$2"; shift 2 ;;
      --token-source) need_val --token-source "${2-}"; tsrc="$2"; shift 2 ;;
      --duration-ms) need_val --duration-ms "${2-}"; dur="$2"; shift 2 ;;
      --cost-micro) need_val --cost-micro "${2-}"; cost="$2"; shift 2 ;;
      -h | --help) usage; return 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac
  done
  { [ -n "$event" ] && [ -n "$component" ] && [ -n "$task_id" ]; } || die "$ERR_USAGE" span_args_missing "span requires --event --component --task-id" "see --help"
  _probe || cmd_start >/dev/null
  invariant_i3 "$exit_code" "${tin:-0}" "${tout:-0}" "$cost" || die "$ERR_INVARIANT" i3 "exit/tokens/cost must be integers" "pass whole numbers only"
  [ -n "$attempt" ] || attempt="$(_attempt_for "$task_id")"

  local payload span_out="null"
  payload="$(jq -cn --arg event "$event" --arg component "$component" --arg task_id "$task_id" \
    --argjson attempt "$attempt" --arg model "$model" --arg provider "$provider" \
    --arg generator_model "$gen" --arg verifier_model "$ver" --argjson exit_code "$exit_code" \
    --arg tin "$tin" --arg tout "$tout" --arg token_source "$tsrc" --argjson duration_ms "$dur" \
    --arg endpoint "$ENDPOINT" --arg project "$PROJECT" \
    '{event:$event,component:$component,task_id:$task_id,attempt:$attempt,model:$model,
      provider:$provider,generator_model:$generator_model,verifier_model:$verifier_model,
      exit_code:$exit_code,tokens_in:(if $tin=="" then null else ($tin|tonumber) end),
      tokens_out:(if $tout=="" then null else ($tout|tonumber) end),token_source:$token_source,
      duration_ms:$duration_ms,endpoint:$endpoint,project:$project}')"
  mkdir -p "$DIR"
  span_out="$(printf '%s' "$payload" | "$PYBIN" "$HELPER" span 2>"$DIR/last-span-error.log")" \
    || { printf 'telemetry: span emission failed (is phoenix running? "telemetry.sh start") -- receipt recorded anyway: %s\n' "$(cat "$DIR/last-span-error.log" 2>/dev/null)" >&2; span_out="null"; }

  local rc
  rc="$(receipt_append "$(now_iso)" "$event" "$component" "$task_id" "${gen:-none}" "${ver:-none}" "$exit_code" "${tin:-0}" "${tout:-0}" "$cost")" \
    || die "$ERR_GENERIC" receipt_failed "could not append the telemetry receipt" "repair FLEET_LEDGER and retry"

  if [ "$JSON" -eq 1 ]; then
    jq -cn --arg hash "$rc" --argjson attempt "$attempt" --argjson span "$span_out" '{receipt_hash:$hash,attempt:$attempt,span:$span}'
  else
    toon_preamble telemetry 'span recorded' "$(now_iso)"
    toon_open span 1 'event,component,task_id,attempt,receipt_hash'
    toon_row "$event" "$component" "$task_id" "$attempt" "$rc"
  fi
}

cmd_prove() {
  local task_id="claim-proof-$$" out payload
  cmd_span --event claim-proof --component claim-runner --task-id "$task_id" --model fleet-claim-prover \
    --tokens-in 3 --tokens-out 5 --duration-ms 120 >/dev/null
  payload="$(jq -cn --arg endpoint "$ENDPOINT" --arg project "$PROJECT" --arg task_id "$task_id" \
    '{endpoint:$endpoint,project:$project,task_id:$task_id}')"
  out="$(printf '%s' "$payload" | "$PYBIN" "$HELPER" readback)" || die "$ERR_GENERIC" trace_proof_failed 'Phoenix did not return the emitted trace' 'inspect telemetry logs'
  printf '%s\n' "$out"
}

cmd_prove_p50() {
  local prefix="claim-p50-$$" tmp export_ec phoenix duck
  local duration
  for duration in 100 200 300; do
    cmd_span --event claim-p50 --component claim-runner --task-id "$prefix-$duration" --model fleet-claim-prover \
      --tokens-in 3 --tokens-out 5 --duration-ms "$duration" >/dev/null
  done
  tmp="$(mktemp "$DIR/prove-p50.XXXXXX")" || die "$ERR_GENERIC" tmp_failed 'could not create P50 scratch file' 'check telemetry directory permissions'
  payload="$(jq -cn --arg endpoint "$ENDPOINT" --arg project "$PROJECT" '{endpoint:$endpoint,project:$project}')"
  printf '%s' "$payload" | "$PYBIN" "$HELPER" export >"$tmp" 2>"$DIR/last-export-error.log"
  export_ec=$?
  [ "$export_ec" -eq 0 ] || { rm -f "$tmp"; die "$ERR_GENERIC" export_failed 'Phoenix span export failed' 'inspect telemetry logs'; }
  phoenix="$(printf '%s' "$(jq -cn --arg endpoint "$ENDPOINT" --arg project "$PROJECT" --arg prefix "$prefix" \
    '{endpoint:$endpoint,project:$project,task_prefix:$prefix}')" | "$PYBIN" "$HELPER" p50)" || { rm -f "$tmp"; die "$ERR_GENERIC" phoenix_p50_failed 'Phoenix P50 query failed' 'inspect telemetry logs'; }
  duck="$(duckdb -json -c "SELECT quantile_cont(latency_ms,0.5) AS duckdb_p50 FROM read_json_auto('$tmp') WHERE task_id LIKE '$prefix-%'")"
  export_ec=$?
  rm -f "$tmp"
  [ "$export_ec" -eq 0 ] || die "$ERR_GENERIC" duckdb_p50_failed 'DuckDB P50 query failed' 'install duckdb'
  jq -cn --argjson p "$phoenix" --argjson d "$duck" '{phoenix_p50:$p.phoenix_p50,duckdb_p50:($d[0].duckdb_p50),span_count:$p.span_count,match:($p.phoenix_p50 == $d[0].duckdb_p50)}'
}

cmd_usage() {
  local group="${1:-daily}"
  case "$group" in
    daily|weekly|monthly|session|blocks) ;;
    *) die "$ERR_USAGE" usage_group "unknown ccusage group '$group'" 'use daily, weekly, monthly, session, or blocks' ;;
  esac
  require_bin ccusage 'install ccusage (it reads local Claude/Codex transcripts; no API key)'
  ccusage "$group" --json --offline
}

cmd_tokens() {
  local model="" text="" file="" use_stdin=0 reported="" counted="" encoding=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --model) need_val --model "${2-}"; model="$2"; shift 2 ;;
      --text) need_val --text "${2-}"; text="$2"; shift 2 ;;
      --file) need_val --file "${2-}"; file="$2"; shift 2 ;;
      --stdin) use_stdin=1; shift ;;
      --reported) need_val --reported "${2-}"; reported="$2"; shift 2 ;;
      -h | --help) usage; return 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac
  done
  if [ "$use_stdin" -eq 1 ]; then
    text="$(cat)"
  elif [ -n "$file" ]; then
    [ -r "$file" ] || die "$ERR_USAGE" file_unreadable "cannot read --file $file" "pass a readable path"
    text="$(cat "$file")"
  fi
  { [ -n "$text" ] || [ -n "$reported" ]; } || die "$ERR_USAGE" tokens_args_missing "tokens needs --text/--file/--stdin, or --reported, or both" "see --help"
  if [ -n "$text" ]; then
    local out; out="$(jq -cn --arg model "$model" --arg text "$text" '{model:$model,text:$text}' | "$PYBIN" "$HELPER" tokens)" \
      || die "$ERR_PARSE" tiktoken_failed "tiktoken could not count this text" "check the input"
    counted="$(printf '%s' "$out" | jq -r '.count')"; encoding="$(printf '%s' "$out" | jq -r '.encoding')"
  fi
  [ -z "$reported" ] || _is_int "$reported" || die "$ERR_INVARIANT" i3 "--reported must be an integer" "pass a whole token count"
  if [ "$JSON" -eq 1 ]; then
    jq -cn --arg c "$counted" --arg e "$encoding" --arg r "$reported" \
      '{counted:(if $c=="" then null else ($c|tonumber) end),encoding:(if $e=="" then null else $e end),reported:(if $r=="" then null else ($r|tonumber) end)}'
  else
    toon_preamble telemetry 'counted (tiktoken, exact) vs reported (harness-declared) -- never conflated' "$(now_iso)"
    toon_open tokens 1 'counted,encoding,reported'
    toon_row "${counted:-none}" "${encoding:-n/a}" "${reported:-none}"
  fi
}

cmd_query() {
  local sql tmp
  case "${1-}" in
    retry-depth) sql='SELECT task_id, MAX(attempt) AS retry_depth, COUNT(*) AS spans FROM spans WHERE task_id IS NOT NULL GROUP BY task_id ORDER BY retry_depth DESC' ;;
    first-pass-yield) sql='SELECT COUNT(DISTINCT task_id) AS total_tasks, COUNT(DISTINCT CASE WHEN attempt=1 AND exit_code=0 THEN task_id END) AS first_pass_ok FROM spans WHERE task_id IS NOT NULL' ;;
    latency) sql='SELECT name, COUNT(*) AS n, quantile_cont(latency_ms,0.50) AS p50_ms, quantile_cont(latency_ms,0.95) AS p95_ms, quantile_cont(latency_ms,0.99) AS p99_ms FROM spans WHERE latency_ms IS NOT NULL GROUP BY name' ;;
    '') die "$ERR_USAGE" sql_missing 'query needs SQL, or one of: retry-depth, first-pass-yield, latency' "telemetry.sh query 'SELECT ...'" ;;
    *) sql="$1" ;;
  esac
  require_bin duckdb './fleet setup --only duckdb'
  mkdir -p "$DIR"
  tmp="$(mktemp "$DIR/spans.XXXXXX")"   # BSD mktemp needs XXXXXX LAST; a suffix makes the template literal || die "$ERR_GENERIC" tmp_failed 'could not create a scratch file' 'check $DIR is writable'
  jq -cn --arg endpoint "$ENDPOINT" --arg project "$PROJECT" '{endpoint:$endpoint,project:$project}' | "$PYBIN" "$HELPER" export >"$tmp" 2>"$DIR/last-export-error.log"
  if [ ! -s "$tmp" ]; then rm -f "$tmp"; die "$ERR_GENERIC" export_failed "no spans exported (is phoenix running and non-empty?)" "telemetry.sh start; cat $DIR/last-export-error.log"; fi
  duckdb -json -c "CREATE VIEW spans AS SELECT * FROM read_json_auto('$tmp'); $sql"
  rm -f "$tmp"
}

while [ $# -gt 0 ]; do
  case "$1" in
    --json) JSON=1; shift ;;
    -h | --help) usage; exit 0 ;;
    start | stop | status | span | tokens | query | usage | prove | prove-p50) CMD="$1"; shift; break ;;
    *) reject_unknown_flag "$1" ;;
  esac
done
case "${CMD:-}" in
  start) cmd_start ;;
  stop) cmd_stop ;;
  status) cmd_status ;;
  span) cmd_span "$@" ;;
  tokens) cmd_tokens "$@" ;;
  query) cmd_query "$@" ;;
  usage) cmd_usage "${1:-daily}" ;;
  prove) cmd_prove ;;
  prove-p50) cmd_prove_p50 ;;
  '') usage ;;
esac
