#!/usr/bin/env bash

# S4b prompter-parity DATA COLLECTION ONLY. This script deliberately does not run TOST or make an
# equivalence claim. The committed task pairs live in tests/parity/tasks.json. The mechanical
# output-shaped rubric is committed in tests/parity/rubric.json and is implemented below. Its ten
# checks consume the frozen unified diff only. The former kernel-shaped rubric is retained as
# rubric.ceiling.json so the pilot refusal remains reproducible.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FLEET="${FLEET_BIN:-$ROOT/keel/target/debug/fleet}"
TASKS="$ROOT/tests/parity/tasks.json"
RUBRIC="${PARITY_RUBRIC:-$ROOT/tests/parity/rubric.json}"
AGENT="${PARITY_AGENT:-stub}"
# Declared HERE, not beside the other counters further down: the pilot phase calls
# score_observation before that point, and `set -u` turned the first skipped run into an
# unbound-variable abort instead of the message it was supposed to print.
skipped=0
OUTPUT="$ROOT/var/parity-observations.jsonl"
SMOKE=0
EXPECTED=128

usage() {
  printf '%s\n' "Usage: bin/parity-run.sh [--smoke] [--output PATH]"
  printf '%s\n' "  --smoke       collect 2 prompters x 1 task x 2 runs = 4 observations"
  printf '%s\n' "  --output PATH append JSONL observations (default: var/parity-observations.jsonl)"
  printf '%s\n' "  PARITY_RUBRIC selects a rubric; PARITY_AGENT selects the fleet agent (defaults: committed rubric, stub)"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --smoke)
      SMOKE=1
      shift
      ;;
    --output)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 7
      fi
      OUTPUT="$2"
      # T2: `shift 2` with fewer than 2 args left silently no-ops and the loop spins forever,
      # ignoring SIGTERM. 25 of 94 flags hung this way once. Check cardinality first.
      [ $# -ge 2 ] || { echo "parity: --output requires a value" >&2; exit 7; }
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 7
      ;;
  esac
done

if [ ! -x "$FLEET" ]; then
  printf 'parity: environment fault: fleet is not built at %s\n' "$FLEET" >&2
  exit 3
fi
for tool in git python3 stat mktemp; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'parity: environment fault: required tool is absent: %s\n' "$tool" >&2
    exit 3
  fi
done
if [ ! -r "$TASKS" ] || [ ! -r "$RUBRIC" ]; then
  printf 'parity: invariant violation: committed parity fixtures are unavailable\n' >&2
  exit 6
fi

fixture_status=$(python3 - "$TASKS" "$RUBRIC" <<'PY'
import json
import sys

tasks = json.load(open(sys.argv[1], encoding="utf-8"))
rubric = json.load(open(sys.argv[2], encoding="utf-8"))
assert len(tasks) == 8
ids = [item["id"] for item in rubric["items"]]
assert ids in (
    ["diff_applies", "artifact_frozen", "attestation_verifies", "receipt_written"],
    ["implementation_present", "tests_present", "assertions_present",
     "both_directions_asserted", "boundary_case_named", "empty_or_null_handled",
     "typed_error_path", "no_panic_shortcut", "no_placeholder", "scope_is_focused"],
)
for item in tasks:
    assert set(item) == {"task", "terse", "detailed"}
    assert all(isinstance(item[key], str) and item[key].strip() for key in item)
    assert "\t" not in item["task"] and "\n" not in item["task"]
    assert "\t" not in item["terse"] and "\n" not in item["terse"]
    assert "\t" not in item["detailed"] and "\n" not in item["detailed"]
print("ok")
PY
)
fixture_rc=$?
if [ "$fixture_rc" -ne 0 ] || [ "$fixture_status" != "ok" ]; then
  printf 'parity: invariant violation: fixtures must contain 8 paired tasks and a recognized rubric\n' >&2
  exit 6
fi

output_parent=$(dirname "$OUTPUT")
if ! mkdir -p "$output_parent"; then
  printf 'parity: environment fault: cannot create output directory: %s\n' "$output_parent" >&2
  exit 3
fi
if ! touch "$OUTPUT"; then
  printf 'parity: environment fault: cannot append output: %s\n' "$OUTPUT" >&2
  exit 3
fi

experiment_dir=$(mktemp -d "${TMPDIR:-/tmp}/fleet-parity.XXXXXX")
experiment_rc=$?
if [ "$experiment_rc" -ne 0 ] || [ -z "$experiment_dir" ]; then
  printf 'parity: environment fault: cannot create experiment directory\n' >&2
  exit 3
fi
cleanup() {
  rm -rf "$experiment_dir"
}
trap cleanup EXIT
trap 'exit 130' HUP INT TERM

task_limit=8
run_limit=8
if [ "$SMOKE" -eq 1 ]; then
  task_limit=1
  run_limit=2
fi

task_rows="$experiment_dir/tasks.tsv"
if ! python3 - "$TASKS" > "$task_rows" <<'PY'
import json
import sys

tasks = json.load(open(sys.argv[1], encoding="utf-8"))
for item in tasks:
    print("\t".join((item["task"], item["terse"], item["detailed"])))
PY
then
  printf 'parity: invariant violation: could not read task fixtures\n' >&2
  exit 6
fi

rubric_kind=$(python3 - "$RUBRIC" <<'PY'
import json, sys
ids = [item["id"] for item in json.load(open(sys.argv[1], encoding="utf-8"))["items"]]
print("ceiling" if ids[0] == "diff_applies" else "output")
PY
)

score_observation() {
  local artifact="$1" repo="$2" state="$3" artifact_id="$4"
  if [ "$rubric_kind" = "output" ]; then
    python3 - "$artifact" <<'PY'
import re
import sys

lines = open(sys.argv[1], encoding="utf-8", errors="replace").read().splitlines()
files = []
current = ""
added = []
for line in lines:
    if line.startswith("+++ b/"):
        current = line[6:]
        files.append(current)
    elif line.startswith("+") and not line.startswith("+++"):
        added.append((current, line[1:]))

def test_path(path):
    low = path.lower()
    return bool(re.search(r"(^|/)(tests?|specs?)(/|$)|(?:^|[._-])(test|spec)[._-]", low))

test_lines = [text for path, text in added if test_path(path)]
all_added = [text for _, text in added]
inline_test = any(re.search(r"#\s*\[\s*(?:cfg\s*\(\s*test\s*\)|test)\s*\]|\bdef\s+test_|\bit\s*\(|\btest\s*\(", text, re.I) for text in all_added)
if inline_test and not test_lines:
    test_lines = all_added
impl_lines = [text for path, text in added if not test_path(path)]
substantive = [text for text in impl_lines if text.strip() and not re.match(r"\s*(?://|#|/\*|\*|<!--)", text)]
test_text = "\n".join(test_lines)
impl_text = "\n".join(impl_lines)
all_text = "\n".join(all_added)
tests_present = bool(test_lines)
has_assert = bool(re.search(r"\bassert(?:ion)?\b|assert[_!(]|expect\s*\(|should\s*(?:be|equal)|==|!=", test_text, re.I))
has_success = bool(re.search(r"\b(success|succeeds?|accept(?:ed|s)?|valid|ok|happy|positive)\b", test_text, re.I))
has_failure = bool(re.search(r"\b(fail(?:s|ure|ed)?|reject(?:ed|s)?|invalid|error|err|negative|nonzero)\b", test_text, re.I))
boundary = bool(re.search(r"\b(empty|null|none|nil|missing|zero|min(?:imum)?|max(?:imum)?|boundary|absent|invalid|malformed)\b", test_text, re.I))
empty_handled = bool(re.search(r"(?:\.is_empty\s*\(|\b(?:empty|null|none|nil|absent)\b|len\s*\([^)]*\)\s*==\s*0|==\s*[\"']{2})", impl_text, re.I))
typed_error = bool(re.search(r"\b(Result|Option|Error|Exception|ExitCode|StatusCode|EXIT_[A-Z0-9_]+)\b|->\s*[^\n]*(?:Result|Option)|\berr\s*\(", impl_text))
panic_shortcut = bool(re.search(r"\b(?:panic!|unimplemented!|todo!|throw\s+[^\n;]+|unwrap\s*\(|expect\s*\()", impl_text, re.I))
placeholder = bool(re.search(r"\b(?:TODO|FIXME|XXX|HACK|placeholder|not\s+implemented)\b", all_text, re.I))
banned = any(re.search(r"(^|/)(?:contracts|vendor|node_modules|tests/acceptance)(/|$)|(^|/)(?:Cargo\.lock|package-lock\.json|yarn\.lock|pnpm-lock\.yaml)$|^\.github/", path) for path in files)
checks = [
    bool(substantive), tests_present, tests_present and has_assert,
    tests_present and has_assert and has_success and has_failure,
    tests_present and boundary, empty_handled, typed_error,
    not panic_shortcut, not placeholder, 0 < len(set(files)) <= 4 and not banned,
]
print(sum(checks))
PY
    return
  fi

  local score=0 mode mode_rc
  if (cd "$repo" && git restore --worktree . >/dev/null 2>&1 && git apply --check "$artifact" >/dev/null 2>&1); then
    score=$((score + 1))
  fi
  mode=$(stat -f '%Lp' "$artifact" 2>/dev/null || stat -c '%a' "$artifact" 2>/dev/null)
  mode_rc=$?
  if [ "$mode_rc" -eq 0 ] && [ "$mode" = "444" ]; then score=$((score + 1)); fi
  if FLEET_STATE="$state" "$FLEET" attest verify "$artifact_id" >/dev/null 2>&1; then score=$((score + 1)); fi
  if python3 - "$state/attestations/$artifact_id.json" "$state/ledger/chain.jsonl" >/dev/null 2>&1 <<'PY'
import json, sys
attestation = json.load(open(sys.argv[1], encoding="utf-8"))
with open(sys.argv[2], encoding="utf-8") as f:
    rows = [json.loads(line) for line in f if line.strip()]
named = attestation["predicate"]["receipts"]
recorded = {row["hash"] for row in rows}
assert named and all(receipt in recorded for receipt in named)
PY
  then score=$((score + 1)); fi
  printf '%d\n' "$score"
}

collect_one() {
  local destination="$1" prompter="$2" task_id="$3" run="$4" prompt="$5"
  local observation_dir repo state setup_rc score artifact_id run_output run_rc artifact
  observation_dir=$(mktemp -d "$experiment_dir/observation.XXXXXX") || return 3
  repo="$observation_dir/repo"
  state="$observation_dir/state"
  mkdir -p "$repo" "$state" || return 3
  (
    cd "$repo" || exit 3
    git init -q || exit 3
    git config user.email parity@example.invalid || exit 3
    git config user.name fleet-parity || exit 3
    printf '%s\n' 'fn main() {}' > main.rs || exit 3
    git add main.rs || exit 3
    git commit -qm initial || exit 3
  ) || return 3
  # D48: `score=0` as the DEFAULT made a FAILED run indistinguishable from a run that produced a
  # worthless diff. Every rate-limited or timed-out invocation was silently recorded as a genuine
  # zero, which is exactly the "zero inflation" D33 (77%) and D37 (40%) blamed on the agent -- the
  # observations were not bad, they were ABSENT. Same null-is-not-zero rule as the meter.
  score=""
  artifact_id=""
  # D55: this harness predates the SOW gate (D39). Without the bypass every observation was
  # refused with "task has no accepted SOW" and -- thanks to D48 -- correctly SKIPPED, so the
  # experiment collected nothing and refused. The bypass is right here: S4b measures whether two
  # PROMPTERS produce the same output, not whether the planning gate works, and the gate has its
  # own assertions (S5-S9). The bypass is recorded in every receipt, so these runs stay auditable.
  # D56: pace the calls. Sequential runs succeed and twelve rapid ones do not -- the free lane
  # rate-limits and the adapter returns no fd-3 result (D55). Pacing is the honest fix for a rate
  # limit; retrying harder is not. Default 6s between observations, override with PARITY_PACE_S.
  # A paid lane can set PARITY_PACE_S=0.
  sleep "${PARITY_PACE_S:-6}"
  run_output=$(FLEET_SOW_BYPASS=1 FLEET_STATE="$state" "$FLEET" run --task "$prompt" --repo "$repo" --agent "$AGENT" 2>&1)
  run_rc=$?
  if [ "$run_rc" -eq 0 ] && [[ "$run_output" =~ artifact=([0-9a-f]{64}) ]]; then artifact_id="${BASH_REMATCH[1]}"; fi
  if [ -n "$artifact_id" ]; then
    artifact="$state/artifacts/$artifact_id"
    score=$(score_observation "$artifact" "$repo" "$state" "$artifact_id") || return 6
  fi
  if [ -z "$score" ]; then
    # The run did not complete. Record the ABSENCE and skip the observation rather than
    # inventing a zero: an experiment cannot average over runs that never happened.
    printf 'parity: run failed (rc=%s) — observation SKIPPED, not scored 0\n' "$run_rc" >&2
    # Print WHY. A skipped observation with no reason is a dead end for whoever debugs the next
    # empty experiment -- exactly the D40 class, inside the harness rather than the product.
    printf '%s\n' "$run_output" | grep -viE 'sow bypass' | head -2 | sed 's/^/  parity:   /' >&2
    skipped=$((skipped + 1))
    return 0
  fi
  python3 - "$destination" "$prompter" "$task_id" "$run" "$score" <<'PY'
import json, sys
row = {"prompter":sys.argv[2], "task":sys.argv[3], "run":int(sys.argv[4]), "score":int(sys.argv[5])}
with open(sys.argv[1], "a", encoding="utf-8") as output:
    output.write(json.dumps(row, separators=(",", ":")) + "\n")
PY
}

# Precondition: use both prompt styles across three tasks, twice each. These 12 observations are
# deliberately separate from the registered experiment and are discarded after calibration.
pilot_output="$experiment_dir/pilot.jsonl"
pilot_tasks=0
while IFS="$(printf '\t')" read -r task_id terse_prompt detailed_prompt; do
  pilot_tasks=$((pilot_tasks + 1))
  [ "$pilot_tasks" -le 3 ] || break
  for prompter in terse detailed; do
    if [ "$prompter" = terse ]; then prompt="$terse_prompt"; else prompt="$detailed_prompt"; fi
    for run in 1 2; do
      collect_one "$pilot_output" "$prompter" "$task_id" "$run" "$prompt"
      collect_rc=$?
      if [ "$collect_rc" -ne 0 ]; then
        printf 'parity: pilot observation failed with typed exit %d\n' "$collect_rc" >&2
        exit "$collect_rc"
      fi
    done
  done
done < "$task_rows"
pilot_summary=$(python3 - "$pilot_output" <<'PY'
import json, sys
# D56: the pilot demanded EXACTLY 12 scoreable observations, so a single skipped run aborted the
# whole experiment with a bare AssertionError. D48 made skips the correct response to a failed
# run; this assertion had not caught up. A pilot needs ENOUGH observations to judge variance, not
# a fixed count -- require a floor and report the shortfall instead of crashing.
scores = [json.loads(line)["score"] for line in open(sys.argv[1], encoding="utf-8") if line.strip()]
FLOOR = 6
if len(scores) < FLOOR:
    print(f"pilot collected {len(scores)} of 12 scoreable observations; {FLOOR} is the floor",
          file=sys.stderr)
    raise SystemExit(1)
print(f"{len(scores)}\t{min(scores)}\t{max(scores)}")
PY
)
pilot_summary_rc=$?
if [ "$pilot_summary_rc" -ne 0 ] || [ -z "$pilot_summary" ]; then
  printf 'parity: invariant violation: pilot collected too few scoreable observations to judge\n' >&2
  printf 'parity:   the skip reasons above say why. A rate-limited lane needs PARITY_PACE_S raised.\n' >&2
  exit 6
fi
IFS="$(printf '\t')" read -r pilot_n pilot_min pilot_max <<EOF
$pilot_summary
EOF
if [ "$pilot_min" -eq "$pilot_max" ]; then
  printf 'rubric does not discriminate: all %d pilot observations scored %d\n' "$pilot_n" "$pilot_min" >&2
  exit 6
fi
printf 'parity: pilot variance precondition passed: N=%d min=%d max=%d\n' "$pilot_n" "$pilot_min" "$pilot_max"

collected=0
skipped=0
task_count=0
while IFS="$(printf '\t')" read -r task_id terse_prompt detailed_prompt; do
  task_count=$((task_count + 1))
  [ "$task_count" -le "$task_limit" ] || break
  for prompter in terse detailed; do
    if [ "$prompter" = "terse" ]; then
      prompt="$terse_prompt"
    else
      prompt="$detailed_prompt"
    fi
    run=1
    while [ "$run" -le "$run_limit" ]; do
      collect_one "$OUTPUT" "$prompter" "$task_id" "$run" "$prompt"
      collect_rc=$?
      if [ "$collect_rc" -ne 0 ]; then
        printf 'parity: observation failed with typed exit %d\n' "$collect_rc" >&2
        exit "$collect_rc"
      fi
      collected=$((collected + 1))
      # D37: a CUMULATIVE zero-rate guard, checked mid-collection. The pilot guard passed
      # (N=12, min 0, max 5) and the interim rate at N=60 was 15%, but by N=128 it was 40%:
      # lanes degrade under sustained load as rate limits accumulate, so a sample from the FRONT
      # of a run does not predict the run. Abort as soon as the rate crosses the threshold rather
      # than discovering it in the analysis, and always publish the rate.
      if [ "$collected" -ge "${PARITY_ZERO_MIN_N:-24}" ]; then
        # BOTH numbers must come from the SAME source. The first draft took the numerator from
        # the file and the denominator from the loop counter, which reported "200% (4 of 2)" --
        # the file already held pilot rows the counter never saw. A guard whose numerator and
        # denominator disagree is worse than no guard.
        zero_n=$(awk -F'"score":' 'NF>1{split($2,a,/[,}]/); if (a[1]+0==0) c++} END{print c+0}' "$OUTPUT")
        total_n=$(awk -F'"score":' 'NF>1{c++} END{print c+0}' "$OUTPUT")
        [ "${total_n:-0}" -gt 0 ] || total_n=1
        zero_pct=$(( zero_n * 100 / total_n ))
        if [ "$zero_pct" -gt "${PARITY_ZERO_MAX_PCT:-30}" ]; then
          printf 'parity: REFUSE: cumulative zero rate %d%% (%d of %d) exceeds %d%% -- the agent is
' \
            "$zero_pct" "$zero_n" "$total_n" "${PARITY_ZERO_MAX_PCT:-30}" >&2
          printf '  failing too often for this to measure prompters rather than availability (D37).
' >&2
          printf '  Use a reliable agent, or raise PARITY_ZERO_MAX_PCT deliberately and say so.
' >&2
          exit 6
        fi
      fi
      run=$((run + 1))
    done
  done
done < "$task_rows"

printf 'parity: skipped (run failed, not scored): %d\n' "$skipped"
printf 'parity: observations collected / expected: %d / %d' "$collected" "$EXPECTED"
if [ "$SMOKE" -eq 1 ]; then
  printf ' (smoke target: 4)'
fi
printf '\n'

if [ "$collected" -eq 0 ]; then
  printf 'parity: invariant violation: collected zero observations\n' >&2
  exit 6
fi
if [ "$SMOKE" -eq 1 ] && [ "$collected" -ne 4 ]; then
  printf 'parity: invariant violation: smoke collection expected 4 observations, got %d\n' "$collected" >&2
  exit 6
fi
if [ "$SMOKE" -eq 0 ] && [ "$collected" -ne "$EXPECTED" ]; then
  printf 'parity: invariant violation: full collection expected %d observations, got %d\n' "$EXPECTED" "$collected" >&2
  exit 6
fi
