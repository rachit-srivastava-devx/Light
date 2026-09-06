#!/usr/bin/env bash
# gate.sh -- thin adapter to `no-mistakes` (ADOPT.md S2: "gate board + merge authority").
# no-mistakes owns review -> test -> document -> lint -> push -> PR -> CI; this script only
# drives its agent interface (`axi`), reads the TOON verdict, and records ONE fleet receipt
# per outcome so I1 (generator != verifier, registry/lib/receipt.sh) stays checkable. It contains no
# gate logic of its own -- see ADOPT.md S3 for the fleet-specific SDLC checks (zone boundaries,
# mutation score, budget-vs-runway, lesson guards, migration sign-off, property/contract/perf,
# revert-proof) that no-mistakes does not know about and this adapter does not attempt to
# replace.
#
# push, pr, and ci are ALWAYS skipped, and this floor cannot be lifted by --skip: this adapter
# never pushes, opens a PR, or touches CI. See ADOPT.md S3 for who owns closing that gap.
#
# Assumes: bash 3.2 (macOS default), the no-mistakes binary, registry/lib/err.sh, registry/lib/receipt.sh.
# Does NOT handle: answering an approval gate (that is `no-mistakes axi respond`, a human/agent
# decision, not this adapter's job) or multi-run orchestration -- one invocation, one receipt.
set -u

BIN_DIR="$(cd "$(dirname "$0")" && pwd)"
FLEET_ROOT="$(cd "$BIN_DIR/../../.." && pwd)"
. "$FLEET_ROOT/registry/lib/err.sh"
. "$FLEET_ROOT/registry/lib/receipt.sh"

# no-mistakes installs to ~/.local/bin, which a non-login shell may not have on PATH yet.
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) PATH="$HOME/.local/bin:$PATH" ;;
esac

GATE_NAME=no-mistakes
if [ -n "${FLEET_LEDGER:-}" ]; then
  LEDGER="$FLEET_LEDGER"
else
  LEDGER="$(mktemp "${TMPDIR:-/tmp}/fleet-gate-receipts.XXXXXX")"
fi
TASK_ID="${FLEET_TASK_ID:-$GATE_NAME}"
# Never invent a model name: an unsourced identity is the literal sentinel, not a guess.
GEN="${FLEET_GENERATOR_MODEL:-not-applicable}"
VER="${FLEET_VERIFIER_MODEL:-not-applicable}"
NOW="${FLEET_NOW:-}"
NEVER_SKIP="push,pr,ci"

usage() {
  printf '%s\n' \
    'gate.sh run --intent "TEXT" [--skip STEP,...] [--yes]' \
    'gate.sh status [--run ID]' \
    '  push,pr,ci are always skipped -- this adapter never touches a remote' \
    '  FLEET_LEDGER/FLEET_TASK_ID/FLEET_GENERATOR_MODEL/FLEET_VERIFIER_MODEL/FLEET_NOW override the receipt fields'
}

now_iso() { [ -n "$NOW" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }

# record <event: PASS|FAIL|BLOCKED> <exit_code> -- one receipt, mirrors gates/common.sh:gate_receipt.
record() {
  FLEET_LEDGER="$LEDGER" receipt_append "$(now_iso)" "$1" "$GATE_NAME" "$TASK_ID" "$GEN" "$VER" "$2" 0 0 0 >/dev/null \
    || die "$ERR_GENERIC" receipt_failed 'gate verdict could not be recorded' 'repair the receipt ledger and retry'
}

[ $# -ge 1 ] || { usage; exit "$ERR_USAGE"; }
SUB="$1"; shift
case "$SUB" in --help|-h) usage; exit "$ERR_OK" ;; esac

require_bin no-mistakes 'curl -fsSL https://raw.githubusercontent.com/kunchenguid/no-mistakes/main/docs/install.sh | sh (ADOPT.md S2)'

case "$SUB" in
  run)
    INTENT=""
    EXTRA_SKIP=""
    YES=0
    while [ $# -gt 0 ]; do
      case "$1" in
        --intent) need_val --intent "${2-}"; INTENT="${2-}"; shift 2 ;;
        --skip) need_val --skip "${2-}"; EXTRA_SKIP="${2-}"; shift 2 ;;
        --yes) YES=1; shift ;;
        --help|-h) usage; exit "$ERR_OK" ;;
        *) reject_unknown_flag "$1" ;;
      esac
    done
    [ -n "$INTENT" ] || die "$ERR_USAGE" intent_missing \
      'axi run requires --intent: what the user set out to accomplish, not a diff summary' \
      'pass --intent "..."'
    SKIP="$NEVER_SKIP"
    [ -n "$EXTRA_SKIP" ] && SKIP="$SKIP,$EXTRA_SKIP"

    # The no-mistakes board is downstream of the eight required repository states.
    if [ "${FLEET_GATE_SKIP_SDLC:-0}" -ne 1 ]; then
      # The 8-state pipeline is the product path and runs by default. FLEET_GATE_SKIP_PIPELINE=1
      # exists ONLY so the axi-wrapper unit tests can isolate their subject: they stub
      # no-mistakes but not the pipeline's eight real tools, so the pipeline failed at
      # `graph` and the wrapper was never reached -- 9 of 17 tests red for a reason that had
      # nothing to do with what they assert.
      if [ "${FLEET_GATE_SKIP_PIPELINE:-0}" != "1" ]; then
        "$FLEET_ROOT/registry/services/sdlc/pipeline.sh" run --repo "${FLEET_SDLC_REPO:-$FLEET_ROOT}"
      fi
      sdlc_ec=$?
      if [ "$sdlc_ec" -ne 0 ]; then
        record FAIL "$sdlc_ec"
        exit "$sdlc_ec"
      fi
    fi

    # Top-level `status` is the documented human health check (exits 0 either way); its
    # "not initialized" text is the one observed, stable signal that init must run first.
    ST_OUT="$(no-mistakes status 2>&1)"
    if printf '%s' "$ST_OUT" | grep -qi 'not initialized'; then
      INIT_OUT="$(no-mistakes --yes init </dev/null 2>&1)"
      init_ec=$?
      [ "$init_ec" -eq 0 ] || die "$ERR_USAGE" not_initialized \
        "no-mistakes init failed: $INIT_OUT" 'configure the repo (e.g. git remote add origin <url>) and retry'
    fi

    # TOON on stdout, progress on stderr (no-mistakes' own contract) -- kept apart so the parse
    # below never sees a stray progress line. stderr is inherited so a human sees live progress.
    if [ "$YES" -eq 1 ]; then
      OUT="$(no-mistakes axi run --intent "$INTENT" --skip "$SKIP" --yes </dev/null)"
    else
      OUT="$(no-mistakes axi run --intent "$INTENT" --skip "$SKIP" </dev/null)"
    fi
    nm_ec=$?
    printf '%s\n' "$OUT"

    # Exit contract read straight from no-mistakes' own source (internal/cli/axi_drive.go,
    # renderDriveResult): exit 0 = passed OR parked at a normal decision point OR checks-passed;
    # exit 1 = blocked, failed, or cancelled. A parked gate carries no `outcome:` field, only
    # `gate:` -- that is what tells the two exit-0 cases apart below.
    if [ "$nm_ec" -ne 0 ]; then
      record FAIL "$ERR_GENERIC"
      exit "$ERR_GENERIC"
    fi
    if printf '%s\n' "$OUT" | grep -q '^gate:'; then
      record BLOCKED "$ERR_AMBIGUOUS"
      exit "$ERR_AMBIGUOUS"
    fi
    record PASS "$ERR_OK"
    exit "$ERR_OK"
    ;;
  status)
    RUN_ID=""
    while [ $# -gt 0 ]; do
      case "$1" in
        --run) need_val --run "${2-}"; RUN_ID="${2-}"; shift 2 ;;
        --help|-h) usage; exit "$ERR_OK" ;;
        *) reject_unknown_flag "$1" ;;
      esac
    done
    if [ -n "$RUN_ID" ]; then
      no-mistakes axi status --run "$RUN_ID"
    else
      no-mistakes axi status
    fi
    ec=$?
    exit "$ec"
    ;;
  *) reject_unknown_flag "$SUB" ;;
esac
