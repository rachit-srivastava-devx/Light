#!/usr/bin/env bash
set -uo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
LOG=${1:-"$ROOT/.gate-logs/reachability.log"}
REPORT=$(mktemp "/tmp/orb-reachability-report.XXXXXX")
mkdir -p "$(dirname "$LOG")"

cleanup() {
  if [[ "$REPORT" == /tmp/orb-reachability-report.* || "$REPORT" == /private/tmp/orb-reachability-report.* ]]; then
    rm -f -- "$REPORT"
  fi
}
trap cleanup EXIT INT TERM

COMMAND=(node "$ROOT/tooling/reachability-check.mjs" --root "$ROOT" --config "$ROOT/tooling/reachability.config.json" --json)
if [[ ${ORB_REACHABILITY_SKIP_CLIPPY:-0} == 1 ]]; then COMMAND+=(--skip-clippy); fi

"${COMMAND[@]}" >"$REPORT" 2>"$LOG"
CHECK_RC=$?
if [[ -s "$REPORT" ]]; then
  cp "$REPORT" "$LOG"
fi

SUMMARY=$(node -e '
  const fs = require("fs");
  try {
    const report = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    const d = report.denominators;
    const status = report.status === "PASS" && Number(d.symbolsScanned) > 0 ? "PASS" : "FAIL";
    console.log([
      status,
      report.tool.typescript,
      d.symbolsScanned,
      d.importersResolved,
      d.callSitesScanned,
      d.wireFieldsScanned,
      d.thresholdParametersScanned,
      d.suppressionsFound,
      d.rustFilesScanned,
      d.rustToolRuns,
      d.allowlistSize,
      report.findings.length,
    ].join(" "));
  } catch {
    console.log("FAIL unknown 0 0 0 0 0 0 0 0 0 1");
  }
' "$REPORT")
read -r STATUS VERSION SYMBOLS IMPORTERS CALLS WIRE_FIELDS THRESHOLDS SUPPRESSIONS RUST_FILES RUST_RUNS ALLOWLIST FINDINGS <<<"$SUMMARY"
if [[ "$STATUS" != PASS ]]; then CHECK_RC=1; fi

printf '%s %-24s tool=%s@%s denominator=%s_symbols/%s_importers/%s_calls/%s_wire_fields/%s_threshold_params/%s_suppressions/%s_rust_files/%s_rust_runs/%s_allowlist findings=%s log=%s\n' \
  "$STATUS" reachability ts-compiler-api+python-ast+cargo-clippy "$VERSION" \
  "$SYMBOLS" "$IMPORTERS" "$CALLS" "$WIRE_FIELDS" "$THRESHOLDS" "$SUPPRESSIONS" \
  "$RUST_FILES" "$RUST_RUNS" "$ALLOWLIST" "$FINDINGS" "$LOG"
exit "$CHECK_RC"
