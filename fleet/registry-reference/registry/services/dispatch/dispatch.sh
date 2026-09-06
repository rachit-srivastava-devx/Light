#!/usr/bin/env bash
# dispatch.sh — ONE surface, TWO real backends (claude, codex), ONE identical TOON envelope.
#
# WHY THIS EXISTS (req8): bin/codex-run.sh drives codex only; Sonnet is never used as a worker
# anywhere, and the opus-lead/codex-worker/sonnet-worker split has only ever been done by hand.
# This generalises codex-run.sh's dispatch pattern to any harness, so bin/fanout.sh can route
# work to codex AND claude automatically.
#
# Usage: dispatch.sh run --harness <claude|codex> --model <m> --role <r> --budget <micro_usd>
#                        --brief <file> [--target <repo-path>] [--dry-run] [--timeout <sec>]
#                        [--skip-intake-gate] (sow-author stage only)
#
# Exit: 0 ok | 1 backend failed/timed out | 2 usage | 3 harness/tool missing |
#       4 backend output unparseable | 5 budget refused | 6 guard refused / ownership violated
#
# THE GUARD LIVES HERE (see docs/design/DISPATCH.md, "Why the guard is here and not in the harness"):
# reused faithfully from bin/codex-run.sh's three-tier deny logic. NOTE: codex-run.sh's tier-B
# patterns originally used `[^\n]{0,N}` for the verb-to-target gap. In a POSIX bracket expression
# `\n` is NOT "newline" — it is the two literal characters backslash and 'n' — so `[^\n]` means
# "not backslash, not the letter n", and any gap containing a plain 'n' (e.g. "read main config
# then .env") silently escaped the pattern. codex-run.sh has since been fixed to use `.` (grep is
# line-oriented, so `.` never matches a newline anyway). This file was written FRESH with `.` from
# the start — verified empirically against the four bypass strings before writing a line of this
# guard (see tests/dispatch.test.sh's RECALL cases).
#
# Backends:
#   codex:  codex exec --skip-git-repo-check -s workspace-write [-m MODEL] -   (prompt on stdin)
#   claude: claude -p [--model MODEL] --output-format json                    (prompt on stdin)
#     --output-format json is what makes token/cost accounting possible for claude; plain -p text
#     mode prints only the final answer with no usage metadata.
#   --model claude-default / --model codex-default: caller wants the CLI's own configured default
#     rather than a forced model. dispatch.sh omits the model flag entirely in that case and still
#     records the sentinel string as generator_model, per instruction: never invent a real model
#     name for "no flag was passable/desired".
#
# Fixture injection (same idiom as registry/lib/meter.sh's FLEET_CCUSAGE_JSON / FLEET_QUOTA_JSON):
#   FLEET_CODEX_BIN / FLEET_CLAUDE_BIN  — override the binary invoked (default: codex / claude).
#     Tests point these at tiny stub scripts so the REAL subprocess/timeout/parse/receipt pipeline
#     runs for real, at zero token cost. Only ONE test in tests/dispatch.test.sh invokes the actual
#     `claude` binary — everything else, including "two concurrent dispatches", "timeout", and
#     "unparseable output", is exercised through stub binaries, not through --dry-run (--dry-run
#     structurally cannot reach those code paths, since it never invokes a backend at all).
#   FLEET_NOW — inject the timestamp used for TS_START/TS_END and the receipt (codex-run.sh does
#     not support this; added here for determinism, matching route.sh/meter.sh's convention).
#   FLEET_DISPATCH_TIMEOUT — default --timeout when the flag is not given (default 2700s).
#   FLEET_DISPATCH_HASH_MAX_KB — files at or under this size (KiB, default 1024) are content-
#     hashed in the pre-run snapshot; larger ones are byte-count-compared and a size-equal change
#     to one of them is reported UNVERIFIED, never cleared. Lowering it trades precision for time
#     in the safe direction (more findings, none dropped). Measured on this tree — 7,940 files /
#     215 MB, warm cache, macOS/APFS — the default snapshot costs 1.7s, against a backend call
#     that takes 96s–2700s.
#   FLEET_DISPATCH_SNAPSHOT — path of the SHARED pre-run content snapshot (default
#     .fleet/tree-snapshot.tsv), and FLEET_DISPATCH_SNAPSHOT_MAX_AGE_SEC (default 300) for how
#     long one may be reused. Concurrent dispatches share a build instead of each re-reading the
#     whole tree; every run still takes its own private copy before touching the marker. Point
#     the path somewhere fresh to force a cold build — which is how the suite tests it.
#   FLEET_DISPATCH_RUNS_DIR — where concurrent-run records live (default var/fleet/dispatch-runs).
#     Injectable so a test suite does not have to write into the live registry to exercise it.
#   FLEET_DISPATCH_RUN_TTL_MIN — how long a var/fleet/dispatch-runs/ record stays available for
#     attributing a concurrent write (default 1440 minutes). A record whose owner died without
#     finalising is kept for the same TTL on purpose: a killed run still wrote what it wrote.
#
# Design choices worth stating plainly:
#   - No `set -e`. codex-run.sh toggles it transiently; route.sh and meter.sh (the two newest,
#     most complex components) never use it at all. This file does a lot of grep/jq parsing where
#     "not found" is an expected, explicitly-checked outcome, not a script-aborting surprise — so
#     every exit status here is checked explicitly instead.
#   - The printed `exit` column and this script's own process exit code are the SAME value, and
#     that value is always one of the closed 0-8 codes from CONTRACT.md — never the raw 124 that
#     timeout(1) produces. `state` (ok/failed/timeout) is what carries the ok/failed/timeout
#     distinction; a bare 124 is never leaked into structured output, since 124 is outside the
#     closed table. The raw backend exit status is always visible in var/runs/<id>/meta.txt and the log.
#   - A backend that claims success (raw exit 0) but produced no trustworthy structured signal
#     (no "tokens used" marker for codex; no parseable JSON line for claude) is treated as upstream
#     output unparseable (exit 4), overriding its own exit-0 claim — because state=ok with a
#     silently-defaulted tokens=0 would be exactly the "silently wrong output" the L8 bar forbids.
#     A backend that reports failure/timeout via its OWN exit code is not additionally flagged as
#     unparseable just because it also has no token count — that failure is already honest.
#   - A write outside the brief's declared `owns:` set is detected in THREE stages, because
#     "this file's mtime moved" is not the same claim as "my backend wrote this file", and the
#     original single-stage `find -newer` sweep conflated the two. That was reproduced for real
#     on 2026-08-22 (docs/reports/CYCLE-PROOF.md, "The stage 5c incident"): a correctly-scoped run that
#     touched exactly its one owned file was failed with exit 6, naming three files a DIFFERENT,
#     concurrently-running dispatch had written during the same wall-clock window.
#       1. CONTENT, not mtime. A pre-run snapshot records a sha256 per file (a byte count for the
#          few above the hash cap). A candidate whose bytes are identical afterwards was touched,
#          not written — reported as such, never counted as a violation.
#       2. ATTRIBUTION. Every dispatch registers its id / pid / window / `owns:` set under
#          var/fleet/dispatch-runs/. A changed path that this run's own backend log never mentions,
#          but that another dispatch with an OVERLAPPING window had declared, is reported as that
#          run's write. fanout.sh's pre-flight owns-conflict check only covers briefs a single
#          fanout knows about; this also covers ad-hoc `dispatch.sh run` calls and a second fanout.
#       3. HONEST LABELS. What survives 1 and 2 is still exit 6, but the row says whether the
#          backend's own log corroborates it, and the help block prints the command to check. A
#          same-window write by a human or a non-fleet agent is genuinely undecidable from here;
#          saying so is the fix — presenting it as a confirmed violation was the defect.
#     Ordering matters and is deliberate: backend-log evidence OUTRANKS attribution, so a path
#     this run's own log names stays a violation even when another dispatch also declared it.
#     Attribution can only ever excuse a path this run left no trace of.
#     If a brief declares no `owns:` line at all, the check cannot run — that is surfaced as a
#     defect row, never silently skipped.
#
# Does NOT handle: multi-turn sessions (one-shot only), resuming a killed run, per-call USD cost
# for the codex backend (its plain-text output carries no cost field, so cost_micro_usd is 0 for
# codex dispatches — codex-run.sh's own precedent), or verifying the SEMANTIC content of a backend's
# response (that is fanout.sh's manual-verify dispatch, by design — this file only executes).
set -u

D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/err.sh"
. "$D/registry/lib/toon.sh"
. "$D/registry/lib/receipt.sh"

NOW="${FLEET_NOW:-}"
now_iso() { if [ -n "$NOW" ]; then printf '%s' "$NOW"; else date -u +%Y-%m-%dT%H:%M:%SZ; fi; }

CODEX_BIN="${FLEET_CODEX_BIN:-codex}"
CLAUDE_BIN="${FLEET_CLAUDE_BIN:-claude}"
NO_MISTAKES_BIN="${FLEET_NO_MISTAKES_BIN:-no-mistakes}"
DEFAULT_TIMEOUT="${FLEET_DISPATCH_TIMEOUT:-2700}"

JSON=0
HARNESS=""; MODEL=""; ROLE=""; BUDGET=""; BRIEF=""; TARGET=""; TARGET_WORKTREE=""; DRYRUN=0; TIMEOUT="$DEFAULT_TIMEOUT"; SKIP_INTAKE_GATE=0

usage() {
  printf '%s\n' \
    'dispatch.sh run --harness <claude|codex> --model <m> --role <r> --budget <micro_usd> --brief <file> [--target <repo-path>] [--dry-run] [--timeout <sec>] [--json]' \
    '  --harness   claude | codex — which backend CLI actually runs' \
    '  --model     model name, or claude-default / codex-default to omit the model flag' \
    '  --role      free-text role label (e.g. implementation, verification); recorded on the receipt and in the prompt' \
    '  --budget    integer micro-USD cap; enforced via registry/features/budget/budget.sh before any backend runs' \
    '  --brief     path to the brief file; piped to the backend on stdin, never interpolated into a shell string' \
    '  --dry-run   run the guard and the budget gate but never invoke the backend (zero-token check)' \
    '  --skip-intake-gate  intake authoring only; requires role=intake and the sow-author brief marker' \
    "  --timeout   wall-clock seconds before the backend is killed (default: $DEFAULT_TIMEOUT)" \
    '  --json      emit the normalized JSON model instead of TOON'
}

# ---- flag parsing ---------------------------------------------------------
[ $# -gt 0 ] || { usage; exit 2; }
case "$1" in
  run) shift ;;
  -h|--help) usage; exit 0 ;;
  *) reject_unknown_flag "$1" ;;
esac

while [ $# -gt 0 ]; do
  case "$1" in
    --harness) need_val --harness "${2-}"; HARNESS="${2:-}"; shift 2 ;;
    --model) need_val --model "${2-}"; MODEL="${2:-}"; shift 2 ;;
    --role) need_val --role "${2-}"; ROLE="${2:-}"; shift 2 ;;
    --budget) need_val --budget "${2-}"; BUDGET="${2:-}"; shift 2 ;;
    --brief) need_val --brief "${2-}"; BRIEF="${2:-}"; shift 2 ;;
    --target) need_val --target "${2-}"; TARGET="${2:-}"; shift 2 ;;
    --timeout) need_val --timeout "${2-}"; TIMEOUT="${2:-}"; shift 2 ;;
    --dry-run) DRYRUN=1; shift ;;
    --skip-intake-gate) SKIP_INTAKE_GATE=1; shift ;;
    --json) JSON=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) reject_unknown_flag "$1" ;;
  esac
done

[ -n "$HARNESS" ] || die "$ERR_USAGE" missing_harness "--harness <claude|codex> is required" "dispatch.sh run --harness codex --model codex-default --role implementation --budget 100000 --brief var/runs/x/brief.md"
case "$HARNESS" in
  claude|codex) ;;
  *) die "$ERR_USAGE" bad_harness "unknown harness '$HARNESS'" "use --harness claude or --harness codex" ;;
esac
[ -n "$MODEL" ] || die "$ERR_USAGE" missing_model "--model <m> is required" "pass --model <name>, or claude-default / codex-default"
[ -n "$ROLE" ] || die "$ERR_USAGE" missing_role "--role <r> is required" "pass --role implementation (or another task-shape role)"
[ -n "$BUDGET" ] || die "$ERR_USAGE" missing_budget "--budget <micro_usd> is required" "pass --budget 100000"
case "$BUDGET" in ''|*[!0-9]*) die "$ERR_USAGE" bad_budget "--budget must be a non-negative integer micro-USD value" "pass --budget 100000" ;; esac
[ -n "$BRIEF" ] || die "$ERR_USAGE" missing_brief "--brief <file> is required" "pass --brief path/to/brief.md"
[ -f "$BRIEF" ] || die "$ERR_USAGE" brief_missing "brief not found: $BRIEF" "check the path"
BRIEF="$(cd "$(dirname "$BRIEF")" 2>/dev/null && pwd)/$(basename "$BRIEF")" || die "$ERR_USAGE" brief_missing "brief not readable: $BRIEF" "check the path"

if [ "$SKIP_INTAKE_GATE" -eq 1 ]; then
  [ "$ROLE" = intake ] && [ "${FLEET_SOW_AUTHOR_STAGE:-}" = 1 ] && \
    grep -q '^sow-author-stage: true$' "$BRIEF" || \
    die "$ERR_USAGE" intake_gate_bypass_forbidden 'the intake-gate bypass is reserved for sow-author.sh' 'run the intake authoring stage instead of dispatching with this flag'
fi

# Code cannot start from a brief alone. The staged intake gate is the deterministic
# authority for SOW, atomic decomposition, corpus-backed challenges, and answered
# blocking clarifications. A model may draft those files; this check decides.
#
# ID is derived here -- once, from BRIEF alone, which is already an absolute path by this point --
# so a --target run's receipt ledger can be placed at a DURABLE path before the intake gate runs.
# The gate's own refusal is exactly the receipt this must not lose: a measured `--target` dispatch
# that refused with exit 7 correctly wrote refusal:intake_not_ready | fleet | exit 7 -- but into a
# mktemp file under $TMPDIR, which nothing reads, nothing aggregates, and macOS purges (three such
# orphans were found sitting there, one holding 1,362 bytes of lost evidence). var/runs/<id>/ is
# where a reader already looks (meta.txt, out.log live there) -- it is deliberately NOT
# ledger/RECEIPTS.jsonl, so a measurement/target run can never append to the production chain
# (776 rows once broke that chain permanently at line 229).
ID="$(basename "$BRIEF" .md)"
TARGET_LEDGER=""
if [ -n "$TARGET" ] && [ -z "${FLEET_LEDGER:-}" ]; then
  TARGET_LEDGER="$D/var/runs/$ID/target-ledger.jsonl"
  export FLEET_LEDGER="$TARGET_LEDGER"
fi
if [ "$SKIP_INTAKE_GATE" -eq 0 ]; then
  INTAKE_GATE_OUT="$(bash "$D/registry/features/intake/intake.sh" --json gate 2>&1)"
  INTAKE_GATE_EC=$?
  [ "$INTAKE_GATE_EC" -eq 0 ] || die "$ERR_AMBIGUOUS" intake_not_ready "intake gate refused dispatch: $INTAKE_GATE_OUT" 'complete intake.sh sow, atomic, challenges, and clarifications, then answer blockers'
else
  printf 'dispatch: intake gate bypassed for the marked sow-author stage\n' >&2
fi

case "$TIMEOUT" in ''|*[!0-9]*) die "$ERR_USAGE" bad_timeout "--timeout must be a non-negative integer number of seconds" "pass --timeout 2700" ;; esac

require_bin jq 'install jq and retry'
require_bin shasum 'install shasum and retry'
require_bin timeout 'install GNU coreutils (brew install coreutils) and retry'

if [ -n "$TARGET" ]; then
  require_bin git 'install git and retry'
  TARGET="$(cd "$TARGET" 2>/dev/null && pwd)" || die "$ERR_USAGE" target_missing "target is not a readable directory: $TARGET" 'pass --target /path/to/git/repo'
  git -C "$TARGET" rev-parse --show-toplevel >/dev/null 2>&1 || die "$ERR_USAGE" target_not_git "target is not a git repository: $TARGET" 'initialize the target repository and retry'
fi

case "$HARNESS" in
  codex) BACKEND_BIN="$CODEX_BIN" ;;
  claude) BACKEND_BIN="$CLAUDE_BIN" ;;
esac
HARNESS_UPPER="$(printf '%s' "$HARNESS" | tr '[:lower:]' '[:upper:]')"
require_bin "$BACKEND_BIN" "install $HARNESS and ensure it is on PATH, or set FLEET_${HARNESS_UPPER}_BIN to its path"

# ---- harness preflight: an environment fault must not become an agent failure ---------------
# Codex is an ESM Node entrypoint. `#!/usr/bin/env node` makes its result depend on the first
# node on PATH, so a stale Node can make a healthy Codex install look like a failed agent. Probe
# the interpreter before the CLI, then probe the CLI exactly as it will be invoked.
BACKEND_PATH="$(command -v "$BACKEND_BIN" 2>/dev/null || true)"
BACKEND_ENV_PATH="$PATH"
BACKEND_PROBE_OUTPUT=""
BACKEND_PROBE_EC=0
BACKEND_USES_NODE=0
NODE_BIN=""

node_runs_esm() {
  local candidate="$1"
  [ -x "$candidate" ] || return 1
  "$candidate" --input-type=module -e 'import { spawn } from "node:child_process"; process.exit(0)' >/dev/null 2>&1
  local probe_ec=$?
  [ "$probe_ec" -eq 0 ]
}

resolve_esm_node() {
  local candidate node_dir search_dirs ambient_node
  NODE_BIN=""

  if [ -n "${FLEET_NODE_BIN:-}" ]; then
    candidate="$FLEET_NODE_BIN"
    case "$candidate" in
      */*) ;;
      *) candidate="$(command -v "$candidate" 2>/dev/null || true)" ;;
    esac
    if node_runs_esm "$candidate"; then NODE_BIN="$candidate"; return 0; fi
    return 1
  fi

  # A test or an embedding environment can replace the search roots. The default searches the
  # current PATH, the CLI's sibling bin directory, and common macOS/Linux install locations.
  if [ "${FLEET_NODE_SEARCH_PATHS+x}" = x ]; then
    search_dirs="$FLEET_NODE_SEARCH_PATHS"
  else
    search_dirs="/opt/homebrew/bin:/usr/local/bin:/opt/local/bin:/usr/bin"
  fi

  ambient_node="$(command -v node 2>/dev/null || true)"
  if [ -n "$ambient_node" ] && node_runs_esm "$ambient_node"; then
    NODE_BIN="$ambient_node"
    return 0
  fi

  if [ "${FLEET_NODE_SEARCH_PATHS+x}" != x ]; then
    for node_dir in ${PATH//:/ }; do
      candidate="$node_dir/node"
      if node_runs_esm "$candidate"; then NODE_BIN="$candidate"; return 0; fi
    done
  fi

  if [ "${FLEET_NODE_SEARCH_PATHS+x}" != x ]; then
    node_dir="$(dirname "$BACKEND_PATH")"
    candidate="$node_dir/node"
    if node_runs_esm "$candidate"; then NODE_BIN="$candidate"; return 0; fi
  fi

  for node_dir in ${search_dirs//:/ }; do
    candidate="$node_dir/node"
    if node_runs_esm "$candidate"; then NODE_BIN="$candidate"; return 0; fi
  done
  return 1
}

if [ "$HARNESS" = codex ] && [ -f "$BACKEND_PATH" ]; then
  case "$(sed -n '1p' "$BACKEND_PATH" 2>/dev/null || true)" in
    *node*) BACKEND_USES_NODE=1 ;;
  esac
fi

if [ "$BACKEND_USES_NODE" -eq 1 ]; then
  if ! resolve_esm_node; then
    ambient_node="$(command -v node 2>/dev/null || printf '%s' 'not found')"
    die "$ERR_NOTOOL" codex_node_unusable \
      "codex CLI cannot run: no ESM-capable Node.js interpreter was found (ambient node: $ambient_node)" \
      'install Node.js 16 or newer and put its bin directory on PATH, or set FLEET_NODE_BIN to a verified node'
  fi
  BACKEND_ENV_PATH="$(dirname "$NODE_BIN"):$PATH"
fi

# This is an environment/tool preflight, not an agent run. A nonzero probe is therefore always
# ERR_NOTOOL, with the actual tail preserved as the repair signal.
BACKEND_PROBE_OUTPUT="$(PATH="$BACKEND_ENV_PATH" timeout 10 "$BACKEND_BIN" --version 2>&1)"
BACKEND_PROBE_EC=$?
if [ "$BACKEND_PROBE_EC" -ne 0 ]; then
  PROBE_REASON="$(printf '%s\n' "$BACKEND_PROBE_OUTPUT" | tail -5 | tr '\n' ' ')"
  [ -n "$PROBE_REASON" ] || PROBE_REASON="exit $BACKEND_PROBE_EC with no diagnostic output"
  die "$ERR_NOTOOL" harness_unusable \
    "$HARNESS CLI failed its --version preflight (exit $BACKEND_PROBE_EC): $PROBE_REASON" \
    "repair/reinstall $HARNESS and verify its runtime; this is an environment fault, not an agent failure"
fi

# ---- tier-C support: does this line TARGET the protected path, or FORBID it? -------------------
# The defect this fixes (docs/reports/CYCLE-PROOF.md, "What a junior would have hit", stage 5a): tier C
# matched a bare substring, so a brief whose entire point was "do not touch ledger/" was refused
# by the same rule as one saying "append the result to ledger/RECEIPTS.jsonl" — and the remedy it
# printed, `add a guard-allow: line`, told the author to GRANT the exact permission the sentence
# was written to withhold. Both halves are wrong: the verdict and the advice.
#
# Getting this right is narrower than "look for a negation", because a negation can be an
# instruction: "never skip writing to ledger/" is a double negative that MUST still be denied.
# So the test is whitelist-shaped and unknown wording always falls to DENY:
#
#   a line is a prohibition only if EVERY word between the last negation marker and the protected
#   token is on the approved access-verb / filler list below.
#
#   "do not touch ledger/"                       -> [touch]                     -> allowed
#   "must not create or modify files in ledger/" -> [create or modify files in]  -> allowed
#   "never skip writing to ledger/"              -> [skip ...]                   -> DENIED
#   "do not forget to append to RECEIPTS.jsonl"  -> [forget ...]                 -> DENIED
#   "do not hesitate to write to ledger/"        -> [hesitate ...]               -> DENIED
#
# Matching is word-level, not substring-level, on purpose: "another" contains the letters n-o-t,
# and a regex hunting for `not` would have found a negation inside it.
GUARD_NEG_WORDS=' not never dont cant wont cannot doesnt didnt shouldnt mustnt isnt arent nor no none neither without avoid avoids avoiding refrain excluding except outside '
GUARD_OK_WORDS=' touch touching modify modifying write writing writes read reading edit editing append appending create creating delete deleting remove removing change changing alter altering access accessing update updating use using commit committing reference referencing rename renaming move moving copy copying overwrite overwriting include including includes the a an any anything anywhere its it this that these those their your our my other another new existing and or of from at on in into inside to under within near for else file files path paths dir directory directories folder folders content contents line lines entry entries record records row rows ever also then even still just only '

# guard_line_is_prohibition <line> <token_regex> — exit 0 when the line only forbids the token.
# Pure text classification: no filesystem, no wall clock. O(words on the line).
guard_line_is_prohibition() {
  local line="$1" tok="$2" low tokl prefix words w marker_seen=0 tailwords=''
  low="$(printf '%s' "$line" | LC_ALL=C tr '[:upper:]' '[:lower:]' | LC_ALL=C tr -d "'" | sed "s/\xe2\x80\x99//g")"
  tokl="$(printf '%s' "$tok" | LC_ALL=C tr '[:upper:]' '[:lower:]')"
  # everything to the left of the FIRST occurrence of the protected token on this line
  prefix="$(printf '%s' "$low" | sed -E "s|${tokl}.*\$||")"
  [ "$prefix" != "$low" ] || return 1          # token not on this line after all -> not our call
  words="$(printf '%s' "$prefix" | LC_ALL=C tr -c 'a-z0-9' ' ')"
  for w in $words; do
    case "$GUARD_NEG_WORDS" in
      *" $w "*) marker_seen=1; tailwords='' ; continue ;;
    esac
    [ "$marker_seen" = 1 ] && tailwords="$tailwords $w"
  done
  [ "$marker_seen" = 1 ] || return 1             # no negation at all -> a target
  for w in $tailwords; do
    case "$GUARD_OK_WORDS" in *" $w "*) ;; *) return 1 ;; esac
  done
  return 0
}

# ---- THE GUARD (dispatcher-side; every backend passes through here) ------
# Faithful reimplementation of bin/codex-run.sh's three-tier deny logic, written with `.` for the
# verb-to-target gap from the start (see header comment for why `[^\n]` is a bypassable bug).
guard_deny() {
  local f="$1" pat hit line cleared
  for pat in 'id_rsa' '\.ssh/' 'security +(dump|find)-(keychain|generic)' 'BEGIN [A-Z ]*PRIVATE KEY'; do
    hit="$(grep -nEi "$pat" "$f" 2>/dev/null | head -1 || true)"
    [ -z "$hit" ] || { printf 'guard: HARD-DENY /%s/ at %s\n' "$pat" "$hit" >&2; return 1; }
  done
  for pat in '(cat|read|open|print|echo|source|export|send|curl|post|upload|exfiltrat).{0,40}(\.env|credentials\.json|_API_KEY|_SECRET|_TOKEN)' \
             '(\.env|credentials\.json|_API_KEY|_SECRET).{0,30}(value|contents|and (send|post|upload))'; do
    hit="$(grep -nEi "$pat" "$f" 2>/dev/null | head -1 || true)"
    [ -z "$hit" ] || { printf 'guard: HARD-DENY exfiltration-intent at %s\n' "$hit" >&2; return 1; }
  done
  # Tier C — protected bookkeeping. Two explicit overrides, and they mean different things:
  #   guard-allow:   <pattern>  this brief OWNS and writes that path.
  #   guard-mention: <pattern>  this brief only refers to the path; grants nothing.
  for pat in 'RECEIPTS\.jsonl' 'ledger/' 'lockcheck'; do
    grep -qEi "^guard-(allow|mention):.*$pat" "$f" 2>/dev/null && continue
    hit=''; cleared=0
    while IFS= read -r line; do
      [ -n "$line" ] || continue
      case "$line" in *[Gg]uard-[Aa]llow:*|*[Gg]uard-[Mm]ention:*) continue ;; esac
      if guard_line_is_prohibition "$line" "$pat"; then cleared=$((cleared+1)); continue; fi
      hit="$line"; break
    done <<EOF
$(grep -nEi "$pat" "$f" 2>/dev/null)
EOF
    if [ -z "$hit" ]; then
      [ "$cleared" -eq 0 ] || printf 'guard: tier-C mention of /%s/ reads as a prohibition, not a target (%s line(s)) — allowed\n' "$pat" "$cleared" >&2
      continue
    fi
    printf 'guard: SOFT-DENY /%s/ at %s\n' "$pat" "$hit" >&2
    printf 'guard:   that line names protected bookkeeping as a TARGET, not as something to stay away from.\n' >&2
    printf 'guard:   fix, best first: (1) rephrase so the path is not named at all (e.g. "stay inside your owns: set");\n' >&2
    printf 'guard:   (2) '"'"'guard-mention: %s'"'"' if the brief only refers to it — that grants nothing;\n' "$pat" >&2
    printf 'guard:   (3) '"'"'guard-allow: %s'"'"' ONLY if this brief genuinely owns and writes that path.\n' "$pat" >&2
    return 1
  done
  return 0
}

guard_deny "$BRIEF" || die "$ERR_INVARIANT" guard_refused "brief targets a denied path/secret" "rephrase so the path is not named; or 'guard-mention: <pattern>' if the brief only refers to it (grants nothing); 'guard-allow: <pattern>' only if the brief truly owns and writes it"

# ---- the budget gate (I2), before any backend runs ------------------------
GATE_OUT="$("$D/registry/features/budget/budget.sh" gate --cost "$BUDGET" --remaining "${FLEET_REMAINING_RUNWAY_MICRO_USD:-${FLEET_REMAINING_MICRO_USD:-}}" 2>&1)"; GATE_EC=$?
if [ "$GATE_EC" -ne 0 ]; then
  printf '%s\n' "$GATE_OUT" >&2
  exit "$GATE_EC"
fi

WORK="$D/var/runs/$ID"; mkdir -p "$WORK"
LOG="$WORK/out.log"
META="$WORK/meta.txt"
TS_START="$(now_iso)"

target_cleanup() {
  if [ -n "$TARGET_WORKTREE" ]; then
    [ -n "${FLEET_LEDGER:-}" ] && FLEET_LEDGER="$FLEET_LEDGER" "$D/registry/features/isolate/isolate.sh" return "$TARGET_WORKTREE" >/dev/null 2>&1 || true
    TARGET_WORKTREE=""
  fi
}

provision_target() {
  mkdir -p "$TARGET_WORKTREE/.claude/skills" "$TARGET_WORKTREE/.claude/commands" || return 1
  cp -R "$D/.claude/skills/." "$TARGET_WORKTREE/.claude/skills/" || return 1
  cp -R "$D/.claude/commands/." "$TARGET_WORKTREE/.claude/commands/" || return 1
  printf 'dispatch: target worktree=%s\n' "$TARGET_WORKTREE" >&2
  printf 'dispatch: injected .claude contents:\n' >&2
  find "$TARGET_WORKTREE/.claude" -type f -print | sort >&2
}

target_title() {
  local title
  title="$(sed -n -E 's/^#[[:space:]]+//p' "$BRIEF" | head -1)"
  [ -n "$title" ] || title="$ID"
  printf '%s' "$title"
}

# no-mistakes refuses to validate the default branch ("refusing to validate \"main\": it is the
# default branch") -- correct behaviour, since the whole point is gating a change BEFORE it reaches
# the trunk. So the work must happen on a feature branch. Two infrastructure refusals in a row hid
# the real gate from every first dispatch: an unseeded remote, then this.
ensure_target_branch() {
  local wt="$1" br cur
  cur="$(git -C "$wt" symbolic-ref --quiet --short HEAD 2>/dev/null || true)"
  br="fleet/${ID:-run}-$(date -u +%Y%m%d%H%M%S)"
  case "$cur" in
    main|master|trunk|'')
      git -C "$wt" switch -q -c "$br" 2>/dev/null || git -C "$wt" checkout -q -b "$br" 2>/dev/null || return 1
      printf 'dispatch: target work branch=%s\n' "$br" >&2 ;;
    *) printf 'dispatch: target already on feature branch=%s\n' "$cur" >&2 ;;
  esac
}

run_target_gate() {
  ensure_target_branch "${TARGET_WORKTREE:-$TARGET}" || return 1
  local gate_log="$1" intent init_ec gate_ec
  intent="$(target_title)"
  : > "$gate_log" || return 1
  (cd "$TARGET_WORKTREE" && "$NO_MISTAKES_BIN" init) >"$gate_log" 2>&1
  init_ec=$?
  if [ "$init_ec" -ne 0 ]; then
    printf 'dispatch: no-mistakes init exit=%s\n' "$init_ec" >&2
    cat "$gate_log" >&2
    return "$init_ec"
  fi
  (cd "$TARGET_WORKTREE" && "$NO_MISTAKES_BIN" axi run --intent "$intent" --yes) >"$gate_log" 2>&1
  gate_ec=$?
  printf 'dispatch: no-mistakes axi run --intent %s --yes exit=%s\n' "$(printf '%q' "$intent")" "$gate_ec" >&2
  cat "$gate_log" >&2
  return "$gate_ec"
}

WORK_BRANCH=""; WORK_COMMIT=""; WORK_CHANGED_FILES=0; WORK_LANDING="no-commit"; COMMIT_EC=0

commit_target_work() {
  local wt="$TARGET_WORKTREE" rel staged
  WORK_BRANCH="$(git -C "$wt" symbolic-ref --quiet --short HEAD 2>/dev/null || true)"
  [ -n "$WORK_BRANCH" ] || { WORK_LANDING="commit-failed"; return 1; }

  # Stage only declared output paths. Fleet-injected policy files and interpreter caches must not
  # become the agent's product commit; undeclared writes remain visible to the ownership gate.
  while IFS= read -r rel; do
    [ -n "$rel" ] || continue
    case "$rel" in */__pycache__/*|*.pyc) continue ;; esac
    path_is_owned "$rel" "$OWNS_LIST" || continue
    git -C "$wt" add -- "$rel"
    rel_ec=$?
    [ "$rel_ec" -eq 0 ] || { WORK_LANDING="commit-failed"; return "$rel_ec"; }
  done < <({ git -C "$wt" diff --name-only; git -C "$wt" diff --cached --name-only; git -C "$wt" ls-files --others --exclude-standard; } | sort -u)

  staged="$(git -C "$wt" diff --cached --name-only)"
  if [ -z "$staged" ]; then
    WORK_LANDING="no-commit"
    return 0
  fi

  git -C "$wt" commit -m "Implement $ID" -m "Co-Authored-By: Codex <noreply@openai.com>" >/dev/null
  commit_ec=$?
  [ "$commit_ec" -eq 0 ] || { WORK_LANDING="commit-failed"; return "$commit_ec"; }
  WORK_COMMIT="$(git -C "$wt" rev-parse HEAD)"
  WORK_CHANGED_FILES="$(git -C "$wt" diff-tree --no-commit-id --name-only -r "$WORK_COMMIT" | wc -l | tr -d ' ')"
  WORK_LANDING="committed"
  return 0
}

# Prompt = shared header + role context + the brief. Stdin only — never interpolated.
build_prompt() {
  [ -f "$D/docs/guides/AGENT-BRIEFING.md" ] && cat "$D/docs/guides/AGENT-BRIEFING.md"
  printf '\n\n'
  printf 'DISPATCH ROLE: %s\n\n' "$ROLE"
  cat "$BRIEF"
}

# ---- owns: parsing (shared shape with fanout.sh; duplicated per this codebase's convention of
# bin/*.sh scripts never sourcing each other, only registry/lib/*.sh) --------------------------------------
parse_owns() {
  local f="$1" line
  line="$(grep -m1 -E '^owns:[[:space:]]*' "$f" 2>/dev/null || true)"
  [ -n "$line" ] || return 0
  line="${line#*:}"
  printf '%s' "$line" | tr ',' '\n' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' | sed '/^$/d'
}

path_is_owned() {
  local rel="$1" owns_list="$2" entry
  while IFS= read -r entry; do
    [ -n "$entry" ] || continue
    case "$entry" in
      */) case "$rel" in "$entry"*) return 0 ;; esac ;;
      *) [ "$rel" = "$entry" ] && return 0 ;;
    esac
  done <<EOF
$owns_list
EOF
  return 1
}

OWNS_LIST="$(parse_owns "$BRIEF")"

# ---- stage 1: write detection by CONTENT, not by mtime -----------------------------------------
# `find -newer` answers "was this file's mtime bumped during the window", which is a strictly
# weaker claim than "this run wrote it". A rewrite with identical bytes (a formatter, a
# regenerator, a plain touch) bumps mtime and changes nothing. So the pre-run state is recorded
# as content, and the post-run comparison is a content comparison.
#
# The sweep set is pruned at the find level rather than grep-filtered afterwards. That is both
# faster (work/ holds 66 MB of backend logs on this tree and is never walked at all) and safer:
# the old filter interpolated $D straight into a grep -E pattern, where a '.' or '+' anywhere in
# the install path is a metacharacter.
HASH_MAX_KB="${FLEET_DISPATCH_HASH_MAX_KB:-1024}"
case "$HASH_MAX_KB" in ''|*[!0-9]*) die "$ERR_USAGE" bad_hash_cap "FLEET_DISPATCH_HASH_MAX_KB must be a non-negative integer number of KiB" "unset it to take the 1024 default" ;; esac

# BSD stat wants -f '%z %N', GNU stat wants -c '%s %n'. Detected once, not per file.
if stat -f '%z' "$0" >/dev/null 2>&1; then STAT_SIZE_FLAG='-f'; STAT_SIZE_FMT='%z %N'
else STAT_SIZE_FLAG='-c'; STAT_SIZE_FMT='%s %n'; fi

# In --target mode the sweep MUST walk the target worktree, not fleet's tree. Hardcoding "$D" made
# this gate structurally blind to the very repo it exists to protect: a verifier planted an
# undeclared file in the target and the dispatch still reported ownership clean.
sweep_root() { if [ -n "${TARGET_WORKTREE:-}" ]; then printf '%s' "$TARGET_WORKTREE"; else printf '%s' "$D"; fi; }
sweep_find() { # sweep_find <extra find predicates...>
  local R; R="$(sweep_root)"
  find "$R" \( -path "$R/var" -o -path "$R/ledger" -o -path "$R/.git" -o -path "$R/node_modules" \) -prune -o -type f "$@" 2>/dev/null
}

# snapshot_tree <outfile> — one record per file: "H:<sha256>\t<abs path>" for files under the hash
# cap, "S:<bytes>\t<abs path>" for the few above it. A file too big to hash is byte-count-compared
# instead, and a size-equal change to one of those is reported UNVERIFIED rather than cleared —
# the cap trades precision for time, never in the direction of dropping a write.
snapshot_tree() {
  local out="$1" root
  root="$(sweep_root)"
  : > "$out"
  sweep_find -size "-${HASH_MAX_KB}k" -print0 \
    | xargs -0 shasum -a 256 2>/dev/null \
    | awk -v d="$root/" '{h=$1; sub(/^[0-9a-f]+  /,""); if (index($0,d)==1) printf "H:%s\t%s\n", h, $0}' >> "$out"
  # Batched through one xargs, not a `wc -c` per file: at the default cap that is 20 files here,
  # but the cost has to stay sane on a tree of large files, and at HASH_MAX_KB=0 (hash nothing)
  # EVERY file lands in this branch. BSD stat and GNU stat disagree on the flag, so detect once.
  sweep_find \! -size "-${HASH_MAX_KB}k" -print0 \
    | xargs -0 stat "$STAT_SIZE_FLAG" "$STAT_SIZE_FMT" 2>/dev/null \
    | awk -v d="$root/" '{sz=$1; sub(/^[0-9]+ /,""); if (index($0,d)==1) printf "S:%s\t%s\n", sz, $0}' >> "$out"
}

# ensure_snapshot — leave a private pre-run snapshot at $SNAPSHOT, building the shared one only
# when it is missing or older than the max age.
#
# WHY SHARED, MEASURED: the first cut of this took a full-tree snapshot per dispatch. That is
# fine for one run (1.7s) and catastrophic under fanout.sh, which starts up to --max-parallel
# dispatches at once: 38 concurrent whole-tree content reads turned a 22-second test suite into
# a >10-minute one, all of them hashing the same unchanged bytes. Concurrency is exactly the
# situation this whole fix exists for, so the fix must not itself fall over under it.
#
# A slightly stale shared snapshot is still SOUND, and that is worth stating precisely rather
# than assuming: a file written between the snapshot and this run's marker has an mtime older
# than the marker, so it is never a candidate in the first place. Staleness can only affect
# files this run's own sweep already selected.
#
# The private copy is not paranoia either: without it a later dispatch could replace the shared
# file mid-run, and the post-run comparison would then read a snapshot that already contains
# this backend's own writes — silently clearing a real violation.
SNAP_SHARED="${FLEET_DISPATCH_SNAPSHOT:-$D/var/fleet/tree-snapshot.tsv}"
SNAP_LOCK="$SNAP_SHARED.lock"
SNAP_MAX_AGE="${FLEET_DISPATCH_SNAPSHOT_MAX_AGE_SEC:-300}"

_file_mtime() { stat -f %m "$1" 2>/dev/null || stat -c %Y "$1" 2>/dev/null; }

_snapshot_fresh() {
  local m now
  [ -s "$SNAP_SHARED" ] || return 1
  m="$(_file_mtime "$SNAP_SHARED")"; case "$m" in ''|*[!0-9]*) return 1 ;; esac
  now="$(date +%s)"
  [ $((now - m)) -le "$SNAP_MAX_AGE" ]
}

ensure_snapshot() {
  local n=0 lock_age lock_m now
  mkdir -p "$D/var/fleet" 2>/dev/null || true
  if ! _snapshot_fresh; then
    # A build that died holding the lock must not wedge every later dispatch (the same failure
    # registry/lib/receipt.sh already had to fix for the ledger lock), so the lock is breakable by age.
    if [ -d "$SNAP_LOCK" ]; then
      lock_m="$(_file_mtime "$SNAP_LOCK")"; now="$(date +%s)"
      case "$lock_m" in ''|*[!0-9]*) lock_m="$now" ;; esac
      lock_age=$((now - lock_m))
      # rm-rf-ok: $SNAP_LOCK is a lock directory this code computed, never user input
      [ "$lock_age" -ge 600 ] && { printf 'dispatch: breaking snapshot lock stale for %ss\n' "$lock_age" >&2; rm -rf "$SNAP_LOCK" 2>/dev/null || true; }
    fi
    if mkdir "$SNAP_LOCK" 2>/dev/null; then
      printf '%s' "$$" > "$SNAP_LOCK/pid" 2>/dev/null || true
      if ! _snapshot_fresh; then
        snapshot_tree "$SNAP_SHARED.$$" && mv -f "$SNAP_SHARED.$$" "$SNAP_SHARED" 2>/dev/null
        rm -f "$SNAP_SHARED.$$" 2>/dev/null || true
      fi
      # rm-rf-ok: $SNAP_LOCK is a lock directory this code computed, never user input
      rm -rf "$SNAP_LOCK" 2>/dev/null || true
    else
      # somebody else is building the very snapshot this run needs: wait for it rather than
      # duplicate the work. Capped, then fall through to a private build.
      while [ -d "$SNAP_LOCK" ] && [ "$n" -lt 300 ]; do sleep 0.1; n=$((n+1)); done
    fi
  fi
  # The shared cache is keyed to FLEET's tree. In --target mode the sweep root is the target
  # worktree, so a cache hit would snapshot the wrong repo -- which surfaced as "pre-run content
  # snapshot is empty: stage 1 could not run" and classed every changed path as `created`.
  if [ -z "${TARGET_WORKTREE:-}" ] && _snapshot_fresh && cp "$SNAP_SHARED" "$SNAPSHOT" 2>/dev/null; then return 0; fi
  snapshot_tree "$SNAPSHOT"
}

# snap_class <abs path> — created | modified | unchanged | unverified, against $SNAPSHOT.
# An unreadable or vanished file hashes to nothing, which compares unequal — i.e. it is reported,
# not silently cleared.
snap_class() {
  local pth="$1" was cur
  was="$(awk -F'\t' -v want="$pth" '$2==want{print $1; exit}' "$SNAPSHOT" 2>/dev/null)"
  [ -n "$was" ] || { printf 'created'; return 0; }
  case "$was" in
    H:*) cur="H:$(shasum -a 256 "$pth" 2>/dev/null | awk '{print $1}')"
         if [ "$cur" = "$was" ]; then printf 'unchanged'; else printf 'modified'; fi ;;
    S:*) cur="S:$(wc -c < "$pth" 2>/dev/null | tr -d ' ')"
         if [ "$cur" = "$was" ]; then printf 'unverified'; else printf 'modified'; fi ;;
    *)   printf 'unverified' ;;
  esac
}

class_phrase() {
  case "$1" in
    created)    printf 'created during this run' ;;
    modified)   printf 'content changed during this run' ;;
    unverified) printf "mtime moved and the file is over the ${HASH_MAX_KB}KiB hash cap, so the content change is UNVERIFIED" ;;
    *)          printf '%s' "$1" ;;
  esac
}

# ---- stage 2: the concurrent-run registry ------------------------------------------------------
# Attribution instead of guesswork. Every dispatch records its id / pid / window / declared owns:
# set under var/fleet/dispatch-runs/, which the sweep already prunes. A path this run's own backend
# log never mentions, but that another dispatch with an overlapping window had declared, is that
# run's write. This is the case fanout.sh's pre-flight owns-conflict check structurally cannot
# cover: a hand-run `dispatch.sh run`, a second fanout, or a run that started before this one.
# A record is finalised with ended_epoch on exit and garbage-collected by age, so the directory
# stays bounded; a record whose owner was killed keeps its full TTL on purpose, because a killed
# run still wrote whatever it wrote before dying.
RUNS_DIR="${FLEET_DISPATCH_RUNS_DIR:-$D/var/fleet/dispatch-runs}"
RUN_TTL_MIN="${FLEET_DISPATCH_RUN_TTL_MIN:-1440}"
RUN_FILE=""
RUN_FINALIZED=0
RUNS_OVERLAP=""

# GC by file mtime, in one find: a record's mtime IS its end time, because finalising appends
# ended_epoch to it. The earlier version read four fields per record with four subprocesses, which
# is O(records) process spawns on every single dispatch — the same shape of mistake as the
# per-dispatch snapshot above, and it showed up the same way.
runs_gc() {
  [ -d "$RUNS_DIR" ] || return 0
  find "$RUNS_DIR" -type f -mmin "+$RUN_TTL_MIN" -delete 2>/dev/null || true
  return 0
}

run_register() {
  mkdir -p "$RUNS_DIR" 2>/dev/null || { RUN_FILE=""; return 0; }
  RUN_FILE="$RUNS_DIR/$SEC_START.$$.$(printf '%s' "$ID" | tr -c 'A-Za-z0-9._-' '_')"
  { printf 'pid=%s\nid=%s\nharness=%s\nstarted_epoch=%s\n' "$$" "$ID" "$HARNESS" "$SEC_START"
    printf '%s\n' "$OWNS_LIST" | sed -e '/^$/d' -e 's/^/own=/'
  } > "$RUN_FILE" 2>/dev/null || RUN_FILE=""
  return 0
}

run_finalize() {
  [ "$RUN_FINALIZED" = 0 ] || return 0
  RUN_FINALIZED=1
  [ -n "$RUN_FILE" ] && [ -f "$RUN_FILE" ] && printf 'ended_epoch=%s\n' "$(date +%s)" >> "$RUN_FILE" 2>/dev/null
  return 0
}

# runs_overlapping — every "<id>|<pid>|<declared path>" triple from OTHER dispatch records whose
# window overlaps this one. One awk pass over the whole directory, evaluated once per dispatch
# rather than once per suspect path.
#
# Overlap, not mere existence: a run that had already finished before this one started cannot
# have written inside this window. A record with no ended_epoch is still open (or its owner was
# killed) and counts as overlapping — a killed run still wrote whatever it wrote.
runs_overlapping() {
  [ -d "$RUNS_DIR" ] || return 0
  awk -v me="$RUN_FILE" -v s="$SEC_START" -v e="$SEC_END" '
    function flush(  i) {
      if (file != "" && file != me && st != "" && st+0 <= e+0 && (en == "" || en+0 >= s+0))
        for (i = 1; i <= n; i++) print id "|" pid "|" own[i]
    }
    FILENAME != file { flush(); file=FILENAME; st=""; en=""; pid=""; id=""; n=0; for (k in own) delete own[k] }
    /^started_epoch=/ { st  = substr($0, 15) }
    /^ended_epoch=/   { en  = substr($0, 13) }
    /^pid=/           { pid = substr($0, 5) }
    /^id=/            { id  = substr($0, 4) }
    /^own=/           { own[++n] = substr($0, 5) }
    END { flush() }
  ' "$RUNS_DIR"/* 2>/dev/null
}

# owner_of_concurrent <rel path> — "<id>|<pid>" of another dispatch that declared this path.
owner_of_concurrent() {
  local rel="$1" row
  [ -n "$RUNS_OVERLAP" ] || return 0
  while IFS= read -r row; do
    [ -n "$row" ] || continue
    if path_is_owned "$rel" "${row#*|*|}"; then printf '%s' "${row%|*}"; return 0; fi
  done <<EOF
$RUNS_OVERLAP
EOF
  return 0
}

# ---- stage 3: does this run's OWN backend log corroborate the write? ---------------------------
# path > name > none. A full relative-path hit is strong evidence the backend really touched it;
# a bare filename hit is weak (README.md appears everywhere) and is labelled as weak rather than
# promoted. This only ever strengthens a finding — nothing is cleared on the strength of silence.
log_mentions() {
  local rel="$1"
  grep -qF -- "$rel" "$LOG" 2>/dev/null && { printf 'path'; return 0; }
  grep -qF -- "$(basename "$rel")" "$LOG" 2>/dev/null && { printf 'name'; return 0; }
  printf 'none'
}

if [ "$DRYRUN" = 1 ]; then
  PBYTES="$(build_prompt | wc -c | tr -d ' ')"
  TS="$(now_iso)"
  if [ "$JSON" -eq 1 ]; then
    jq -cn --arg generatedAt "$TS" --arg id "$ID" --arg harness "$HARNESS" --arg model "$MODEL" --arg role "$ROLE" \
      --argjson prompt_bytes "$PBYTES" \
      '{generatedAt:$generatedAt,id:$id,state:"dry-run",exit:0,elapsed_sec:0,tokens:0,harness:$harness,model:$model,role:$role,prompt_bytes:$prompt_bytes,guard:"passed",budget_gate:"passed"}'
  else
    toon_preamble dispatch "dry run — guard and budget gate passed, backend not invoked" "$TS"
    toon_open dispatch 1 'id,state,exit,elapsed_sec,tokens'
    toon_row "$ID" dry-run 0 0 0
    toon_open diagnostics 1 'prompt_bytes,guard,budget_gate'
    toon_row "$PBYTES" passed passed
    toon_next "drop --dry-run to execute against $HARNESS/$MODEL"
  fi
  exit 0
fi

if [ -n "$TARGET" ]; then
  require_bin "$NO_MISTAKES_BIN" 'install no-mistakes and retry'
  TARGET_JSON="$(FLEET_LEDGER="${FLEET_LEDGER:-${TMPDIR:-/tmp}/fleet-dispatch-isolate.XXXXXX}" "$D/registry/features/isolate/isolate.sh" acquire --repo "$TARGET" --holder "dispatch-$ID" --json 2>&1)"
  TARGET_ISOLATE_EC=$?
  [ "$TARGET_ISOLATE_EC" -eq 0 ] || die "$ERR_GENERIC" target_isolation_failed "target isolation failed: $TARGET_JSON" 'check treehouse and the target repository'
  TARGET_WORKTREE="$(printf '%s' "$TARGET_JSON" | jq -r '.path // empty' 2>/dev/null)"
  [ -n "$TARGET_WORKTREE" ] || die "$ERR_PARSE" target_isolation_unparseable 'target isolation returned no worktree path' 'inspect treehouse output'
  trap 'target_cleanup' EXIT
  provision_target || die "$ERR_GENERIC" target_provision_failed 'could not inject fleet .claude skills and commands into target worktree' 'inspect the leased worktree permissions'
fi

# ---- invoke the backend, wrapped in a hard wall-clock timeout -------------
MARKER="$WORK/.pre-run-marker"
SNAPSHOT="$WORK/.pre-run-snapshot"
# In --target mode the stage-1 snapshot ran before the worktree lease, so sweep_root still pointed
# at fleet and the ownership diff had no baseline for the target -- every changed path came back
# `created`. Re-take it here, now that both TARGET_WORKTREE and SNAPSHOT exist.
if [ -n "${TARGET_WORKTREE:-}" ]; then snapshot_tree "$SNAPSHOT"; fi
# Snapshot first, marker second: a file written by somebody else during the snapshot itself then
# fails the -newer test and is never a candidate, which is the harmless direction. Marker-first
# would make it a candidate whose snapshot record already held the NEW bytes.
ensure_snapshot
: > "$MARKER"

SEC_START=$(date +%s)
runs_gc
run_register
trap 'run_finalize; target_cleanup' EXIT
trap 'run_finalize; target_cleanup; exit 130' INT
trap 'run_finalize; target_cleanup; exit 143' HUP TERM
if [ -n "$TARGET_WORKTREE" ]; then cd "$TARGET_WORKTREE" || exit 1; fi
if [ "$HARNESS" = codex ]; then
  if [ "$MODEL" = codex-default ]; then
    build_prompt | PATH="$BACKEND_ENV_PATH" timeout "$TIMEOUT" "$CODEX_BIN" exec --skip-git-repo-check -s workspace-write - > "$LOG" 2>&1
  else
    build_prompt | PATH="$BACKEND_ENV_PATH" timeout "$TIMEOUT" "$CODEX_BIN" exec --skip-git-repo-check -s workspace-write -m "$MODEL" - > "$LOG" 2>&1
  fi
else
  if [ "$MODEL" = claude-default ]; then
    build_prompt | PATH="$BACKEND_ENV_PATH" timeout "$TIMEOUT" "$CLAUDE_BIN" -p --output-format json > "$LOG" 2>&1
  else
    build_prompt | PATH="$BACKEND_ENV_PATH" timeout "$TIMEOUT" "$CLAUDE_BIN" -p --model "$MODEL" --output-format json > "$LOG" 2>&1
  fi
fi
RAW_EC=$?
if [ -n "$TARGET_WORKTREE" ]; then cd "$D" || exit 1; fi
SEC_END=$(date +%s); ELAPSED=$((SEC_END-SEC_START))
TS_END="$(now_iso)"

STATE=ok
[ "$RAW_EC" -eq 0 ] || STATE=failed
[ "$RAW_EC" -eq 124 ] && STATE=timeout

# ---- token/cost extraction, backend-specific -------------------------------
_usd_to_micro() {
  local raw="${1-}" intpart fracpart frac6
  case "$raw" in ''|*[!0-9.]*|*.*.*) printf '0'; return 0 ;; esac
  intpart="${raw%%.*}"
  if [ "$raw" = "$intpart" ]; then fracpart=""; else fracpart="${raw#*.}"; fi
  [ -n "$intpart" ] || intpart=0
  while [ "${#intpart}" -gt 1 ] && [ "${intpart#0}" != "$intpart" ]; do intpart="${intpart#0}"; done
  case "$fracpart" in *[!0-9]*) fracpart="" ;; esac
  frac6="${fracpart}000000"; frac6="${frac6:0:6}"
  printf '%s' $((10#$intpart * 1000000 + 10#$frac6))
}

TOK=0; COST_MICRO=0; UNPARSEABLE=0
if [ "$HARNESS" = codex ]; then
  RAWTOK="$(grep -A1 -i '^tokens used' "$LOG" 2>/dev/null | tail -1 | tr -cd '0-9')"
  if [ -n "$RAWTOK" ]; then
    TOK="$RAWTOK"
  elif [ "$RAW_EC" -eq 0 ]; then
    UNPARSEABLE=1
  fi
else
  CJSON=""
  while IFS= read -r _line; do
    case "$_line" in
      \{*) printf '%s' "$_line" | jq -e . >/dev/null 2>&1 && CJSON="$_line" ;;
    esac
  done < "$LOG"
  if [ -n "$CJSON" ]; then
    TIN="$(printf '%s' "$CJSON" | jq -r '.usage.input_tokens // 0' 2>/dev/null)"; case "$TIN" in ''|*[!0-9]*) TIN=0 ;; esac
    TOUT="$(printf '%s' "$CJSON" | jq -r '.usage.output_tokens // 0' 2>/dev/null)"; case "$TOUT" in ''|*[!0-9]*) TOUT=0 ;; esac
    TOK=$((TIN+TOUT))
    CRAW="$(printf '%s' "$CJSON" | jq -r '.total_cost_usd // 0' 2>/dev/null)"
    COST_MICRO="$(_usd_to_micro "$CRAW")"
  elif [ "$RAW_EC" -eq 0 ]; then
    UNPARSEABLE=1
  fi
fi

if [ "$UNPARSEABLE" -eq 1 ]; then
  receipt_append "$TS_END" dispatch "$ROLE" "$ID" "$MODEL" '' "$ERR_PARSE" 0 0 0 >/dev/null || true
  { printf 'id=%s\nharness=%s\nmodel=%s\nrole=%s\nraw_exit=%s\nexit=%s\nstate=unparseable\nelapsed_sec=%s\ntokens=0\ncost_micro_usd=0\nstarted=%s\nended=%s\n' \
      "$ID" "$HARNESS" "$MODEL" "$ROLE" "$RAW_EC" "$ERR_PARSE" "$ELAPSED" "$TS_START" "$TS_END"; } > "$META"
  die "$ERR_PARSE" backend_output_unparseable "$HARNESS exited 0 but produced no parseable token/result signal in $LOG" "inspect $LOG by hand; this is treated as untrustworthy, not silently ok"
fi

TARGET_GATE_EC=""
TARGET_GATE_LOG=""
if [ -n "$TARGET_WORKTREE" ]; then
  TARGET_GATE_LOG="$WORK/no-mistakes.log"
  run_target_gate "$TARGET_GATE_LOG"
  TARGET_GATE_EC=$?
fi

# ---- ownership check: did THIS run write outside its declared owns: set? ----
# Rows are TAB-separated (path, class, reason) so a comma anywhere in a path or a reason cannot
# split a field — the previous comma-joined form could.
DEFECT_ROWS=""
DEFECT_COUNT=0
CHANGED_COUNT=0; VIOLATION_COUNT=0; CONFIRMED_VIOLATION_COUNT=0; UNCONFIRMED_COUNT=0; ATTRIBUTED_COUNT=0; TOUCHED_ONLY_COUNT=0
FIRST_UNCONFIRMED=""

add_defect() { # <rel path> <class> <reason>
  DEFECT_COUNT=$((DEFECT_COUNT+1))
  [ -z "$DEFECT_ROWS" ] || DEFECT_ROWS="$DEFECT_ROWS
"
  DEFECT_ROWS="${DEFECT_ROWS}${1}	${2}	${3}"
}

if [ -z "$OWNS_LIST" ]; then
  add_defect '-' skipped 'ownership check skipped: brief has no owns: line'
else
  # An empty snapshot would silently reclassify every candidate as "created". Say so rather than
  # let a degraded run look like a confident one.
  [ -s "$SNAPSHOT" ] || add_defect '-' degraded "pre-run content snapshot is empty ($SNAPSHOT): stage 1 could not run, so every changed path below is classed created"
  RUNS_OVERLAP="$(runs_overlapping)"
  CHANGED="$(sweep_find -newer "$MARKER" -print || true)"
  while IFS= read -r _path; do
    [ -n "$_path" ] || continue
    # Strip the SWEEP root, not fleet's root. `owns:` in a brief is relative to the repo being
    # worked on; in --target mode that is the target worktree. Stripping "$D" left an absolute
    # /var/folders/... path, which never matched a declared relative path, so every legitimately
    # owned file in a target run was reported as a violation.
    _sweep_root="$(sweep_root)"
    _rel="${_path#"$_sweep_root"/}"
    path_is_owned "$_rel" "$OWNS_LIST" && continue
    _class="$(snap_class "$_path")"

    # stage 1 — the bytes are identical, so nothing was written here, whoever bumped the mtime.
    if [ "$_class" = unchanged ]; then
      TOUCHED_ONLY_COUNT=$((TOUCHED_ONLY_COUNT+1))
      add_defect "$_rel" touched-only 'mtime moved but the bytes are identical to the pre-run snapshot: not a write'
      continue
    fi
    CHANGED_COUNT=$((CHANGED_COUNT+1))

    # stage 3 first, deliberately — this run's own log outranks attribution. A path this backend
    # named is this run's write even if a concurrent dispatch happened to declare it too.
    _ref="$(log_mentions "$_rel")"
    if [ "$_ref" = path ]; then
      VIOLATION_COUNT=$((VIOLATION_COUNT+1))
      CONFIRMED_VIOLATION_COUNT=$((CONFIRMED_VIOLATION_COUNT+1))
      add_defect "$_rel" violation "written outside declared owns set ($(class_phrase "$_class"); this run's own $HARNESS log names this path)"
      continue
    fi

    # stage 2 — somebody else's declared territory, and this run left no trace of it.
    _other="$(owner_of_concurrent "$_rel")"
    if [ -n "$_other" ]; then
      ATTRIBUTED_COUNT=$((ATTRIBUTED_COUNT+1))
      add_defect "$_rel" attributed "$(class_phrase "$_class") but declared by concurrent dispatch '${_other%%|*}' (pid ${_other##*|}) and absent from this run's $HARNESS log: not attributed to this dispatch"
      continue
    fi

    # nothing exonerates it and nothing corroborates it. Still exit 6 — but say which it is.
    VIOLATION_COUNT=$((VIOLATION_COUNT+1))
    UNCONFIRMED_COUNT=$((UNCONFIRMED_COUNT+1))
    [ -n "$FIRST_UNCONFIRMED" ] || FIRST_UNCONFIRMED="$_rel"
    case "$_ref" in
      name) _note="this run's $HARNESS log mentions the filename but never this path" ;;
      *)    _note="this run's $HARNESS log never mentions it and no concurrent dispatch declared it - UNCONFIRMED: a same-window write by a human or a non-fleet agent is indistinguishable from here" ;;
    esac
    add_defect "$_rel" violation-unconfirmed "written outside declared owns set ($(class_phrase "$_class"); $_note)"
  done <<EOF
$CHANGED
EOF
fi

TARGET_READY=0
[ -n "$TARGET_GATE_EC" ] && [ "$TARGET_GATE_EC" -eq 0 ] && TARGET_READY=1
if [ -n "$TARGET_WORKTREE" ] && [ "$RAW_EC" -eq 0 ] && [ "$VIOLATION_COUNT" -eq 0 ] && [ "$TARGET_READY" -eq 1 ]; then
  commit_target_work
  COMMIT_EC=$?
elif [ -n "$TARGET_WORKTREE" ]; then
  WORK_BRANCH="$(git -C "$TARGET_WORKTREE" symbolic-ref --quiet --short HEAD 2>/dev/null || true)"
fi
rm -f "$MARKER" 2>/dev/null || true
run_finalize

# Fatal-on-ANY-violation used to conflate two different claims: "this run wrote outside its owns
# set" (CONFIRMED -- this run's own backend log names the path) and "something changed that nobody
# claimed" (UNCONFIRMED -- a same-window write by a human or a non-fleet agent is indistinguishable
# from here; see log_mentions/owner_of_concurrent above, whose confirmation logic is unchanged by
# this). A human running e.g. `fleet memory remember` in the same window is routine on a
# single-machine local harness, not exceptional, and killing a run that already burned real tokens
# over an unconfirmed finding misattributes the human's write to the agent. Only a CONFIRMED
# violation is fatal; an unconfirmed one is still reported in full below (defect row, stderr,
# JSON/TOON) -- never silently dropped or downgraded -- it just does not, by itself, fail the run.
VIOLATED=0
[ "$CONFIRMED_VIOLATION_COUNT" -gt 0 ] && VIOLATED=1

# Anything unattributable gets said out loud on stderr too, not only in the structured block:
# fanout.sh redirects a worker's stdout+stderr into var/runs/<id>/fanout-worker.log, and the whole
# point of the 2026-08-22 incident was that a reader could not tell a real violation from a
# concurrency artefact without doing three cross-checks by hand.
if [ "$UNCONFIRMED_COUNT" -gt 0 ]; then
  if [ "$VIOLATED" -eq 0 ]; then
    printf 'dispatch: run completed with %s unconfirmed guard finding(s) -- non-fatal, reported below; not silently hidden.\n' "$UNCONFIRMED_COUNT" >&2
  fi
  printf 'dispatch: %s ownership finding(s) could not be attributed to this run. On a shared tree that can be a concurrent-writer false positive (docs/reports/CYCLE-PROOF.md, "The stage 5c incident"). Check before acting on it:\n' "$UNCONFIRMED_COUNT" >&2
  printf '  grep -nF %s var/runs/%s/%s.log            # a hit means this run really did write it\n' "'$FIRST_UNCONFIRMED'" "$ID" "$HARNESS" >&2
  printf '  ls -la var/fleet/dispatch-runs/ && grep dispatch ledger/RECEIPTS.jsonl | tail -5   # who else was running\n' >&2
fi

FINAL_EC=0
if [ "$VIOLATED" -eq 1 ]; then
  FINAL_EC="$ERR_INVARIANT"
elif [ "$STATE" != ok ]; then
  FINAL_EC=1
elif [ -n "$TARGET_GATE_EC" ] && [ "$TARGET_GATE_EC" -ne 0 ]; then
  FINAL_EC=1
elif [ "$COMMIT_EC" -ne 0 ]; then
  FINAL_EC=1
fi

if [ -n "$TARGET" ]; then
  "$D/registry/services/dispatch/record-run.sh" --id "$ID" --brief "$BRIEF" --out "$LOG" \
    --harness "$HARNESS" --model "$MODEL" --role "$ROLE" --exit "$FINAL_EC" \
    --started "$TS_START" --ended "$TS_END" --target "$TARGET" --tokens "$TOK" \
    --cost-micro-usd "$COST_MICRO" --raw-exit "$RAW_EC" --gate-exit "${TARGET_GATE_EC:-0}" \
    --elapsed "$ELAPSED" >/dev/null
  RECORD_EC=$?
  [ "$RECORD_EC" -eq 0 ] || { printf 'dispatch: record-run exit=%s\n' "$RECORD_EC" >&2; FINAL_EC=1; }
else
  {
    printf 'id=%s\nharness=%s\nmodel=%s\nrole=%s\nraw_exit=%s\nexit=%s\nstate=%s\nelapsed_sec=%s\ntokens=%s\ncost_micro_usd=%s\nstarted=%s\nended=%s\nunconfirmed_guard_findings=%s\n' \
      "$ID" "$HARNESS" "$MODEL" "$ROLE" "$RAW_EC" "$FINAL_EC" "$STATE" "$ELAPSED" "$TOK" "$COST_MICRO" "$TS_START" "$TS_END" "$UNCONFIRMED_COUNT"
  } > "$META"
fi

receipt_append "$TS_END" dispatch "$ROLE" "$ID" "$MODEL" '' "$FINAL_EC" 0 "$TOK" "$COST_MICRO" >/dev/null || true
if [ "$VIOLATED" -eq 1 ]; then
  receipt_append "$TS_END" ownership_violation "$ROLE" "$ID" "$MODEL" '' "$ERR_INVARIANT" 0 0 0 >/dev/null || true
fi

if [ "$JSON" -eq 1 ]; then
  DEFECTS_JSON="$(printf '%s\n' "$DEFECT_ROWS" | sed '/^$/d' | jq -R -c 'split("\t") | {path:.[0], class:.[1], reason:.[2]}' | jq -cs '.')"
  [ -n "$DEFECTS_JSON" ] || DEFECTS_JSON='[]'
  jq -cn --arg generatedAt "$TS_END" --arg id "$ID" --arg state "$STATE" --argjson exit "$FINAL_EC" \
    --argjson elapsed "$ELAPSED" --argjson tokens "$TOK" --arg harness "$HARNESS" --arg model "$MODEL" --arg role "$ROLE" \
    --argjson cost "$COST_MICRO" --argjson defects "$DEFECTS_JSON" \
    --argjson changed "$CHANGED_COUNT" --argjson violations "$VIOLATION_COUNT" \
    --argjson confirmed "$CONFIRMED_VIOLATION_COUNT" --argjson unconfirmed "$UNCONFIRMED_COUNT" --argjson attributed "$ATTRIBUTED_COUNT" \
    --argjson touched_only "$TOUCHED_ONLY_COUNT" --argjson hash_max_kb "$HASH_MAX_KB" \
    --arg work_branch "$WORK_BRANCH" --arg work_commit "$WORK_COMMIT" \
    --arg work_status "$WORK_LANDING" --argjson work_changed_files "$WORK_CHANGED_FILES" \
    '{generatedAt:$generatedAt,id:$id,state:$state,exit:$exit,elapsed_sec:$elapsed,tokens:$tokens,harness:$harness,model:$model,role:$role,cost_micro_usd:$cost,defects:$defects,ownership:{changed_outside_owns:$changed,violations:$violations,confirmed:$confirmed,unconfirmed:$unconfirmed,attributed_elsewhere:$attributed,touched_only:$touched_only,hash_max_kb:$hash_max_kb},work_landed:{branch:$work_branch,commit:$work_commit,status:$work_status,changed_files:$work_changed_files}}'
else
  printf 'work_landed: branch=%s commit=%s status=%s changed_files=%s\n' \
    "$WORK_BRANCH" "$WORK_COMMIT" "$WORK_LANDING" "$WORK_CHANGED_FILES"
  [ "$UNCONFIRMED_COUNT" -gt 0 ] && printf 'guard: completed with %s unconfirmed guard finding(s) (non-fatal; see defect rows below)\n' "$UNCONFIRMED_COUNT"
  toon_preamble dispatch "one brief dispatched to $HARNESS" "$TS_END"
  toon_open dispatch 1 'id,state,exit,elapsed_sec,tokens'
  toon_row "$ID" "$STATE" "$FINAL_EC" "$ELAPSED" "$TOK"
  if [ "$DEFECT_COUNT" -eq 0 ]; then
    toon_empty defect
  else
    toon_open defect "$DEFECT_COUNT" 'path,class,reason'
    while IFS=$'\t' read -r _p _c _r; do
      [ -n "${_p:-}" ] || continue
      toon_row "$_p" "$_c" "$_r"
    done <<EOF
$DEFECT_ROWS
EOF
  fi
  if [ "$UNCONFIRMED_COUNT" -gt 0 ]; then
    toon_next "log: var/runs/$ID/out.log" "meta: var/runs/$ID/meta.txt" \
      "UNCONFIRMED findings above: grep -nF '$FIRST_UNCONFIRMED' var/runs/$ID/out.log — no hit means this run may not be the writer" \
      "who else was running: ls var/fleet/dispatch-runs/ ; pre-run content snapshot: var/runs/$ID/.pre-run-snapshot"
  else
    toon_next "log: var/runs/$ID/out.log" "meta: var/runs/$ID/meta.txt"
  fi
fi

exit "$FINAL_EC"
