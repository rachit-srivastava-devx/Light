#!/usr/bin/env bash
set -uo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

LOG_DIR=${GATE_LOG_DIR:-"$ROOT/.gate-logs"}
TOOL_DIR="$ROOT/.gate-tools"
RUNTIME_DIR=${ORB_GATE_RUNTIME_DIR:-/tmp/orb-production-gate-tools}
REDACTOR="$ROOT/scripts/redact-gate-log.mjs"
COMMAND_LOG="$LOG_DIR/commands.log"
OVERALL=0
PIDS=()
TEMP_DIRS=()

mkdir -p "$LOG_DIR" "$TOOL_DIR" "$RUNTIME_DIR"
: >"$COMMAND_LOG"

cleanup() {
  for pid in "${PIDS[@]-}"; do
    [[ -n "$pid" ]] || continue
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" 2>/dev/null || true
  done
  for dir in "${TEMP_DIRS[@]-}"; do
    [[ -n "$dir" ]] || continue
    if [[ "$dir" == /tmp/orb-gate-* || "$dir" == /private/tmp/orb-gate-* ]]; then
      rm -rf -- "$dir"
    fi
  done
}
trap cleanup EXIT INT TERM

log_command() {
  printf '%q ' "$@" >>"$COMMAND_LOG"
  printf '\n' >>"$COMMAND_LOG"
}

run_logged() {
  local log=$1
  shift
  log_command "$@"
  "$@" 2>&1 | node "$REDACTOR" >"$log"
  return "${PIPESTATUS[0]}"
}

append_redacted() {
  local source=$1
  local destination=$2
  [[ -f "$source" ]] || return 0
  node "$REDACTOR" <"$source" >>"$destination"
}

record() {
  local gate=$1
  local tool=$2
  local version=$3
  local rc=$4
  local denominator_count=$5
  local denominator_label=$6
  local findings=$7
  local log=$8
  local status=PASS

  if ! [[ "$denominator_count" =~ ^[0-9]+$ ]] || (( denominator_count == 0 )); then
    rc=97
  fi
  if (( rc != 0 )); then
    status=FAIL
    OVERALL=1
  fi
  printf '%s %-24s tool=%s@%s denominator=%s_%s findings=%s log=%s\n' \
    "$status" "$gate" "$tool" "$version" "$denominator_count" "$denominator_label" \
    "$findings" "$log"
}

json_array_count() {
  node -e 'const fs=require("fs");try{const x=JSON.parse(fs.readFileSync(process.argv[1],"utf8"));console.log(Array.isArray(x)?x.length:0)}catch{console.log(0)}' "$1"
}

snapshot_files() {
  local mode=$1
  local destination=$2
  local manifest=$3
  if [[ "$mode" == tracked ]]; then
    git ls-files -z >"$manifest"
  else
    git ls-files --cached --others --exclude-standard -z >"$manifest"
  fi
  while IFS= read -r -d '' path; do
    [[ -e "$path" || -L "$path" ]] || continue
    mkdir -p "$destination/$(dirname "$path")"
    cp -pP -- "$path" "$destination/$path"
  done <"$manifest"
}

manifest_count() {
  python3 -c 'import sys; print(sys.stdin.buffer.read().count(b"\0"))' <"$1"
}

gitleaks_scan() {
  local gate=$1
  local target=$2
  local denominator=$3
  local log=$4
  local report="$log.report"
  local rc

  log_command gitleaks dir --redact=100 --no-banner --report-format json --report-path "$report" "$target"
  gitleaks dir --redact=100 --no-banner --report-format json --report-path "$report" "$target" \
    2>&1 | node "$REDACTOR" >"$log"
  rc=${PIPESTATUS[0]}
  printf '\n[redacted JSON report]\n' >>"$log"
  append_redacted "$report" "$log"
  local findings
  findings=$(json_array_count "$report")
  record "$gate" gitleaks "$GITLEAKS_VERSION" "$rc" "$denominator" files "$findings" "$log"
}

printf 'Orb production gate stack (all detailed output is redacted and stored under %s)\n' "$LOG_DIR"

# Tool versions are evidence and are intentionally kept out of package manifests owned by Track A/C.
VERSIONS_LOG="$LOG_DIR/versions.log"
: >"$VERSIONS_LOG"
GITLEAKS_VERSION=$(gitleaks version 2>>"$VERSIONS_LOG" | head -n 1)
TRIVY_VERSION=$(trivy --version 2>>"$VERSIONS_LOG" | awk 'NR==1 {print $2}')
RUFF_VERSION=$(backend/relay-py/.venv/bin/python -m ruff --version 2>>"$VERSIONS_LOG" | awk '{print $2}')
MYPY_VERSION=$(backend/relay-py/.venv/bin/python -m mypy --version 2>>"$VERSIONS_LOG" | awk '{print $2}')
CARGO_VERSION=$(cargo --version 2>>"$VERSIONS_LOG" | awk '{print $2}')
CLIPPY_VERSION=$(cargo clippy --version 2>>"$VERSIONS_LOG" | awk '{print $2}')
CARGO_AUDIT_VERSION=$(cargo audit --version 2>>"$VERSIONS_LOG" | awk '{print $2}')
CARGO_DENY_VERSION=$(cargo deny --version 2>>"$VERSIONS_LOG" | awk '{print $2}')
NODE_VERSION=$(node --version 2>>"$VERSIONS_LOG" | tr -d v)

SEMGREP_ENV=(
  env
  "XDG_CACHE_HOME=$TOOL_DIR/xdg-cache"
  "XDG_CONFIG_HOME=$TOOL_DIR/xdg-config"
  "SEMGREP_LOG_FILE=$LOG_DIR/semgrep-internal.log"
)
mkdir -p "$TOOL_DIR/xdg-cache" "$TOOL_DIR/xdg-config"
SEMGREP_VERSION=$("${SEMGREP_ENV[@]}" semgrep --version 2>>"$VERSIONS_LOG" | tail -n 1)

PY_TOOL_ENV="$TOOL_DIR/python-tools"
PY_TOOL_LOG="$LOG_DIR/python-tool-bootstrap.log"
if [[ ! -x "$PY_TOOL_ENV/bin/bandit" || ! -x "$PY_TOOL_ENV/bin/pre-commit" ]]; then
  log_command python3 -m venv "$PY_TOOL_ENV"
  python3 -m venv "$PY_TOOL_ENV" >"$PY_TOOL_LOG" 2>&1
  log_command "$PY_TOOL_ENV/bin/python" -m pip install --disable-pip-version-check bandit==1.9.4 pre-commit==4.6.2
  "$PY_TOOL_ENV/bin/python" -m pip install --disable-pip-version-check \
    bandit==1.9.4 pre-commit==4.6.2 2>&1 | node "$REDACTOR" >>"$PY_TOOL_LOG"
fi
BANDIT_VERSION=$("$PY_TOOL_ENV/bin/bandit" --version 2>>"$VERSIONS_LOG" | awk 'NR==1 {print $2}')
PRECOMMIT_VERSION=$("$PY_TOOL_ENV/bin/pre-commit" --version 2>>"$VERSIONS_LOG" | awk '{print $2}')

ESLINT_VERSION=$(scripts/eslint.sh --version 2>>"$VERSIONS_LOG" | tr -d v)

printf 'gitleaks=%s semgrep=%s trivy=%s bandit=%s eslint=%s ruff=%s mypy=%s cargo=%s clippy=%s cargo-audit=%s cargo-deny=%s pre-commit=%s node=%s\n' \
  "$GITLEAKS_VERSION" "$SEMGREP_VERSION" "$TRIVY_VERSION" "$BANDIT_VERSION" \
  "$ESLINT_VERSION" "$RUFF_VERSION" "$MYPY_VERSION" "$CARGO_VERSION" "$CLIPPY_VERSION" \
  "$CARGO_AUDIT_VERSION" "$CARGO_DENY_VERSION" "$PRECOMMIT_VERSION" "$NODE_VERSION" \
  >>"$VERSIONS_LOG"

printf '\n[Gate 1: secrets]\n'
SNAPSHOT_ROOT=$(mktemp -d "/tmp/orb-gate-secrets.XXXXXX")
TEMP_DIRS+=("$SNAPSHOT_ROOT")
mkdir -p "$SNAPSHOT_ROOT/tree" "$SNAPSHOT_ROOT/tracked"
snapshot_files working "$SNAPSHOT_ROOT/tree" "$SNAPSHOT_ROOT/tree.manifest"
snapshot_files tracked "$SNAPSHOT_ROOT/tracked" "$SNAPSHOT_ROOT/tracked.manifest"
TREE_FILES=$(manifest_count "$SNAPSHOT_ROOT/tree.manifest")
TRACKED_FILES=$(manifest_count "$SNAPSHOT_ROOT/tracked.manifest")

gitleaks_scan secrets.working-tree "$SNAPSHOT_ROOT/tree" "$TREE_FILES" "$LOG_DIR/gitleaks-working-tree.log"
gitleaks_scan secrets.keyshape-tracked "$SNAPSHOT_ROOT/tracked" "$TRACKED_FILES" "$LOG_DIR/gitleaks-tracked-keyshape.log"

HISTORY_LOG="$LOG_DIR/gitleaks-history.log"
HISTORY_REPORT="$HISTORY_LOG.report"
COMMITS=$(git rev-list --all --count)
log_command gitleaks git --redact=100 --no-banner --report-format json --report-path "$HISTORY_REPORT" --log-opts=--all .
gitleaks git --redact=100 --no-banner --report-format json --report-path "$HISTORY_REPORT" \
  --log-opts='--all' . 2>&1 | node "$REDACTOR" >"$HISTORY_LOG"
HISTORY_RC=${PIPESTATUS[0]}
printf '\n[redacted JSON report]\n' >>"$HISTORY_LOG"
append_redacted "$HISTORY_REPORT" "$HISTORY_LOG"
HISTORY_FINDINGS=$(json_array_count "$HISTORY_REPORT")
record secrets.full-history gitleaks "$GITLEAKS_VERSION" "$HISTORY_RC" "$COMMITS" commits "$HISTORY_FINDINGS" "$HISTORY_LOG"

ENV_LOG="$LOG_DIR/env-ignore.log"
: >"$ENV_LOG"
log_command git check-ignore -v .env
git check-ignore -v .env >>"$ENV_LOG" 2>&1
IGNORE_RC=$?
log_command git ls-files --error-unmatch .env
git ls-files --error-unmatch .env >>"$ENV_LOG" 2>&1
TRACK_RC=$?
ENV_RC=0
ENV_FINDINGS=0
(( IGNORE_RC == 0 )) || { ENV_RC=1; ENV_FINDINGS=$((ENV_FINDINGS + 1)); }
(( TRACK_RC != 0 )) || { ENV_RC=1; ENV_FINDINGS=$((ENV_FINDINGS + 1)); }
record secrets.env-contract git "$({ git --version | awk '{print $3}'; } 2>/dev/null)" "$ENV_RC" 2 checks "$ENV_FINDINGS" "$ENV_LOG"

printf '\n[Gate 2: SAST and dependency security]\n'
SEMGREP_LOG="$LOG_DIR/semgrep.log"
: >"$SEMGREP_LOG"
SEMGREP_JSON="$SNAPSHOT_ROOT/semgrep.json"
SEMGREP_STDERR="$SNAPSHOT_ROOT/semgrep.stderr"
SEMGREP_COMMAND=(
  semgrep scan
  --config p/typescript
  --config p/python
  --config p/secrets
  --config p/owasp-top-ten
  --config p/react
  --strict
  --error
  --metrics=off
  --json
  --time
  --exclude '**/node_modules/**'
  --exclude '**/.venv/**'
  --exclude '**/target/**'
  --exclude '**/build/**'
  --exclude '**/dist/**'
  apps backend domain tooling scripts
)
log_command "${SEMGREP_ENV[@]}" "${SEMGREP_COMMAND[@]}"
"${SEMGREP_ENV[@]}" "${SEMGREP_COMMAND[@]}" >"$SEMGREP_JSON" 2>"$SEMGREP_STDERR"
SEMGREP_RC=$?
append_redacted "$SEMGREP_STDERR" "$SEMGREP_LOG"
printf '\n[redacted JSON report]\n' >>"$SEMGREP_LOG"
append_redacted "$SEMGREP_JSON" "$SEMGREP_LOG"
read -r SEMGREP_RULES SEMGREP_FILES SEMGREP_FINDINGS < <(node -e '
  const fs=require("fs");
  try {
    const j=JSON.parse(fs.readFileSync(process.argv[1],"utf8"));
    const rules=new Set((j.time?.rules??[]).map((r)=>typeof r==="string"?r:(r.rule_id??r.id)).filter(Boolean));
    console.log(rules.size, (j.paths?.scanned??[]).length, (j.results??[]).length);
  } catch { console.log("0 0 0"); }
' "$SEMGREP_JSON")
SEMGREP_RULES_RUN=$(sed -n 's/.*Rules run: \([0-9][0-9]*\).*/\1/p' "$SEMGREP_STDERR" | tail -n 1)
SEMGREP_RULES_RUN=${SEMGREP_RULES_RUN:-0}
if (( SEMGREP_RULES == 0 )); then SEMGREP_RC=97; fi
record sast.semgrep semgrep "$SEMGREP_VERSION" "$SEMGREP_RC" "$SEMGREP_RULES" "rules_loaded/${SEMGREP_RULES_RUN}_rules_run/${SEMGREP_FILES}_files" "$SEMGREP_FINDINGS" "$SEMGREP_LOG"

TRIVY_LOG="$LOG_DIR/trivy.log"
: >"$TRIVY_LOG"
TRIVY_JSON="$SNAPSHOT_ROOT/trivy.json"
TRIVY_STDERR="$SNAPSHOT_ROOT/trivy.stderr"
TRIVY_COMMAND=(
  trivy fs
  --cache-dir "$TOOL_DIR/trivy-cache"
  --scanners vuln,secret,misconfig
  --exit-code 1
  --format json
  --output "$TRIVY_JSON"
  --skip-dirs .git
  --skip-dirs .claude/worktrees
  --skip-dirs '**/.claude/worktrees/**'
  --skip-dirs node_modules
  --skip-dirs .venv
  --skip-dirs '**/.venv/**'
  --skip-dirs evals/.venv
  --skip-dirs '**/__pycache__/**'
  --skip-dirs target
  --skip-dirs build
  --skip-dirs dist
  --skip-dirs Pods
  --skip-dirs .gate-tools
  --skip-dirs .gate-logs
  --skip-files .env
  --skip-files .env.local
  --timeout 10m
  .
)
log_command "${TRIVY_COMMAND[@]}"
"${TRIVY_COMMAND[@]}" >"$TRIVY_STDERR" 2>&1
TRIVY_RC=$?
append_redacted "$TRIVY_STDERR" "$TRIVY_LOG"
printf '\n[redacted JSON report]\n' >>"$TRIVY_LOG"
append_redacted "$TRIVY_JSON" "$TRIVY_LOG"
read -r TRIVY_TARGETS TRIVY_PACKAGES TRIVY_FINDINGS < <(node -e '
  const fs=require("fs");
  try {
    const j=JSON.parse(fs.readFileSync(process.argv[1],"utf8"));
    const results=j.Results??[];
    let packages=0, findings=0;
    for (const r of results) {
      packages += (r.Packages??[]).length;
      findings += (r.Vulnerabilities??[]).length + (r.Misconfigurations??[]).filter(x=>x.Status!=="PASS").length + (r.Secrets??[]).length;
    }
    console.log(results.length, packages, findings);
  } catch { console.log("0 0 0"); }
' "$TRIVY_JSON")
record sast.trivy trivy "$TRIVY_VERSION" "$TRIVY_RC" "$TRACKED_FILES" "repository_files/${TRIVY_PACKAGES}_packages" "$TRIVY_FINDINGS" "$TRIVY_LOG"

PY_FILES=$(find backend/relay-py/src -type f -name '*.py' | wc -l | tr -d ' ')
BANDIT_LOG="$LOG_DIR/bandit.log"
: >"$BANDIT_LOG"
BANDIT_JSON="$SNAPSHOT_ROOT/bandit.json"
BANDIT_STDERR="$SNAPSHOT_ROOT/bandit.stderr"
BANDIT_COMMAND=("$PY_TOOL_ENV/bin/bandit" -r backend/relay-py/src -f json -o "$BANDIT_JSON")
log_command "${BANDIT_COMMAND[@]}"
"${BANDIT_COMMAND[@]}" >"$BANDIT_STDERR" 2>&1
BANDIT_RC=$?
append_redacted "$BANDIT_STDERR" "$BANDIT_LOG"
printf '\n[redacted JSON report]\n' >>"$BANDIT_LOG"
append_redacted "$BANDIT_JSON" "$BANDIT_LOG"
BANDIT_FINDINGS=$(node -e 'const fs=require("fs");try{console.log((JSON.parse(fs.readFileSync(process.argv[1],"utf8")).results??[]).length)}catch{console.log(0)}' "$BANDIT_JSON")
record sast.bandit bandit "$BANDIT_VERSION" "$BANDIT_RC" "$PY_FILES" py_files "$BANDIT_FINDINGS" "$BANDIT_LOG"

RS_FILES=$(find backend/relay-rs/src -type f -name '*.rs' | wc -l | tr -d ' ')
RUST_PACKAGES=$(rg -c '^\[\[package\]\]' backend/relay-rs/Cargo.lock || true)
RUST_PACKAGES=${RUST_PACKAGES:-0}
RUST_ENV=(env "CARGO_HOME=$RUNTIME_DIR/cargo-home" "CARGO_TARGET_DIR=$TOOL_DIR/relay-rs-target")
mkdir -p "$RUNTIME_DIR/cargo-home" "$TOOL_DIR/relay-rs-target"

CLIPPY_LOG="$LOG_DIR/cargo-clippy.log"
run_logged "$CLIPPY_LOG" "${RUST_ENV[@]}" cargo clippy --manifest-path backend/relay-rs/Cargo.toml --locked --all-targets --all-features -- -D warnings
CLIPPY_RC=$?
CLIPPY_FINDINGS=$(rg -c '^error(?:\[|:)' "$CLIPPY_LOG" || true)
CLIPPY_FINDINGS=${CLIPPY_FINDINGS:-0}
record sast.cargo-clippy cargo-clippy "$CLIPPY_VERSION" "$CLIPPY_RC" "$RS_FILES" rs_files "$CLIPPY_FINDINGS" "$CLIPPY_LOG"

AUDIT_LOG="$LOG_DIR/cargo-audit.log"
: >"$AUDIT_LOG"
AUDIT_JSON="$SNAPSHOT_ROOT/cargo-audit.json"
log_command "${RUST_ENV[@]}" cargo audit --file backend/relay-rs/Cargo.lock --json
"${RUST_ENV[@]}" cargo audit --file backend/relay-rs/Cargo.lock --json >"$AUDIT_JSON" 2>"$SNAPSHOT_ROOT/cargo-audit.stderr"
AUDIT_RC=$?
append_redacted "$SNAPSHOT_ROOT/cargo-audit.stderr" "$AUDIT_LOG"
append_redacted "$AUDIT_JSON" "$AUDIT_LOG"
AUDIT_FINDINGS=$(node -e 'const fs=require("fs");try{const j=JSON.parse(fs.readFileSync(process.argv[1],"utf8"));console.log((j.vulnerabilities?.list??[]).length+(j.warnings?.unmaintained??[]).length+(j.warnings?.unsound??[]).length+(j.warnings?.yanked??[]).length)}catch{console.log(0)}' "$AUDIT_JSON")
record sast.cargo-audit cargo-audit "$CARGO_AUDIT_VERSION" "$AUDIT_RC" "$RUST_PACKAGES" cargo_packages "$AUDIT_FINDINGS" "$AUDIT_LOG"

DENY_LOG="$LOG_DIR/cargo-deny.log"
run_logged "$DENY_LOG" "${RUST_ENV[@]}" cargo deny --manifest-path backend/relay-rs/Cargo.toml --config deny.toml check --show-stats
DENY_RC=$?
DENY_FINDINGS=$(rg -c '(^| )error(\[|:| )' "$DENY_LOG" || true)
DENY_FINDINGS=${DENY_FINDINGS:-0}
record sast.cargo-deny cargo-deny "$CARGO_DENY_VERSION" "$DENY_RC" "$RUST_PACKAGES" cargo_packages "$DENY_FINDINGS" "$DENY_LOG"

printf '\n[Gate 3: linting]\n'
TS_FILES=$(find apps backend \( -path 'apps/*/src/*' -o -path 'backend/*-sidecar/src/*' \) -type f \( -name '*.ts' -o -name '*.tsx' \) | wc -l | tr -d ' ')
ESLINT_LOG="$LOG_DIR/eslint.log"
: >"$ESLINT_LOG"
ESLINT_JSON="$SNAPSHOT_ROOT/eslint.json"
ESLINT_STDERR="$SNAPSHOT_ROOT/eslint.stderr"
log_command scripts/eslint.sh apps backend/gateway-sidecar backend/voice-provider-sidecar --max-warnings 0 --format json --output-file "$ESLINT_JSON"
scripts/eslint.sh apps backend/gateway-sidecar backend/voice-provider-sidecar --max-warnings 0 --format json --output-file "$ESLINT_JSON" >"$ESLINT_STDERR" 2>&1
ESLINT_RC=$?
append_redacted "$ESLINT_STDERR" "$ESLINT_LOG"
printf '\n[redacted JSON report]\n' >>"$ESLINT_LOG"
append_redacted "$ESLINT_JSON" "$ESLINT_LOG"
read -r ESLINT_SCANNED ESLINT_FINDINGS < <(node -e 'const fs=require("fs");try{const j=JSON.parse(fs.readFileSync(process.argv[1],"utf8"));console.log(j.length,j.reduce((n,x)=>n+(x.messages??[]).length,0))}catch{console.log("0 0")}' "$ESLINT_JSON")
record lint.eslint eslint "$ESLINT_VERSION" "$ESLINT_RC" "$ESLINT_SCANNED" ts_files "$ESLINT_FINDINGS" "$ESLINT_LOG"

RUFF_CHECK_LOG="$LOG_DIR/ruff-check.log"
: >"$RUFF_CHECK_LOG"
RUFF_CHECK_JSON="$SNAPSHOT_ROOT/ruff-check.json"
log_command backend/relay-py/.venv/bin/python -m ruff check --config backend/relay-py/pyproject.toml --output-format json backend/relay-py
backend/relay-py/.venv/bin/python -m ruff check --config backend/relay-py/pyproject.toml --output-format json backend/relay-py >"$RUFF_CHECK_JSON" 2>"$SNAPSHOT_ROOT/ruff-check.stderr"
RUFF_CHECK_RC=$?
append_redacted "$SNAPSHOT_ROOT/ruff-check.stderr" "$RUFF_CHECK_LOG"
append_redacted "$RUFF_CHECK_JSON" "$RUFF_CHECK_LOG"
RUFF_CHECK_FINDINGS=$(json_array_count "$RUFF_CHECK_JSON")
record lint.ruff-check ruff "$RUFF_VERSION" "$RUFF_CHECK_RC" "$PY_FILES" py_files "$RUFF_CHECK_FINDINGS" "$RUFF_CHECK_LOG"

RUFF_FORMAT_LOG="$LOG_DIR/ruff-format.log"
run_logged "$RUFF_FORMAT_LOG" backend/relay-py/.venv/bin/python -m ruff format --check --config backend/relay-py/pyproject.toml backend/relay-py
RUFF_FORMAT_RC=$?
RUFF_FORMAT_FINDINGS=$(rg -c 'unformatted: File would be reformatted' "$RUFF_FORMAT_LOG" || true)
RUFF_FORMAT_FINDINGS=${RUFF_FORMAT_FINDINGS:-0}
record lint.ruff-format ruff "$RUFF_VERSION" "$RUFF_FORMAT_RC" "$PY_FILES" py_files "$RUFF_FORMAT_FINDINGS" "$RUFF_FORMAT_LOG"

MYPY_LOG="$LOG_DIR/mypy.log"
run_logged "$MYPY_LOG" bash -c 'cd "$1/backend/relay-py" && .venv/bin/python -m mypy src' bash "$ROOT"
MYPY_RC=$?
MYPY_FINDINGS=$(sed -n 's/^Found \([0-9][0-9]*\) errors.*/\1/p' "$MYPY_LOG" | tail -n 1)
MYPY_FINDINGS=${MYPY_FINDINGS:-0}
record lint.mypy mypy "$MYPY_VERSION" "$MYPY_RC" "$PY_FILES" py_files "$MYPY_FINDINGS" "$MYPY_LOG"

BOUNDARY_LOG="$LOG_DIR/boundary-lint.log"
BOUNDARY_FILES=$(find apps/mobile/src backend/gateway-sidecar/src backend/relay-py/src backend/relay-rs/src -type f \( -name '*.ts' -o -name '*.tsx' -o -name '*.py' -o -name '*.rs' \) | wc -l | tr -d ' ')
run_logged "$BOUNDARY_LOG" node tooling/boundary-lint.mjs
BOUNDARY_RC=$?
BOUNDARY_FINDINGS=$(rg -c '^ - ' "$BOUNDARY_LOG" || true)
BOUNDARY_FINDINGS=${BOUNDARY_FINDINGS:-0}
record lint.boundary boundary-lint "$NODE_VERSION" "$BOUNDARY_RC" "$BOUNDARY_FILES" source_files "$BOUNDARY_FINDINGS" "$BOUNDARY_LOG"

printf '\n[Gate 4: performance]\n'
OHA_ROOT=${ORB_OHA_ROOT:-/tmp/orb-gate-oha-1.16.0}
OHA_BIN="$OHA_ROOT/install/bin/oha"
OHA_INSTALL_LOG="$LOG_DIR/oha-install.log"
if [[ ! -x "$OHA_BIN" ]]; then
  mkdir -p "$OHA_ROOT/cargo-home" "$OHA_ROOT/target" "$OHA_ROOT/install"
  run_logged "$OHA_INSTALL_LOG" env "CARGO_HOME=$OHA_ROOT/cargo-home" "CARGO_TARGET_DIR=$OHA_ROOT/target" cargo install oha --version 1.16.0 --locked --root "$OHA_ROOT/install"
  OHA_INSTALL_RC=$?
else
  printf 'cached binary: %s\n' "$OHA_BIN" >"$OHA_INSTALL_LOG"
  OHA_INSTALL_RC=0
fi
OHA_VERSION=$({ "$OHA_BIN" --version 2>/dev/null || true; } | awk '{print $2}')
OHA_VERSION=${OHA_VERSION:-1.16.0}

PERF_LOG="$LOG_DIR/performance.log"
STARTUP_LOG="$LOG_DIR/startup-health.log"
: >"$PERF_LOG"
: >"$STARTUP_LOG"
PERF_RC=$OHA_INSTALL_RC
STARTUP_RC=1
STARTUP_MS=0
PERF_FINDINGS=0
REQUEST_COUNT=100
REQUEST_BODY_COUNT=10000
if (( OHA_INSTALL_RC == 0 )); then
  if lsof -nP -iTCP:8082 -sTCP:LISTEN >/dev/null 2>&1 || lsof -nP -iTCP:8765 -sTCP:LISTEN >/dev/null 2>&1; then
    printf 'Refused: port 8082 or 8765 already has a listener; adapter provenance cannot be guaranteed.\n' >"$PERF_LOG"
    printf 'Refused: port 8082 or 8765 already has a listener; startup was not attempted.\n' >"$STARTUP_LOG"
    PERF_RC=98
  elif [[ ! -x node_modules/.bin/vite-node ]]; then
    printf 'Blocked: node_modules/.bin/vite-node absent after dependency-stability check.\n' >"$PERF_LOG"
    PERF_RC=99
  else
    GATEWAY_RAW="$SNAPSHOT_ROOT/gateway-sidecar.raw"
    RELAY_RAW="$SNAPSHOT_ROOT/relay.raw"
    START_NS=$(python3 -c 'import time; print(time.monotonic_ns())')
    env -u ANTHROPIC_API_KEY -u GEMINI_API_KEY -u OPENAI_API_KEY \
      ORB_LLM_GATEWAY_ADAPTER=memory GATEWAY_SIDECAR_PORT=8082 \
      node_modules/.bin/vite-node backend/gateway-sidecar/src/index.ts >"$GATEWAY_RAW" 2>&1 &
    PIDS+=("$!")
    env -u ANTHROPIC_API_KEY -u GEMINI_API_KEY -u OPENAI_API_KEY \
      ORB_GATEWAY_URL=http://127.0.0.1:8082 \
      ORB_RATE_PAISE_PER_1K_LLM_TOKENS_IN=0 \
      ORB_RATE_PAISE_PER_1K_LLM_TOKENS_OUT=0 \
      ORB_RATE_PAISE_PER_1K_TTS_CHARS=0 \
      ORB_RATE_PAISE_PER_STT_MINUTE=0 \
      backend/relay-py/.venv/bin/python -m uvicorn orb_relay.app:app \
      --app-dir backend/relay-py/src --host 127.0.0.1 --port 8765 >"$RELAY_RAW" 2>&1 &
    PIDS+=("$!")

    for _ in $(seq 1 100); do
      if curl -fsS http://127.0.0.1:8765/healthz >/dev/null 2>&1 && \
        curl -fsS http://127.0.0.1:8082/healthz >/dev/null 2>&1; then
        STARTUP_RC=0
        break
      fi
      sleep 0.1
    done
    END_NS=$(python3 -c 'import time; print(time.monotonic_ns())')
    STARTUP_MS=$(python3 -c 'import sys; print((int(sys.argv[2]) - int(sys.argv[1])) // 1_000_000)' "$START_NS" "$END_NS")
    printf 'startup_ms=%s relay_health_checks=1 gateway_health_checks=1\n' "$STARTUP_MS" >"$STARTUP_LOG"
    append_redacted "$GATEWAY_RAW" "$STARTUP_LOG"
    append_redacted "$RELAY_RAW" "$STARTUP_LOG"

    if (( STARTUP_RC == 0 )); then
      REQUEST_BODIES="$SNAPSHOT_ROOT/respond-bodies.jsonl"
      log_command node -e 'for (let i=0;i<Number(process.argv[1]);i++) console.log(JSON.stringify({tenant_id:"gate-load",user_id:`gate-load-${i}`,session_id:`gate-load-'"$$"'-${i}`,text:"Give me one short focus tip.",mode:"converse"}))' "$REQUEST_BODY_COUNT"
      node -e 'for (let i=0;i<Number(process.argv[1]);i++) console.log(JSON.stringify({tenant_id:"gate-load",user_id:`gate-load-${i}`,session_id:`gate-load-'"$$"'-${i}`,text:"Give me one short focus tip.",mode:"converse"}))' "$REQUEST_BODY_COUNT" >"$REQUEST_BODIES"
      PERF_JSON="$SNAPSHOT_ROOT/oha.json"
      log_command "$OHA_BIN" -n "$REQUEST_COUNT" -c 10 --no-tui --output-format json -m POST -H 'Content-Type: application/json' -Z "$REQUEST_BODIES" http://127.0.0.1:8765/v1/respond
      "$OHA_BIN" -n "$REQUEST_COUNT" -c 10 --no-tui --output-format json -m POST \
        -H 'Content-Type: application/json' -Z "$REQUEST_BODIES" \
        http://127.0.0.1:8765/v1/respond >"$PERF_JSON" 2>"$SNAPSHOT_ROOT/oha.stderr"
      PERF_RC=$?
      append_redacted "$SNAPSHOT_ROOT/oha.stderr" "$PERF_LOG"
      append_redacted "$PERF_JSON" "$PERF_LOG"
      read -r PERF_SENT PERF_2XX PERF_ERRORS PERF_P50 PERF_P95 PERF_P99 PERF_RPS < <(node -e '
        const fs=require("fs");
        try {
          const j=JSON.parse(fs.readFileSync(process.argv[1],"utf8"));
          const statuses=j.statusCodeDistribution??{};
          const sent=Object.values(statuses).reduce((a,b)=>a+b,0)+Object.values(j.errorDistribution??{}).reduce((a,b)=>a+b,0);
          const ok=Object.entries(statuses).filter(([k])=>/^2/.test(k)).reduce((n,[,v])=>n+v,0);
          const errors=Object.values(j.errorDistribution??{}).reduce((a,b)=>a+b,0);
          const m=j.metrics??{};
          console.log(sent,ok,errors,m.latency_ms?.p50??"na",m.latency_ms?.p95??"na",m.latency_ms?.p99??"na",m.requests_per_sec??"na");
        } catch { console.log("0 0 0 na na na na"); }
      ' "$PERF_JSON")
      if (( PERF_SENT != REQUEST_COUNT || PERF_2XX != REQUEST_COUNT || PERF_ERRORS != 0 )); then
        PERF_RC=1
        PERF_FINDINGS=$((REQUEST_COUNT - PERF_2XX + PERF_ERRORS))
      fi
      printf 'METRICS requests=%s 2xx=%s errors=%s p50_ms=%s p95_ms=%s p99_ms=%s throughput_rps=%s identity_corpus=%s adapter=memory rates=zero scope=backend-overhead-only\n' \
        "$PERF_SENT" "$PERF_2XX" "$PERF_ERRORS" "$PERF_P50" "$PERF_P95" "$PERF_P99" "$PERF_RPS" "$REQUEST_BODY_COUNT"
    else
      printf 'Startup failed; load was not sent. See %s\n' "$STARTUP_LOG" >"$PERF_LOG"
      PERF_RC=1
    fi
  fi
fi
record performance.startup curl "$(curl --version | awk 'NR==1 {print $2}')" "$STARTUP_RC" 2 health_checks "$((STARTUP_RC == 0 ? 0 : 1))" "$STARTUP_LOG"
record performance.respond-load oha "$OHA_VERSION" "$PERF_RC" "$REQUEST_COUNT" requests "$PERF_FINDINGS" "$PERF_LOG"

printf '\n[Gate 5: pre-commit hooks]\n'
HOOK_LOG="$LOG_DIR/pre-commit.log"
HOOK_FILES=$(git ls-files | rg -c '^(apps/[^/]+|backend/[^/]+-sidecar)/src/.*\.tsx?$|^backend/relay-py/.*\.py$|^(apps/mobile/src|backend/(gateway-sidecar|relay-py|relay-rs)/src)/' || true)
HOOK_FILES=${HOOK_FILES:-0}
HOOK_COUNT=$(rg -c '^      - id:' .pre-commit-config.yaml || true)
HOOK_COUNT=${HOOK_COUNT:-0}
PRECOMMIT_HOME="$TOOL_DIR/pre-commit-cache"
mkdir -p "$PRECOMMIT_HOME"
log_command env "PRE_COMMIT_HOME=$PRECOMMIT_HOME" "$PY_TOOL_ENV/bin/pre-commit" validate-config
env "PRE_COMMIT_HOME=$PRECOMMIT_HOME" "$PY_TOOL_ENV/bin/pre-commit" validate-config 2>&1 | node "$REDACTOR" >"$HOOK_LOG"
HOOK_VALIDATE_RC=${PIPESTATUS[0]}
log_command env "PRE_COMMIT_HOME=$PRECOMMIT_HOME" "$PY_TOOL_ENV/bin/pre-commit" run --all-files --show-diff-on-failure
env "PRE_COMMIT_HOME=$PRECOMMIT_HOME" "$PY_TOOL_ENV/bin/pre-commit" run --all-files --show-diff-on-failure 2>&1 | node "$REDACTOR" >>"$HOOK_LOG"
HOOK_RUN_RC=${PIPESTATUS[0]}
HOOK_RC=0
(( HOOK_VALIDATE_RC == 0 && HOOK_RUN_RC == 0 )) || HOOK_RC=1
HOOK_FINDINGS=$(rg -c 'Failed$' "$HOOK_LOG" || true)
HOOK_FINDINGS=${HOOK_FINDINGS:-0}
record hooks.pre-commit pre-commit "$PRECOMMIT_VERSION" "$HOOK_RC" "$HOOK_COUNT" "hooks/${HOOK_FILES}_eligible_files" "$HOOK_FINDINGS" "$HOOK_LOG"

printf '\nGate logs: %s\n' "$LOG_DIR"
printf 'Commands: %s\n' "$COMMAND_LOG"
printf 'OVERALL %s\n' "$([[ $OVERALL -eq 0 ]] && printf PASS || printf FAIL)"
exit "$OVERALL"
