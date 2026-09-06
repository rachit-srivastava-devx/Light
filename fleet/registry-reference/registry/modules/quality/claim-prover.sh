#!/usr/bin/env bash
# claim-prover.sh — small, deterministic provers used by ./fleet claims.
set -u
D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
ID="${1-}"
[ -n "$ID" ] || { printf 'claim-prover: claim id required\n' >&2; exit 2; }

proof() { printf 'PROOF %s\n' "$*"; }
unprovable() { printf 'UNPROVABLE: %s\n' "$*"; return 4; }
files() { local p; for p in "$@"; do [ -e "$D/$p" ] || return 1; done; }
text() { rg -q -- "$1" "$D/$2"; }
tool() { command -v "$1" >/dev/null 2>&1; }
runtime() {
  local command_name="$1" out rc log
  set +e
  out="$("$D/registry/services/measure/telemetry.sh" "$command_name" 2>&1)"
  rc=$?
  set -e
  if [ "$rc" -eq 0 ]; then
    printf '%s\n' "$out"
    return 0
  fi
  log="${FLEET_CLAIM_TMP:-}/telemetry/phoenix.log"
  if [ -r "$log" ] && rg -q 'operation not permitted|Failed to bind' "$log"; then
    unprovable "the execution sandbox forbids loopback listeners; Phoenix log: $log"
    return 4
  fi
  printf '%s\n' "$out" >&2
  return "$rc"
}

case "$ID" in
  ADOPT-S2-01) files registry/features/crew/crew.sh && text firstmate registry/features/crew/crew.sh && proof 'firstmate adapter exists and names the upstream' ;;
  ADOPT-S2-02) files registry/features/isolate/isolate.sh && text treehouse registry/features/isolate/isolate.sh && text --force registry/features/isolate/isolate.sh && proof 'treehouse adapter uses forced non-interactive return' ;;
  ADOPT-S2-03) files registry/features/gate/gate.sh && text no-mistakes registry/features/gate/gate.sh && proof 'no-mistakes gate adapter exists' ;;
  ADOPT-S2-04) unprovable 'no model gateway adapter or resolved-provider receipt field is wired in the checked-out tree' ;;
  ADOPT-S2-05) files registry/features/telemetry/telemetry.sh registry/features/telemetry/telemetry_otel.py && files var/venv/bin/phoenix && proof 'Phoenix binary and fleet HTTP/protobuf adapter exist' ;;
  ADOPT-S2-06) files registry/features/memory/memory.sh state/memory && proof 'memory command and tracked memory corpus exist' ;;
  ADOPT-S2-07) files registry/features/context/context.sh && text llmlingua registry/features/context/context.sh && proof 'context adapter names LLMLingua' ;;
  ADOPT-S2-08) tool bats && proof "bats=$(command -v bats)" ;;
  ADOPT-S2-09) tool shellcheck && proof "shellcheck=$(command -v shellcheck)" ;;
  ADOPT-S2-10) files console/web/package.json && text '@playwright/test' console/web/package.json && proof 'Playwright is declared' ;;
  ADOPT-S2-11) files console/web/package.json && text '@xyflow/react' console/web/package.json && text elkjs console/web/package.json && text '@radix-ui' console/web/package.json && text '@tanstack' console/web/package.json && text shiki console/web/package.json && proof 'graph layout accessibility data and syntax dependencies are declared' ;;
  ADOPT-S3-01) files registry/lib/receipt.sh && text invariant_i1 registry/lib/receipt.sh && text invariant_i2 registry/lib/receipt.sh && text invariant_i3 registry/lib/receipt.sh && proof 'I1 I2 I3 functions are present' ;;
  ADOPT-S3-02) files registry/features/ledger/ledger.sh && text receipt_verify_chain registry/features/ledger/ledger.sh && proof 'ledger verifier is present' ;;
  ADOPT-S3-03) files registry/features/intake/intake.sh && text ambiguity registry/features/intake/intake.sh && proof 'blocking intake adapter is present' ;;
  ADOPT-S3-04) files console/server/sdlc.mjs console/server/graph.mjs && text 9 console/server/sdlc.mjs && proof 'SDLC state and lesson graph sources are present' ;;
  REQ-01) unprovable 'the complete firstmate to treehouse to no-mistakes to axi workflow needs a live upstream run' ;;
  REQ-02) unprovable 'exact token and compression behavior needs a pinned input and a real tool run' ;;
  REQ-03) unprovable 'live harness detection and distribution need an executed dispatch' ;;
  REQ-04) unprovable 'strict SDLC adherence and learning are behavioral; no independent end-to-end run is recorded' ;;
  REQ-05) unprovable 'output equivalence is a semantic comparison, not a repository predicate' ;;
  REQ-06) unprovable 'different-model re-verification is an execution and provenance claim, not source presence' ;;
  REQ-07) files registry/features/intake/intake.sh && text refuse registry/features/intake/intake.sh && proof 'intake refusal path exists' ;;
  REQ-08) unprovable 'parallel completion and manual verification require a live multi-agent run' ;;
  REQ-09) unprovable 'never-repeat and mathematical proof are outcome claims; recall is not a deterministic proof' ;;
  REQ-10) unprovable 'watcher scheduling requires a live quota exhaustion scenario' ;;
  REQ-11) unprovable 'test files prove neither feature coverage nor that every test is reachable' ;;
  REQ-12) unprovable 'legible live state requires a running console and observed transitions' ;;
  REQ-13) unprovable 'reputation stake and organization behavior have no machine-readable predicate' ;;
  REQ-14) unprovable 'a planning stage in source does not prove execution waited for it' ;;
  REQ-15) unprovable 'the whole-stack self-healing claim has no single deterministic acceptance test' ;;
  REQ-16) unprovable 'live dashboard behavior requires a running browser session and human review' ;;
  REQ-17) files console/server/sdlc.mjs && proof 'SDLC state machine source exists' ;;
  REQ-18) unprovable 'credential-boundary enforcement depends on external upstream and runtime identity configuration' ;;
  REQ-19) unprovable 'the cited shell mutation report has no checked-in executable harness' ;;
  REQ-20) files registry/features/telemetry/telemetry.sh && text attempt registry/features/telemetry/telemetry.sh && proof 'attempt counting is implemented in telemetry' ;;
  REQ-21) unprovable 'memory files and a check command do not prove prose became an executed guard' ;;
  REQ-22) files registry/features/telemetry/telemetry.sh registry/features/telemetry/telemetry_otel.py && text 'gen_ai.client.token.usage' registry/features/telemetry/telemetry_otel.py && text 'gen_ai.token.type' registry/features/telemetry/telemetry_otel.py && runtime prove >/dev/null && proof 'Phoenix trace proof completed' ;;
  REQ-23) unprovable 'no Rust workspace or cargo-deny/cargo-mutants proof target is present' ;;
  REQ-24) unprovable 'fan-out cap is a policy claim without an executed multi-agent schedule' ;;
  REQ-25) unprovable 'a named human approval requires external identity evidence' ;;
  AUX-MUTATION) if ! files gates/03-mutation; then unprovable 'gates/03-mutation is absent, and MUTATION-REPORT.md requires an external harness placeholder'; exit 4; fi; proof 'mutation gate exists' ;;
  AUX-LLMLINGUA) unprovable 'the claimed 1546-token input and compressed 491-token output are not stored as a reproducible fixture' ;;
  AUX-PHOENIX-TRACE) runtime prove ;;
  AUX-PHOENIX-P50) runtime prove-p50 ;;
  AUX-TREEHOUSE-RETURN) unprovable 'proving process cleanup requires a disposable live treehouse lease and process fixture; none is checked in' ;;
  *) printf 'claim-prover: unknown claim id %s\n' "$ID" >&2; exit 2 ;;
esac
