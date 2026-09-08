#!/usr/bin/env bash
# M9 — every non-zero exit must write at least one line naming the reason.
# Driving fleet by hand (docs/delta.d/opus-walkthrough.md, B8) found three silent failure paths
# the suite never caught, because the suite only asserted exit codes and the exit codes were
# right: `fleet run --task "" --repo . --agent stub` exited 7 with ZERO bytes on stdout+stderr;
# `fleet ledger verify` on a tampered chain exited 8 with NO output; `fleet plan` could hit an
# environment fault after already having decided its answer. This is the D58/D59/D60 class again
# -- the layer between working code and the person using it, invisible to a check that only reads
# `$?`. This detector drives every refusable surface it can reach through the real binary and
# asserts every non-zero exit produced at least one line, on either stream. It publishes the
# denominator: how many distinct refusal surfaces it actually triggered (exit != 0), not merely
# how many it tried -- a surface this script could not make refuse counts against the detector,
# never as a silent pass (AGENTS.md #6).
set -u
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
# B18b: honor the same FLEET_BIN override verify.sh derives from CARGO_TARGET_DIR (M2 moved the
# build out of the repo; without this M9 would silently exit 77 -- excluded, not checked -- every
# time CARGO_TARGET_DIR is set, the exact AGENTS.md-recommended worktree convention).
B="${FLEET_BIN:-$ROOT/keel/target/debug/fleet}"
[ -x "$B" ] || exit 77

STATE="$(mktemp -d "${TMPDIR:-/tmp}/fleet-m9.XXXXXX")" || exit 3
TARGET="$(mktemp -d "${TMPDIR:-/tmp}/fleet-m9-target.XXXXXX")" || exit 3
cleanup() { rm -rf "$STATE" "$TARGET"; }
trap cleanup EXIT

export FLEET_STATE="$STATE"
export FLEET_SOW_BYPASS=1
( cd "$TARGET" && git init -q && printf 'fn main(){}\n' > main.rs \
  && git add -A && git -c user.email=t@t -c user.name=t commit -qm init ) || exit 3

# Seed one real, valid ledger row so `ledger verify` and friends have a genuine chain to work
# against (rather than the vacuous empty-ledger case, which is a different, already-refused
# surface covered separately below).
"$B" run --task "m9-seed" --repo "$TARGET" --agent stub >/dev/null 2>&1
( cd "$TARGET" && git reset --hard -q HEAD 2>/dev/null; git clean -fdq 2>/dev/null ) || true

triggered=0
silent=0
declare -a SILENT_CASES=()

# check_case <label> -- args... : run the binary, and if it exits non-zero, assert the combined
# stdout+stderr is non-empty. Exit-0 invocations are printed as informational only -- they are not
# refusal surfaces on this invocation of the environment and are not counted in the denominator
# (AGENTS.md #6: a check must not inflate its own denominator with inputs that prove nothing).
check_case() {
  label="$1"
  shift
  out=$("$B" "$@" 2>&1)
  rc=$?
  if [ "$rc" -eq 0 ]; then
    echo "M9: (skip, exited 0) $label"
    return
  fi
  triggered=$((triggered + 1))
  if [ -z "$out" ]; then
    echo "M9: SILENT rc=$rc for: $label"
    silent=$((silent + 1))
    SILENT_CASES+=("$label (rc=$rc)")
  else
    echo "M9: ok rc=$rc, $(printf '%s' "$out" | wc -l | tr -d ' ') line(s): $label"
  fi
}

# --- run: every documented refusal surface B8 and D29/D40/D53 already named -------------------
check_case "run --task empty"                       run --task "" --repo "$TARGET" --agent stub
check_case "run missing --task"                     run --repo "$TARGET" --agent stub
check_case "run missing --repo"                     run --task "m9-x" --agent stub
check_case "run missing --agent/--role"              run --task "m9-x" --repo "$TARGET"
check_case "run --repo does not exist"               run --task "m9-x" --repo "$TARGET/does-not-exist" --agent stub
check_case "run --repo is not a git repo"            run --task "m9-x" --repo "$STATE" --agent stub
check_case "run unknown --agent"                     run --task "m9-x" --repo "$TARGET" --agent bogus-agent
check_case "run --task with no value"                run --task
check_case "run unknown flag"                        run --bogus-flag value
check_case "run unknown --role"                      run --task "m9-x" --repo "$TARGET" --role bogus-role

# --- ledger: the tampered-chain path B8 named directly ----------------------------------------
check_case "ledger verify (good chain)"               ledger verify
cp "$STATE/ledger/chain.jsonl" "$STATE/ledger/chain.jsonl.m9-good"
python3 - "$STATE/ledger/chain.jsonl" <<'PY'
import json, sys
path = sys.argv[1]
with open(path, encoding="utf-8") as stream:
    rows = [json.loads(line) for line in stream if line.strip()]
rows[0]["hash"] = "blake3:" + ("0" * 64)
with open(path, "w", encoding="utf-8") as stream:
    for row in rows:
        stream.write(json.dumps(row, separators=(",", ":")) + "\n")
PY
check_case "ledger verify (tampered chain)"           ledger verify
mv "$STATE/ledger/chain.jsonl.m9-good" "$STATE/ledger/chain.jsonl"
check_case "ledger unknown subcommand"                ledger bogus-subcommand

# --- plan: the front door; empty/unmatched prompts, both refusal shapes -----------------------
check_case "plan empty prompt"                        plan
check_case "plan unmatched intent"                    plan "zzz-no-such-intent-will-ever-match-zzz"

# --- sow / swarm / status / role-check / contract / mcp / graph / oracle / adjudicate / --------
# --- completions / roles / skills / lifecycle: usage-refusal surfaces reachable with no repo ---
check_case "sow invalid args"                         sow
check_case "sow accept unknown id"                    sow accept --id does-not-exist
check_case "swarm dispatch no args"                   swarm dispatch
check_case "swarm invalid args"                       swarm bogus-verb
check_case "swarm dispatch empty task"                swarm dispatch --task "" --repo "$TARGET"
check_case "swarm dispatch bad repo"                  swarm dispatch --task "m9-x" --repo "$TARGET/does-not-exist"
check_case "status invalid args"                       status --bogus
check_case "role-check invalid args"                   role-check
check_case "contract invalid args"                     contract bogus
check_case "mcp no args"                               mcp
check_case "graph no args"                             graph
check_case "oracle no args"                            oracle
check_case "adjudicate no args"                        adjudicate
check_case "completions unknown shell"                 completions bogus-shell
check_case "roles invalid args"                        roles --bogus
check_case "skills invalid args"                       skills --bogus
check_case "lifecycle unknown verb"                    lifecycle bogus-verb
check_case "unknown top-level command"                 totally-bogus-command

echo "M9: $((triggered - silent)) of $triggered triggered refusal surfaces printed a reason (denominator: $triggered)"
if [ "$triggered" -eq 0 ]; then
  echo "M9: triggered zero refusal surfaces -- measuring nothing is a failure"
  exit 1
fi
if [ "$silent" -gt 0 ]; then
  echo "M9: ${#SILENT_CASES[@]} silent refusal(s):"
  for c in "${SILENT_CASES[@]}"; do echo "    - $c"; done
  exit 1
fi
exit 0
