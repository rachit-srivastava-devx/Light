#!/usr/bin/env bash
# The eight-state, tool-backed SDLC floor. It is intentionally an adapter: tools own analysis,
# this file owns ordering, empty-input refusal, result vocabulary, and receipts.
set -o pipefail

SELF_DIR="$(cd "$(dirname "$0")" && pwd -P)"
FLEET_ROOT="$(cd "$SELF_DIR/../../.." && pwd -P)"
THRESHOLDS="$SELF_DIR/thresholds.conf"
PIPELINE_NAME=sdlc-pipeline
TMP_ROOT="$TMPDIR"
[ -n "$TMP_ROOT" ] || TMP_ROOT=/tmp
. "$FLEET_ROOT/registry/lib/receipt.sh"

if [ -x "$FLEET_ROOT/var/venv/bin/python" ]; then
  PATH="$PATH:$FLEET_ROOT/var/venv/bin"
fi
export PATH

usage() {
  printf '%s\n' \
    'pipeline.sh run --repo PATH' \
    '  states: graph edge-cases code test quality security perf maintenance' \
    '  FLEET_LEDGER must be a temporary path when receipts are required'
}

now_iso() { date -u +%Y-%m-%dT%H:%M:%SZ; }

require_tool() {
  local tool install
  tool="$1"
  install="$2"
  command -v "$tool" >/dev/null 2>&1 && return 0
  printf 'missing tool: %s\ninstall: %s\n' "$tool" "$install" >&2
  return 3
}

require_any_tool() {
  local first second install
  first="$1"
  second="$2"
  install="$3"
  if command -v "$first" >/dev/null 2>&1; then
    printf '%s' "$first"
    return 0
  fi
  if command -v "$second" >/dev/null 2>&1; then
    printf '%s' "$second"
    return 0
  fi
  printf 'missing tool: %s or %s\ninstall: %s\n' "$first" "$second" "$install" >&2
  return 3
}

threshold_value() {
  local key
  key="$1"
  awk -F= -v wanted="$key" '$1 !~ /^[[:space:]]*#/ && $1 == wanted {gsub(/[[:space:]]/, "", $2); print $2; exit}' "$THRESHOLDS"
}

thresholds_ok() {
  local ml_error ml_warning ccn over dup perf delta ec
  [ -s "$THRESHOLDS" ] || { printf 'threshold file missing: %s\n' "$THRESHOLDS" >&2; return 1; }
  ml_error="$(threshold_value megalinter_error_max)"
  ml_warning="$(threshold_value megalinter_warning_max)"
  ccn="$(threshold_value lizard_ccn)"
  over="$(threshold_value lizard_over_max)"
  dup="$(threshold_value jscpd_percent_max)"
  perf="$(threshold_value perf_p95_ms_max)"
  delta="$(threshold_value dependency_delta_max)"
  case "$ml_error:$ml_warning:$ccn:$over:$dup:$perf:$delta" in
    *[!0-9.:]*|*::*)
      printf 'threshold file is unparseable\n' >&2
      return 1
      ;;
  esac
  # The starting marks are immutable maximums. A branch cannot weaken a gate by raising them.
  awk -v ml_error="$ml_error" -v ml_warning="$ml_warning" -v ccn="$ccn" -v over="$over" \
    -v dup="$dup" -v perf="$perf" -v delta="$delta" \
    'BEGIN { exit !(ml_error <= 0 && ml_warning <= 0 && ccn <= 15 && over <= 3 && dup <= 0.00 && perf <= 1000 && delta <= 0) }'
  ec=$?
  [ "$ec" -eq 0 ] || { printf 'threshold change weakens the shipped gate; maxima may not be raised\n' >&2; return 1; }
  return 0
}

repo_abs() {
  [ -d "$1" ] || { printf 'repo is not a directory: %s\n' "$1" >&2; return 2; }
  (cd "$1" && pwd -P)
}

indexed_marker() {
  local repo path
  repo="$1"
  for path in \
    "$repo/.code-graph/graph.bin" \
    "$repo/.code-graph/index.json" \
    "$repo/.codegraph/index.json" \
    "$repo/.codebase-memory/graph.db" \
    "$repo/.codebase-memory/graph.db.zst"; do
    [ -s "$path" ] && { printf '%s' "$path"; return 0; }
  done
  return 1
}

state_graph() {
  local repo graph_bin ec marker graph_out inventory
  repo="$1"
  graph_bin="$(require_any_tool code-graph code-graph-cli 'cargo install code-graph-cli')"
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  require_tool tokei 'brew install tokei'
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  marker="$(indexed_marker "$repo")"
  ec=$?
  if [ "$ec" -ne 0 ]; then
    printf 'repo was never indexed: no code-graph index marker under %s\n' "$repo" >&2
    return 1
  fi
  graph_out="$(mktemp "$TMP_ROOT/sdlc-graph.XXXXXX")"
  "$graph_bin" export "$repo" --format dot --granularity file >"$graph_out" 2>&1
  ec=$?
  [ "$ec" -eq 0 ] || { cat "$graph_out" >&2; rm -f "$graph_out"; return "$ec"; }
  rm -f "$graph_out"
  inventory="$(mktemp "$TMP_ROOT/sdlc-tokei.XXXXXX")"
  tokei --files --output json "$repo" >"$inventory" 2>&1
  ec=$?
  if [ "$ec" -eq 0 ]; then
    jq -e '([.[]?.reports[]?] | length) > 0' "$inventory" >/dev/null 2>&1
    ec=$?
  fi
  [ "$ec" -eq 0 ] || { cat "$inventory" >&2; rm -f "$inventory"; return 1; }
  rm -f "$inventory"
  printf 'indexed=%s\n' "$marker"
  return 0
}

state_edge_cases() {
  local repo state ec
  repo="$1"
  # Honour an INHERITED intake state first. This hardcoded two repo-local paths and ignored
  # FLEET_INTAKE_STATE, so a run whose planning artifacts lived outside the target repo failed with
  # "intake SOW challenge register is absent" while a valid, gate-passing SOW sat right there. The
  # artifacts existed; only the wiring did not.
  state="${FLEET_INTAKE_STATE:-}"
  [ -n "$state" ] && [ -d "$state" ] || state="$repo/state/intake"
  [ -d "$state" ] || state="$repo/.fleet/intake"
  [ -d "$state" ] || { printf 'not ready: intake SOW challenge register is absent (looked in FLEET_INTAKE_STATE, %s/state/intake, %s/.fleet/intake)\n' "$repo" "$repo" >&2; return 1; }
  FLEET_INTAKE_STATE="$state" "$FLEET_ROOT/registry/features/intake/intake.sh" gate
  ec=$?
  [ "$ec" -eq 0 ] && printf 'intake challenge register and leaf decisions are valid\n'
  return "$ec"
}

state_code() {
  local cli ec
  cli="$(require_any_tool claude codex 'install Claude Code or Codex CLI')"
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  "$cli" --version >/dev/null 2>&1
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  # This is a runner check. The calling agent supplies the actual coding work.
  printf 'runner=%s; implementation work remains with the calling agent\n' "$cli"
  return 0
}

state_test() {
  local repo ec
  repo="$1"
  if [ -f "$repo/tests/small/sdlc-pipeline.bats" ]; then
    require_tool bats 'brew install bats-core'
    ec=$?
    [ "$ec" -eq 0 ] || return "$ec"
    bats --timing "$repo/tests/small/sdlc-pipeline.bats"
    return $?
  fi
  if [ -d "$repo/tests" ] && find "$repo/tests" -type f \( -name '*.bats' -o -name '*.test.sh' \) -print -quit | grep -q .; then
    require_tool bats 'brew install bats-core'
    ec=$?
    [ "$ec" -eq 0 ] || return "$ec"
    bats --timing "$repo/tests"
    return $?
  fi
  if find "$repo" -maxdepth 3 -type f \( -name 'test_*.py' -o -name '*_test.py' \) -print -quit | grep -q .; then
    require_tool pytest 'python -m pip install pytest'
    ec=$?
    [ "$ec" -eq 0 ] || return "$ec"
    pytest -q --maxfail=1 "$repo"
    return $?
  fi
  if find "$repo" -maxdepth 2 -type f -name 'playwright.config.*' -print -quit | grep -q .; then
    require_tool playwright 'npm install -g playwright'
    ec=$?
    [ "$ec" -eq 0 ] || return "$ec"
    playwright test "$repo"
    return $?
  fi
  STATE_RESULT=not-applicable
  printf 'not-applicable: no bats, pytest, or playwright suite detected\n'
  return 0
}

shell_files() {
  local repo
  repo="$1"
  find "$repo" -path "$repo/.git" -prune -o -path "$repo/var" -prune -o -path "$repo/node_modules" -prune -o -type f -name '*.sh' -print
}

state_quality_legacy() {
  local repo ec ccn over dup shell_count shell_bad file dup_root shell_root
  repo="$1"
  thresholds_ok
  ec=$?
  [ "$ec" -eq 0 ] || return 1
  ccn="$(threshold_value lizard_ccn)"
  over="$(threshold_value lizard_over_max)"
  dup="$(threshold_value jscpd_percent_max)"
  require_tool lizard 'python -m pip install lizard'
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  # The fleet starting mark is the measured Python surface: three functions in model_router.py.
  # Shell and duplication coverage remain language-independent below.
  lizard -l python --CCN "$ccn" -i "$over" "$repo"
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  require_tool jscpd 'npm install -g jscpd'
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  if [ -d "$repo/registry" ]; then
    dup_root="$repo/registry"
    shell_root="$repo/registry"
  else
    dup_root="$repo"
    shell_root="$repo"
  fi
  jscpd --min-lines 12 --threshold "$dup" --exit-code 1 --silent --no-colors \
    --ignore '**/.git/**,**/node_modules/**,**/var/**,**/vendor/**,**/tests/**' "$dup_root"
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  require_tool shellcheck 'brew install shellcheck'
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  shell_count=0
  shell_bad=0
  while IFS= read -r file; do
    [ -n "$file" ] || continue
    shell_count=$((shell_count + 1))
    shellcheck -S warning --enable=all --exclude=SC2250,SC2312,SC2248,SC2292,SC2310,SC2311 "$file"
    ec=$?
    [ "$ec" -eq 0 ] || shell_bad=1
  done <<EOF_SHELL_FILES
$(shell_files "$shell_root")
EOF_SHELL_FILES
  [ "$shell_count" -gt 0 ] || { STATE_RESULT=not-applicable; printf 'not-applicable: no shell files\n'; return 0; }
  [ "$shell_bad" -eq 0 ] || return 1
  return 0
}

megalinter_counts() {
  local report counts ec
  report="$1"
  jq -e 'type == "object" and ([.. | objects | select(has("severity") or has("level") or has("numberErrorsFound") or has("linterKey"))] | length > 0)' "$report" >/dev/null 2>&1
  ec=$?
  [ "$ec" -eq 0 ] || return 4
  counts="$(jq -r '
    def report_objects:
      ([.. | objects] + [.. | strings | try fromjson catch empty | objects]);
    def severity: ((.severity // .level // "") | ascii_downcase);
    reduce (report_objects[] | select((has("severity") or has("level")))) as $item
      ({error: 0, warning: 0, info: 0};
       ($item | if has("numberErrorsFound") then (.numberErrorsFound // 0) else 1 end) as $n |
       ($item | severity) as $s |
       if $s == "error" then .error += $n
       elif $s == "warning" then .warning += $n
       elif $s == "info" then .info += $n
       else . end)
    | [.error, .warning, .info] | @tsv
  ' "$report" 2>/dev/null)"
  ec=$?
  [ "$ec" -eq 0 ] || return 4
  [ -n "$counts" ] || return 4
  printf '%s\n' "$counts"
}

state_quality_megalinter() {
  local repo ec report_dir report runner_output counts ml_error ml_warning ml_info lizard_ec
  repo="$1"
  thresholds_ok
  ec=$?
  [ "$ec" -eq 0 ] || return 1
  # ABSENT npx/docker must FALL BACK, not report `missing`. Returning 3 here meant any machine
  # without Docker lost the quality gate entirely -- the strongest state silently becoming no state.
  # The daemon-unreachable case below already falls back correctly; the tool-absent case did not,
  # which is an inconsistency, not a policy.
  if ! command -v npx >/dev/null 2>&1 || ! command -v docker >/dev/null 2>&1; then
    printf 'MegaLinter unavailable: npx or docker not installed; using documented legacy fallback\n' >&2
    state_quality_legacy "$repo"
    return $?
  fi

  # Docker is a runtime prerequisite of mega-linter-runner. In a restricted shell where the
  # daemon is unreachable, retain the old bounded adapter rather than converting unavailable
  # analysis into a false pass.
  docker info >/dev/null 2>&1
  ec=$?
  if [ "$ec" -ne 0 ]; then
    printf 'MegaLinter unavailable: Docker daemon is not reachable; using documented legacy fallback\n' >&2
    state_quality_legacy "$repo"
    return $?
  fi

  report_dir="$(mktemp -d "$TMP_ROOT/sdlc-megalinter.XXXXXX")"
  runner_output="$(mktemp "$TMP_ROOT/sdlc-megalinter-log.XXXXXX")"
  REPORT_OUTPUT_FOLDER="$report_dir" npx --yes mega-linter-runner \
    --path "$repo" \
    --flavor "${FLEET_MEGA_LINTER_FLAVOR:-all}" \
    --release "${FLEET_MEGA_LINTER_RELEASE:-v9}" \
    --timeout "${FLEET_MEGA_LINTER_TIMEOUT:-300}" \
    --env "VALIDATE_ALL_CODEBASE=true" \
    --env "JSON_REPORTER=true" \
    --env "JSON_REPORTER_OUTPUT_DETAIL=simple" \
    --env "REPORT_OUTPUT_FOLDER=$report_dir" >"$runner_output" 2>&1
  ec=$?
  cat "$runner_output"
  report="$report_dir/mega-linter-report.json"
  if [ ! -s "$report" ]; then
    printf 'MegaLinter report missing: %s\n' "$report" >&2
    rm -rf "$report_dir" "$runner_output"
    [ "$ec" -eq 0 ] && return 4
    return "$ec"
  fi
  counts="$(megalinter_counts "$report")"
  ec=$?
  if [ "$ec" -ne 0 ]; then
    printf 'MegaLinter report is not parseable: %s\n' "$report" >&2
    rm -rf "$report_dir" "$runner_output"
    return 4
  fi
  IFS='	' read -r ml_error ml_warning ml_info <<EOF_COUNTS
$counts
EOF_COUNTS
  printf 'megalinter_findings error=%s warning=%s info=%s report=temporary-json\n' "$ml_error" "$ml_warning" "$ml_info"

  lizard -l python --CCN "$(threshold_value lizard_ccn)" -i "$(threshold_value lizard_over_max)" "$repo"
  lizard_ec=$?
  rm -rf "$report_dir" "$runner_output"
  awk -v errors="$ml_error" -v warnings="$ml_warning" \
    -v error_max="$(threshold_value megalinter_error_max)" \
    -v warning_max="$(threshold_value megalinter_warning_max)" \
    'BEGIN { exit !(errors <= error_max && warnings <= warning_max) }'
  ec=$?
  [ "$ec" -eq 0 ] && [ "$lizard_ec" -eq 0 ]
}

state_quality() {
  local repo ec
  repo="$1"
  require_tool lizard 'python -m pip install lizard'
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  state_quality_megalinter "$repo"
}

tracked_snapshot() {
  local repo out ec rel src dst
  repo="$1"
  out="$2"
  git -C "$repo" rev-parse --is-inside-work-tree >/dev/null 2>&1
  ec=$?
  [ "$ec" -eq 0 ] || return 2
  while IFS= read -r rel; do
    [ -n "$rel" ] || continue
    src="$repo/$rel"
    dst="$out/$rel"
    [ -f "$src" ] || continue
    mkdir -p "$(dirname "$dst")"
    cp -p "$src" "$dst" || return 1
  done <<EOF_TRACKED
$(git -C "$repo" ls-files)
EOF_TRACKED
  find "$out" -type f -print -quit | grep -q .
  return $?
}

state_security() {
  local repo snapshot ec
  repo="$1"
  snapshot="$(mktemp -d "$TMP_ROOT/sdlc-security.XXXXXX")"
  tracked_snapshot "$repo" "$snapshot"
  ec=$?
  if [ "$ec" -eq 2 ]; then
    rm -rf "$snapshot"
    STATE_RESULT=not-applicable
    printf 'not-applicable: repo has no git-tracked file set\n'
    return 0
  fi
  [ "$ec" -eq 0 ] || { rm -rf "$snapshot"; return 1; }
  require_tool semgrep 'python -m pip install semgrep'
  ec=$?
  [ "$ec" -eq 0 ] || { rm -rf "$snapshot"; return "$ec"; }
  semgrep scan --config "$SELF_DIR/semgrep.yml" --error --quiet "$snapshot"
  ec=$?
  [ "$ec" -eq 0 ] || { rm -rf "$snapshot"; return "$ec"; }
  require_tool gitleaks 'brew install gitleaks'
  ec=$?
  [ "$ec" -eq 0 ] || { rm -rf "$snapshot"; return "$ec"; }
  gitleaks detect --source "$snapshot" --no-git --no-banner --redact --exit-code 1
  ec=$?
  [ "$ec" -eq 0 ] || { rm -rf "$snapshot"; return "$ec"; }
  require_tool trivy 'brew install trivy'
  ec=$?
  [ "$ec" -eq 0 ] || { rm -rf "$snapshot"; return "$ec"; }
  trivy fs --quiet --scanners secret,misconfig --skip-db-update --exit-code 1 "$snapshot"
  ec=$?
  rm -rf "$snapshot"
  return "$ec"
}

state_perf() {
  local perf_command ec json p95 platform profiler profile_dir profile_ec profile_pid sample_ec wait_ec
  perf_command="${FLEET_PERF_COMMAND:-}"
  [ -n "$perf_command" ] || {
    STATE_RESULT=not-applicable
    printf 'not-applicable: no benchmark command declared (set FLEET_PERF_COMMAND)\n'
    return 0
  }
  platform="$(uname -s)"
  case "$platform" in
    Linux)
      if command -v poop >/dev/null 2>&1; then
        profiler=poop
      elif command -v perf >/dev/null 2>&1; then
        profiler=perf
      else
        STATE_RESULT=not-applicable
        printf 'not-applicable: Linux instruction/cache profiler missing; install poop (cargo install poop) or perf (sudo apt install linux-tools-common)\n'
        return 0
      fi
      printf 'platform=Linux profiler=%s metrics=instructions,cache-misses\n' "$profiler"
      if [ "$profiler" = poop ]; then
        poop "$perf_command"
      else
        perf stat -x, -e instructions,cache-misses -- sh -c "$perf_command"
      fi
      return $?
      ;;
    Darwin)
      require_tool hyperfine 'brew install hyperfine'
      ec=$?
      [ "$ec" -eq 0 ] || return "$ec"
      if command -v xctrace >/dev/null 2>&1; then
        profiler=xctrace
      elif command -v sample >/dev/null 2>&1; then
        profiler=sample
      else
        printf 'missing profile tool: xctrace or sample\ninstall: xcode-select --install (xctrace) or brew install sample\n' >&2
        return 3
      fi
      printf 'platform=Darwin wall_clock=hyperfine profiler=%s\n' "$profiler"
      ;;
    *)
      STATE_RESULT=not-applicable
      printf 'not-applicable: unsupported platform %s; Linux or Darwin required\n' "$platform"
      return 0
      ;;
  esac
  json="$(mktemp "$TMP_ROOT/sdlc-perf.XXXXXX")"
  hyperfine --warmup 1 --runs 5 --export-json "$json" "$perf_command"
  ec=$?
  if [ "$ec" -ne 0 ]; then
    rm -f "$json"
    return "$ec"
  fi
  p95="$("$FLEET_ROOT/var/venv/bin/python" - "$json" <<'PY_P95'
import json, math, sys
with open(sys.argv[1]) as fh:
    times = sorted(float(x) for x in json.load(fh)["results"][0]["times"])
if not times:
    raise SystemExit(2)
print(times[max(0, math.ceil(len(times) * 0.95) - 1)] * 1000)
PY_P95
  )"
  ec=$?
  rm -f "$json"
  [ "$ec" -eq 0 ] || return 1
  profile_dir="$(mktemp -d "$TMP_ROOT/sdlc-profile.XXXXXX")"
  if [ "$profiler" = xctrace ]; then
    xctrace record --template 'Time Profiler' --output "$profile_dir/trace" --launch -- sh -c "$perf_command" >/dev/null 2>&1
    profile_ec=$?
  else
    sh -c "$perf_command" >/dev/null 2>&1 &
    profile_pid=$!
    sample "$profile_pid" 1 1 >"$profile_dir/sample.txt" 2>&1
    sample_ec=$?
    kill "$profile_pid" >/dev/null 2>&1
    wait "$profile_pid" >/dev/null 2>&1
    wait_ec=$?
    [ "$sample_ec" -eq 0 ] && [ "$wait_ec" -eq 0 ]
    profile_ec=$?
  fi
  printf 'p95_ms=%s threshold_ms=%s profile=%s profile_dir=%s\n' "$p95" "$(threshold_value perf_p95_ms_max)" "$profiler" "$profile_dir"
  # The PROFILE is supplementary evidence; the GATE is the timing budget. Failing the state on a
  # profiler error ran before the p95 comparison, so on Darwin `xctrace` failing unconditionally gave
  # this state exactly two reachable outcomes: not-applicable, or fail-for-the-wrong-reason. A gate
  # that cannot distinguish fast from slow is not measuring performance.
  # A profiler failure is now reported and the timing verdict still decides.
  [ "$profile_ec" -eq 0 ] || printf 'profile unavailable: %s (timing verdict still applies)\n' "$profiler" >&2
  awk -v p95="$p95" -v max="$(threshold_value perf_p95_ms_max)" 'BEGIN { exit !(p95 <= max) }'
}

state_maintenance() {
  local repo cargo_file ec cargo_root lock_file manifest_changed lock_changed
  repo="$1"
  cargo_file="$(find "$repo" -path "$repo/.git" -prune -o -path "$repo/var" -prune -o -name Cargo.toml -print -quit)"
  if [ -z "$cargo_file" ]; then
    STATE_RESULT=not-applicable
    printf 'not-applicable: no Rust Cargo.toml\n'
    return 0
  fi
  require_tool cargo-deny 'cargo install cargo-deny'
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  cargo_root="$(dirname "$cargo_file")"
  # Offline+locked makes this gate bounded and prevents a quality run from mutating or
  # re-resolving the dependency lockfile. A stale lock is a real maintenance failure.
  (cd "$cargo_root" && cargo deny --offline --locked check)
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  lock_file="$cargo_root/Cargo.lock"
  if [ -d "$repo/.git" ] && [ -f "$lock_file" ]; then
    manifest_changed="$(git -C "$repo" diff --name-only -- '*.toml' | grep -E '(^|/)Cargo.toml$' | wc -l | tr -d ' ')"
    lock_changed="$(git -C "$repo" diff --name-only -- '*.lock' | grep -E '(^|/)Cargo.lock$' | wc -l | tr -d ' ')"
    [ "$manifest_changed" -gt 0 ] && [ "$lock_changed" -eq 0 ] && {
      printf 'dependency delta: Cargo.toml changed without Cargo.lock\n' >&2
      return 1
    }
  fi
  printf 'cargo-deny and dependency delta checks passed\n'
  return 0
}

record_state() {
  local state result ec repo ledger event receipt_ec
  state="$1"
  result="$2"
  ec="$3"
  repo="$4"
  ledger="$5"
  event="$(printf '%s' "$result" | tr '[:lower:]' '[:upper:]')"
  FLEET_LEDGER="$ledger" receipt_append "$(now_iso)" "$event" "$PIPELINE_NAME/$state" "$repo" \
    not-applicable not-applicable "$ec" 0 0 0 >/dev/null
  receipt_ec=$?
  [ "$receipt_ec" -eq 0 ] || {
    printf 'receipt write failed for state %s (exit %s)\n' "$state" "$receipt_ec" >&2
    return 1
  }
  return 0
}

run_state() {
  local state runner repo ledger out ec result receipt_ec
  state="$1"
  runner="$2"
  repo="$3"
  ledger="$4"
  out="$(mktemp "$TMP_ROOT/sdlc-state.XXXXXX")"
  STATE_RESULT=pass
  "$runner" "$repo" >"$out" 2>&1
  ec=$?
  cat "$out"
  rm -f "$out"
  if [ "$STATE_RESULT" = not-applicable ]; then
    result=not-applicable
    ec=0
  elif [ "$ec" -eq 0 ]; then
    result=pass
  elif [ "$ec" -eq 3 ]; then
    result=missing
  else
    result=fail
  fi
  printf 'state=%s result=%s exit=%s\n' "$state" "$result" "$ec"
  record_state "$state" "$result" "$ec" "$repo" "$ledger"
  receipt_ec=$?
  [ "$receipt_ec" -eq 0 ] || return 1
  [ "$result" = missing ] && return 3
  [ "$result" = pass ] || [ "$result" = not-applicable ]
}

run_pipeline() {
  local repo ec ledger overall missing state state_ec
  repo="$(repo_abs "$1")"
  ec=$?
  [ "$ec" -eq 0 ] || return "$ec"
  if [ -n "${FLEET_LEDGER:-}" ]; then
    ledger="$FLEET_LEDGER"
  else
    ledger="$(mktemp "$TMP_ROOT/fleet-sdlc-receipts.XXXXXX")"
  fi
  case "$ledger" in
    "$FLEET_ROOT/ledger/RECEIPTS.jsonl")
      printf 'refusing production ledger: %s\n' "$ledger" >&2
      return 2
      ;;
  esac
  [ -f "$THRESHOLDS" ] || { printf 'threshold file missing: %s\n' "$THRESHOLDS" >&2; return 1; }
  printf 'repo=%s ledger=%s\n' "$repo" "$ledger"
  overall=0
  missing=0
  for state in graph edge-cases code test quality security perf maintenance; do
    case "$state" in
      graph) run_state "$state" state_graph "$repo" "$ledger";;
      edge-cases) run_state "$state" state_edge_cases "$repo" "$ledger";;
      code) run_state "$state" state_code "$repo" "$ledger";;
      test) run_state "$state" state_test "$repo" "$ledger";;
      quality) run_state "$state" state_quality "$repo" "$ledger";;
      security) run_state "$state" state_security "$repo" "$ledger";;
      perf) run_state "$state" state_perf "$repo" "$ledger";;
      maintenance) run_state "$state" state_maintenance "$repo" "$ledger";;
    esac
    state_ec=$?
    if [ "$state_ec" -ne 0 ]; then
      overall=1
      [ "$state_ec" -eq 3 ] && missing=1
    fi
  done
  [ "$overall" -eq 0 ] && return 0
  [ "$missing" -eq 1 ] && return 3
  return 1
}

case "${1-}" in
  --help|-h) usage; exit 0 ;;
  run)
    [ "${2-}" = --repo ] && [ -n "${3-}" ] || { usage >&2; exit 2; }
    run_pipeline "$3"
    exit "$?"
    ;;
  *) usage >&2; exit 2 ;;
esac
