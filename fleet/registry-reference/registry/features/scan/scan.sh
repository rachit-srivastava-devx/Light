#!/usr/bin/env bash
# scan.sh — thin adapter running semgrep + gitleaks + trivy, emitting one verdict and one receipt.
# ADOPT.md capability B: gates/08-security exits 3 (no engine installed); this is the standalone
# engine, built so the gate pipeline can call it without depending on gates/ (being replaced).
# Authors no rules, reimplements no analysis: shells out to three real engines and reads their
# real JSON. Findings are parsed from each tool's own report, not trusted from its exit code alone
# (gitleaks/trivy exit codes are neutralised with --exit-code so this script is the one place the
# verdict is decided).
#
# THE INVARIANT THIS EXISTS TO ENFORCE: fleet already shipped a gate that read zero rows and
# reported PASS (gates/07-lessons, lessons[0]). A scan that touches zero files must FAIL, not look
# clean. Checked twice: once on the shared root before any tool runs, once against semgrep's own
# `.paths.scanned` count after it runs.
#
# Usage: scan.sh [path]   # default: repo root
# Exit:  0 clean · 1 real finding · 2 usage · 3 missing tool · 4 unparseable · 6 invariant (empty root)
set -u
D="$(cd "$(dirname "$0")/../../.." && pwd)"
# shellcheck source=../../registry/lib/err.sh
. "$D/registry/lib/err.sh"
# shellcheck source=../../registry/lib/receipt.sh
. "$D/registry/lib/receipt.sh"
# shellcheck source=../../registry/lib/toon.sh
. "$D/registry/lib/toon.sh"

case "${1:-}" in -h|--help) printf '%s\n' 'scan.sh [path]  # semgrep + gitleaks + trivy -> one verdict'; exit 0 ;; esac
[ $# -le 1 ] || die "$ERR_USAGE" too_many_args 'scan.sh takes at most one path argument' 'scan.sh [path]'
ROOT="$(cd "${1:-$D}" 2>/dev/null && pwd)" || die "$ERR_USAGE" bad_root "scan root does not exist: ${1:-$D}" 'scan.sh [path]'

require_bin semgrep 'brew install semgrep'
require_bin gitleaks 'brew install gitleaks'
require_bin trivy 'brew install trivy'
require_bin jq 'brew install jq'

now_iso() { [ -n "${NOW:-}" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }
GEN="scan"; VER="scan-verifier"   # I1: named + distinct; this adapter runs no model.
TMP="$(mktemp -d "${TMPDIR:-/tmp}/fleet-scan.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

_fail_early() { # machine_code human_message exit_code
  receipt_append "$(now_iso)" scan_error scan "$ROOT" "$GEN" "$VER" "$3" 0 0 0 >/dev/null 2>&1 || true
  die "$3" "$1" "$2" "inspect $TMP before it is removed, or rerun with --help"
}

# Pass 1 of the empty-input invariant: refuse before spending a single scanner invocation on it.
file_count="$(find "$ROOT" -type f 2>/dev/null | wc -l | tr -d ' ')"
[ "$file_count" -gt 0 ] || _fail_early empty_scan_root "scan root has zero files: $ROOT" "$ERR_INVARIANT"

semgrep scan --config p/ci --json --quiet "$ROOT" >"$TMP/semgrep.json" 2>"$TMP/semgrep.err"; sg_ec=$?
gitleaks detect --source "$ROOT" --no-git --no-banner --exit-code 0 \
  --report-format json --report-path "$TMP/gitleaks.json" >"$TMP/gitleaks.err" 2>&1; gl_ec=$?
trivy fs --scanners vuln,misconfig --format json -o "$TMP/trivy.json" "$ROOT" >"$TMP/trivy.err" 2>&1; tv_ec=$?

sg_scanned="$(jq -r '(.paths.scanned // []) | length' "$TMP/semgrep.json" 2>/dev/null)" || sg_scanned=""
sg_findings="$(jq -r '(.results // []) | length' "$TMP/semgrep.json" 2>/dev/null)" || sg_findings=""
gl_findings="$(jq -r 'length' "$TMP/gitleaks.json" 2>/dev/null)" || gl_findings=""
tv_findings="$(jq -r '[(.Results // [])[] | ((.Vulnerabilities // []) + (.Misconfigurations // []))[]] | length' "$TMP/trivy.json" 2>/dev/null)" || tv_findings=""

# Any engine whose output did not parse to a clean non-negative integer is a tool failure, never
# a silent 0 -- the exact shape of the lessons[0] bug, generalised to three engines at once.
for n in "$sg_scanned" "$sg_findings" "$gl_findings" "$tv_findings"; do
  case "$n" in ''|*[!0-9]*) _fail_early scan_unparseable \
    "an engine's output was not parseable JSON (exit codes: semgrep=$sg_ec gitleaks=$gl_ec trivy=$tv_ec)" "$ERR_PARSE" ;;
  esac
done

# Pass 2: semgrep's own account of what it touched, on a root already proven non-empty.
[ "$sg_scanned" -gt 0 ] || _fail_early semgrep_scanned_nothing \
  "semgrep reported zero scanned files under a non-empty root ($file_count files)" "$ERR_INVARIANT"

total=$((sg_findings + gl_findings + tv_findings))
verdict="PASS"; ec="$ERR_OK"
[ "$total" -eq 0 ] || { verdict="FAIL"; ec="$ERR_GENERIC"; }

toon_preamble scan 'semgrep + gitleaks + trivy, one verdict' "$(now_iso)"
toon_open scan 1 'root,files_scanned,semgrep_findings,gitleaks_findings,trivy_findings,total,verdict'
toon_row "$ROOT" "$sg_scanned" "$sg_findings" "$gl_findings" "$tv_findings" "$total" "$verdict"

receipt_append "$(now_iso)" "scan_$verdict" scan "$ROOT" "$GEN" "$VER" "$ec" 0 0 0 >/dev/null 2>&1 || true
exit "$ec"
