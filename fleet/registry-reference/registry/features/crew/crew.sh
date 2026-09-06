#!/usr/bin/env bash
# crew.sh — thin adapter onto vendor/firstmate (kunchenguid/firstmate): crew orchestration,
# fan-out, and harness detection. See ADOPT.md #2 "crew orchestration". This file DELEGATES
# every real decision to firstmate's own scripts; it never reimplements dispatch, intent
# classification, or harness detection. Replaces: dispatch.sh route.sh fanout.sh detect.sh
# scheduled-run.sh fleet.sh codex-run.sh dashboard.sh wakeup.sh brief-loop.sh autopilot.sh
# wakeup-install.sh (see ADOPT.md §2 and the integration report for the exact mapping).
#
# Usage:
#   crew.sh detect [crew|secondmate|secondmate-model|secondmate-effort]
#   crew.sh guard
#   crew.sh status [<task-id>]
#   crew.sh brief <task-id> <repo-name> --mode <no-mistakes|direct-PR|local-only> [--herdr-lab]
#   crew.sh brief <task-id> <repo-name> --scout [--herdr-lab]
#   crew.sh dispatch <task-id> <project-dir> --mode <mode> --yolo <on|off> \
#                    [--harness H] [--model M] [--effort E] [--backend B]
#   crew.sh dispatch <task-id> <project-dir> --scout [--harness H] [--model M] [--effort E]
#   crew.sh control  <task-id> <interrupt|exit|relaunch> [--harness H] [--model M] \
#                    [--effort E] [--note TEXT] [--note-file PATH]
#
# Fan-out is repeated `dispatch`, one call per task id — nothing else is needed, because
# firstmate hands each spawn its own treehouse worktree, so the ownership-conflict check
# fanout.sh used to hand-roll cannot be triggered anymore (see the integration report).
#
# Exit: 0 ok | 2 usage | 3 firstmate distro missing/incomplete | 6 invariant I1 violated on
#       the crew receipt | otherwise the exact code the delegated firstmate script returned —
#       this is a passthrough, not a remapped contract.
#
# Env:
#   FLEET_FIRSTMATE_BIN   override the firstmate bin/ dir (tests point this at stub scripts,
#                         which is also how this is verified without ever spawning a real
#                         tmux session or agent process — see tests/crew.bats)
#   FLEET_FIRSTMATE_HOME  override FM_HOME (default fleet/.fleet/firstmate-home — NOT the
#                         vendored checkout, so runtime state never lands inside vendor/)
#   FLEET_GENERATOR_MODEL last-resort override for the receipt's generator_model
#   FLEET_LEDGER          receipt ledger path (see registry/lib/receipt.sh); tests MUST override this
#
# Does NOT: spawn or supervise anything itself (every side effect is a delegated firstmate
# call), classify task intent — firstmate's own design leaves ship-vs-scout and dispatch-
# profile matching to the calling agent's judgment (docs/architecture.md "Dispatch profiles"),
# so there is nothing mechanical here to delegate FOR that concern — or meter tokens/cost:
# firstmate has no token metering, so `status` and `dispatch` always report those fields null.
set -u

D="$(cd "$(dirname "$0")/../../.." && pwd)"
# shellcheck source=../../registry/lib/err.sh
. "$D/registry/lib/err.sh"
# shellcheck source=../../registry/lib/toon.sh
. "$D/registry/lib/toon.sh"
# shellcheck source=../../registry/lib/receipt.sh
. "$D/registry/lib/receipt.sh"

FM_BIN="${FLEET_FIRSTMATE_BIN:-$D/vendor/firstmate/bin}"
FM_HOME="${FLEET_FIRSTMATE_HOME:-$D/var/fleet/firstmate-home}"
NOW="${FLEET_NOW:-}"

now_iso() { [ -n "$NOW" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }

usage() { sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; }

# require_firstmate — locate + verify the vendored distro; exit 3 (ERR_NOTOOL) if absent or
# incomplete. Every subcommand calls this first, so a missing/partial vendor drop fails the
# same way everywhere instead of each caller discovering it differently.
require_firstmate() {
  local s
  [ -d "$FM_BIN" ] || die "$ERR_NOTOOL" firstmate_missing \
    "firstmate distro not found at $FM_BIN" "clone kunchenguid/firstmate into fleet/vendor/firstmate"
  for s in fm-harness.sh fm-spawn.sh fm-brief.sh fm-fleet-snapshot.sh fm-crew-state.sh \
           fm-guard.sh fm-control.sh; do
    [ -f "$FM_BIN/$s" ] || die "$ERR_NOTOOL" firstmate_incomplete \
      "firstmate script missing: $s" "the vendored copy is incomplete — reclone kunchenguid/firstmate"
  done
  mkdir -p "$FM_HOME"
}

# fm_run <script> [args...] — delegate to one firstmate script, scoped to fleet's own FM_HOME
# so its runtime state never lands inside the vendored checkout. Never reinterprets output.
fm_run() { local script="$1"; shift; FM_HOME="$FM_HOME" "$FM_BIN/$script" "$@"; }

# resolve_generator <harness> <model> — best-effort real name for the receipt's
# generator_model. Never invents one: falls back to "not-applicable" when unsourceable.
resolve_generator() {
  local harness="$1" model="$2" out ec
  if [ -n "$model" ]; then printf '%s' "$model"; return 0; fi
  if [ -n "$harness" ]; then printf '%s' "$harness"; return 0; fi
  if [ -n "${FLEET_GENERATOR_MODEL:-}" ]; then printf '%s' "$FLEET_GENERATOR_MODEL"; return 0; fi
  out="$(fm_run fm-harness.sh crew 2>/dev/null)"
  ec=$?
  if [ "$ec" -eq 0 ] && [ -n "$out" ]; then printf '%s' "$out"; else printf 'not-applicable'; fi
}

# record_receipt <event> <task_id> <generator> <verifier> <exit_code> — ONE receipt per crew
# task. receipt_append itself does not enforce I1 (it only checks the numeric fields are
# integers), so invariant_i1 is checked here explicitly, exactly like meter.sh/route.sh/
# lockcheck.sh do — never invent a passing model name to dodge it.
record_receipt() {
  local ev="$1" tid="$2" gen="$3" ver="$4" ec="$5" rc
  invariant_i1 "$gen" "$ver" \
    || die "$ERR_INVARIANT" invariant_i1 "generator_model and verifier_model must differ (got '$gen')" \
         "pass --harness or --model explicitly — firstmate could not identify one for this dispatch"
  receipt_append "$(now_iso)" "$ev" crew "$tid" "$gen" "$ver" "$ec" 0 0 0 >/dev/null
  rc=$?
  [ "$rc" -eq 0 ] || die "$rc" receipt_append_failed "could not append the crew receipt" \
    "repair FLEET_LEDGER and retry"
}

# report_null_metering <task_id> — requirement 4: firstmate reports neither tokens nor
# elapsed time for a task, so say null, plainly, rather than estimating either.
report_null_metering() {
  toon_open crew_metering 1 'task_id,tokens_in,tokens_out,elapsed_sec,reason'
  toon_row "${1:-fleet}" null null null 'firstmate reports neither token usage nor task duration'
}

cmd_status() {
  require_firstmate
  local id="${1-}" ec
  if [ -n "$id" ]; then fm_run fm-crew-state.sh "$id"; else fm_run fm-fleet-snapshot.sh --json; fi
  ec=$?
  report_null_metering "$id"
  return "$ec"
}

cmd_dispatch() {
  require_firstmate
  local id="${1-}" proj="${2-}" harness="" model="" gen ec
  [ -n "$id" ] || die "$ERR_USAGE" no_task_id "dispatch requires <task-id>" \
    "crew.sh dispatch <task-id> <project-dir> --mode <no-mistakes|direct-PR|local-only> --yolo <on|off>"
  [ -n "$proj" ] || die "$ERR_USAGE" no_project_dir "dispatch requires <project-dir>" \
    "crew.sh dispatch <task-id> <project-dir> --mode <no-mistakes|direct-PR|local-only> --yolo <on|off>"
  shift 2
  local args=("$id" "$proj")
  while [ $# -gt 0 ]; do
    case "$1" in
      --harness) need_val --harness "${2-}"; harness="$2"; args+=(--harness "$2"); shift 2 ;;
      --model)   need_val --model "${2-}";   model="$2";   args+=(--model "$2");   shift 2 ;;
      --effort)  need_val --effort "${2-}";  args+=(--effort "$2");  shift 2 ;;
      --backend) need_val --backend "${2-}"; args+=(--backend "$2"); shift 2 ;;
      --mode)    need_val --mode "${2-}";    args+=(--mode "$2");    shift 2 ;;
      --yolo)    need_val --yolo "${2-}";    args+=(--yolo "$2");    shift 2 ;;
      --scout)   args+=(--scout); shift ;;
      *) reject_unknown_flag "$1" ;;
    esac
  done
  # Resolve and validate the generator BEFORE spawning: failing invariant_i1 after a real
  # crew task is already under way would leave a spawned-but-unreceipted task behind.
  gen="$(resolve_generator "$harness" "$model")"
  invariant_i1 "$gen" not-applicable \
    || die "$ERR_INVARIANT" invariant_i1 "generator_model and verifier_model must differ (got '$gen')" \
         "pass --harness or --model explicitly — firstmate could not identify one for this dispatch"
  fm_run fm-spawn.sh "${args[@]}"
  ec=$?
  record_receipt crew_dispatch "$id" "$gen" not-applicable "$ec"
  report_null_metering "$id"
  return "$ec"
}

cmd_control() {
  require_firstmate
  local id="${1-}" verb="${2-}" ec
  [ -n "$id" ] || die "$ERR_USAGE" no_task_id "control requires <task-id>" \
    "crew.sh control <task-id> <interrupt|exit|relaunch>"
  case "$verb" in
    interrupt|exit|relaunch) ;;
    *) die "$ERR_USAGE" bad_verb "control verb must be interrupt, exit, or relaunch" \
         "crew.sh control <task-id> <interrupt|exit|relaunch>" ;;
  esac
  shift 2
  local args=("$id" "$verb")
  while [ $# -gt 0 ]; do
    case "$1" in
      --harness|--model|--effort|--note|--note-file)
        need_val "$1" "${2-}"; args+=("$1" "$2"); shift 2 ;;
      *) reject_unknown_flag "$1" ;;
    esac
  done
  fm_run fm-control.sh "${args[@]}"
  ec=$?
  # A control verb (interrupt/relaunch/exit) is an operator action, not a generation, so no
  # model generated it and none verified it. `crew-control` was a fabricated generator name
  # paired with a sentinel — the shape invariant_i1 now refuses on purpose.
  record_receipt crew_control "$id" not-applicable not-applicable "$ec"
  return "$ec"
}

CMD="${1:-}"
if [ -n "$CMD" ]; then shift; fi
case "$CMD" in
  detect)        require_firstmate; fm_run fm-harness.sh "$@" ;;
  guard)         require_firstmate; fm_run fm-guard.sh ;;
  brief)         require_firstmate; fm_run fm-brief.sh "$@" ;;
  status)        cmd_status "$@" ;;
  dispatch)      cmd_dispatch "$@" ;;
  control)       cmd_control "$@" ;;
  -h|--help|'')  usage ;;
  *) reject_unknown_flag "$CMD" ;;
esac
