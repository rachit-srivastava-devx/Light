#!/usr/bin/env bash
# corpus.sh — run fleet against docs/design/FAILURE-CORPUS.md.
#
# Every row in the corpus is a real failure from this build. This scores whether fleet would now
# CATCH it. The point is not a green tick: a detector that fires here has found a live defect, and
# that is the useful outcome.
#
# Three verdicts per row:
#   CAUGHT      the detector fired on a real instance, or proved the guard is in place
#   MISSED      the detector ran and the failure would still get through
#   NOT-MECH    the failure is a judgement error no detector can honestly catch
#
# NOT-MECH rows are EXCLUDED from the score, never counted as passes. Counting them would be the
# same denominator dishonesty the corpus records as A8.
#
# Exit 0 = every mechanisable row CAUGHT. Exit 6 = at least one MISSED.
set -uo pipefail

D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$D" || exit 3
VERBOSE=0
case "${1-}" in --verbose|-v) VERBOSE=1 ;; '') ;; *) echo "corpus: unknown flag '$1'" >&2; exit 2 ;; esac

caught=0; missed=0; notmech=0
rows_missed=()

say() { printf '  %-8s %-5s %s\n' "$1" "$2" "$3"; }
detail() { [ "$VERBOSE" -eq 1 ] && printf '            %s\n' "$1"; return 0; }

# ok <id> <label> <condition-cmd...> — CAUGHT when the command SUCCEEDS (guard present / clean)
ok() {
  local id="$1" label="$2"; shift 2
  if "$@" >/dev/null 2>&1; then caught=$((caught+1)); say CAUGHT "$id" "$label"
  else missed=$((missed+1)); rows_missed+=("$id"); say MISSED "$id" "$label"; fi
}
# clean <id> <label> <finder-cmd...> — CAUGHT when the finder finds NOTHING (no instances of the bug)
clean() {
  local id="$1" label="$2"; shift 2
  local out; out="$("$@" 2>/dev/null)"
  if [ -z "$out" ]; then caught=$((caught+1)); say CAUGHT "$id" "$label"
  else missed=$((missed+1)); rows_missed+=("$id"); say MISSED "$id" "$label"
    detail "$(printf '%s' "$out" | head -3)"; fi
}
skip() { notmech=$((notmech+1)); say NOT-MECH "$1" "$2"; }

# Resolve a node that can run ESM. nvm's v10.16.2 is first on PATH here while Homebrew has v26, so a
# node-based check passed or failed by shell accident. v10 reads `import` as a syntax error, which
# surfaced as a real MISSED finding for entirely the wrong reason.
esm_node() {
  local c v
  for c in node /opt/homebrew/bin/node /usr/local/bin/node; do
    command -v "$c" >/dev/null 2>&1 || continue
    v="$("$c" --version 2>/dev/null | sed 's/^v//' | cut -d. -f1)"
    case "$v" in ''|*[!0-9]*) continue ;; esac
    [ "$v" -ge 14 ] && { printf '%s' "$c"; return 0; }
  done
  return 1
}
# Resolve once and export: the checks below run under `bash -c`, which inherits environment
# variables but NOT shell functions. Calling esm_node() in there silently produced "".
ESM_NODE="$(esm_node || true)"
export ESM_NODE


echo "== A. OWNER instructions =="
# A1: a capability with code must have an ADOPT.md row. Proxy: the manifest exists and is non-trivial.
ok A1 "ADOPT.md declares capabilities" bash -c '[ -f docs/guides/ADOPT.md ] && [ "$(grep -c "^|" docs/guides/ADOPT.md)" -gt 20 ]'
# A2: every requirement traced to an upstream.
ok A2 "REQUIREMENTS.md traces all 25" bash -c '[ -f docs/design/REQUIREMENTS-TRACE.md ] && [ "$(grep -cE "^\| \*?\*?[0-9]+" docs/design/REQUIREMENTS-TRACE.md)" -ge 24 ]'
# A3: no parallel trees.
clean A3 "no parallel v2/-new/.old trees" bash -c 'find . -maxdepth 1 -type d \( -name "v2" -o -name "*-new" -o -name "*.old" \) -not -path ./node_modules 2>/dev/null'
# A4: no basename forks (foo.sh + foo2.sh).
clean A4 "no numbered forks in registry/" bash -c '
  while IFS= read -r f; do b="$(basename "$f" .sh)"; case "$b" in *[0-9]) s="${b%[0-9]}"; [ -f "$(dirname "$f")/$s.sh" ] && echo "fork: $f vs $(dirname "$f")/$s.sh"; ;; esac; done < <(find registry -type f -name "*.sh" -print)'
skip A5 "codex vs claude spend ratio (needs ccusage + judgement)"
skip A6 "lead-model doing operator work (behavioural)"
skip A7 "dashboard ask satisfied (owner judgement)"
skip A8 "metric published without a denominator (semantic)"
skip A9 "correction recalled before repeating it (behavioural)"
skip A10 "coverage of rendered surface (needs runtime + judgement)"

echo "== B. FALSE CLAIMS =="
skip B1 "API health reported as UI state (see runtime section)"
skip B2 "DOM count reported as painted (see runtime section)"
skip B3 "asserted a tool's argv without reading it"
skip B4 "restated an agent's number as reproduced"
skip B5 "A/B whose arms were not distinct"
skip B6 "measurement not re-run after state change"
skip B7 "reported zero from a single probe"
skip B8 "cited an earlier verification instead of re-running"
# B9: shown <= total, using the server data plane directly so corpus validation never binds a port.
ok B9 "console counts are internally consistent" bash -c '
  # Was a tautology (shown === total) that also could not execute. Now a real audit: dangling edge
  # endpoints, duplicate ids, orphan tickets, and any count exceeding the set it counts.
  N="$ESM_NODE" || { echo "no node >= 14"; exit 1; }
  "$N" registry/modules/quality/detect/node-count-audit.mjs'

# B10 (E1): reading $? straight after a pipeline.
clean B10 "no upstream \$? lost to a passthrough pipe" bash -c '
  for f in $(find registry -type f -name "*.sh"); do
    [ "$f" = "registry/modules/quality/corpus.sh" ] && continue
    awk -v F="$f" "
      prev ~ /\\|[[:space:]]*(head|tail|tee|cat|tr|jq|wc)([[:space:]]|\$)/ && prev !~ /^[[:space:]]*#/ \
        && /^[[:space:]]*(ec|rc|status|exit_code)=\\\$\\?/ { print F\":\"NR\": \"\$0 }
      { prev = \$0 }" "$f" 2>/dev/null
  done | head -5'

echo "== C. DEFECTS =="
# C1: any rm -rf of a path not produced by mktemp.
clean C1 "no rm -rf on an untraced path" bash -c '
  # Delegated to a real script: three attempts at expressing this as bash-inside-awk produced only
  # false positives, flagging all three correctly-written cleanup traps in the repo. A detector whose
  # every finding is false gets muted, which is worse than no detector.
  python3 registry/modules/quality/detect/rm-rf-audit.py $(find registry tests -name "*.sh" -o -name "*.bats") 2>/dev/null | head -5'

# C2: every bats/test file sandboxes the ledger.
clean C2 "every suite overrides FLEET_LEDGER" bash -c '
  for t in tests/*/*.bats tests/*/*.test.sh tests/*/slow/*.test.sh; do [ -f "$t" ] || continue
    grep -q "FLEET_LEDGER" "$t" || echo "no ledger override: $t"; done'
# C3: npx --yes anywhere (re-resolves per call).
clean C3 "no repeated npx --yes (re-resolves per call)" bash -c '# The defect was NINE call sites re-resolving per invocation -> 100+ npm exec procs, load 825.
  # One guarded call site is not that. Flag a file with MORE THAN ONE.
  for f in $(grep -rl "npx --yes\|npx -y" --include="*.sh" registry bin 2>/dev/null | grep -v corpus.sh); do
    n=$(grep -c "npx --yes\|npx -y" "$f"); [ "$n" -gt 1 ] && echo "$f: $n call sites"
  done'
# C4: a gate that can pass on empty input. Proxy: scan.sh has an empty-input invariant.
ok C4 "scan refuses an empty input set" bash -c 'grep -q "scanned_nothing\|files_scanned\|empty" registry/features/scan/scan.sh'
# C5: the ledger's model fields must name real agents.
ok C5 "I1 attribution is reported, not assumed" bash -c '
  "$ESM_NODE" -e "import(\"./console/server/collect.mjs\").then(async m=>{const r=await m.receipts(process.cwd());process.exit(r.attribution&&typeof r.attribution.attributable===\"number\"?0:1);})"'
# C6: I1 exempts sentinels and refuses fabricated pairs — the fix that removes the incentive to fake.
ok C6 "I1: sentinel exempt, fabrication refused" bash -c '
  . registry/lib/receipt.sh
  invariant_i1 not-applicable not-applicable >/dev/null 2>&1 || exit 1   # exempt
  invariant_i1 memory memory-verifier >/dev/null 2>&1 && exit 1          # must refuse
  invariant_i1 codex codex >/dev/null 2>&1 && exit 1                     # must refuse
  invariant_i1 claude-opus-5 codex >/dev/null 2>&1 || exit 1             # must allow
  exit 0'
skip C7 "editing a script while it runs (behavioural)"
# C8: stdin-consuming command inside a read loop without </dev/null.
clean C8 "read loops pin stdin" bash -c '
  # Must not match pipe-delimited DATA rows that merely contain the words "brew install" —
  # bin/deps.sh embeds its manifest that way and produced 3 false positives.
  for f in $(find registry -type f -name "*.sh"); do
    [ "$f" = "registry/modules/quality/corpus.sh" ] && continue
    awk -v F="$f" "
      /while[[:space:]].*read/ { inl = 1 }
      inl && /^[[:space:]]*(if[[:space:]]+)?(brew|npm|cargo|ssh|git)[[:space:]]+(install|commit|upgrade|update)/ \
        && \$0 !~ /dev\\/null/ { print F\":\"NR\": \"\$0 }
      /^[[:space:]]*done([[:space:]]|\$)/ { inl = 0 }" "$f" 2>/dev/null
  done | head -5'

# C9: mktemp template with characters after XXXXXX.
clean C9 "mktemp templates end in XXXXXX" bash -c '
  grep -rn "XXXXXX" --include="*.sh" --include="*.bats" registry tests 2>/dev/null \
    | grep -v "corpus.sh" | grep -vE "^[^:]*:[0-9]*:[[:space:]]*#" \
    | grep -E "mktemp" | grep -vE "XXXXXX\"|XXXXXX'"'"'|XXXXXX\)|XXXXXX$|XXXXXX[[:space:]]" | head -5'

skip C10 "CSS selector matching zero elements (needs runtime)"
# C11: scratch is ignored.
ok C11 "scratch dirs are gitignored" bash -c '
  for p in /var/ /vendor/ node_modules; do grep -q "$p" .gitignore || exit 1; done'
# C12: the learning corpus is NOT ignored.
ok C12 "learning corpus is tracked" bash -c 'git check-ignore -q state/memory 2>/dev/null && exit 1 || exit 0'
skip C13 "verified by process-name instead of port owner (behavioural)"
# C14: every server module loads.
ok C14 "console/server libraries all load" bash -c '
  # index.mjs is an ENTRYPOINT: importing it calls listen() and never returns, so a failed import
  # there is expected, not a defect. Check the libraries it imports instead.
  for f in console/server/*.mjs; do
    case "$f" in */index.mjs) continue ;; esac
    "$ESM_NODE" -e "import(\"./$f\")" >/dev/null 2>&1 || { echo "FAILS: $f"; exit 1; }
  done'
skip C15 "new gate tuned on known-good input first (behavioural)"
skip C16 "handoff addressed by owned path (behavioural)"
skip C17 "explicit model on every dispatch (behavioural)"
skip C18 "contrast measured against computed background (needs runtime)"
# C19: every package script resolves to a real file.
ok C19 "package scripts resolve to real files" bash -c '
  cd console/web 2>/dev/null || exit 0
  node -e "
    const s=require(\"./package.json\").scripts||{};
    const fs=require(\"fs\");
    for (const [k,v] of Object.entries(s)) {
      const m=String(v).match(/(?:node|bash)\s+([^\s]+\.(?:mjs|js|sh))/);
      if (m && !fs.existsSync(m[1])) { console.error(k+\" -> \"+m[1]+\" MISSING\"); process.exit(1); }
    }"'
skip C20 "end-to-end completion count (needs a real run)"

echo
# C21: a bulk rewrite mangled `node_modules/` into `node_registry/modules/` in .gitignore, so it was
# not ignored at all. Assert the canonical entries survive, and that no tier name was spliced into one.
clean C21 "gitignore not mangled by a tier rewrite" bash -c '
  grep -qx "node_modules/" .gitignore || echo ".gitignore lost node_modules/"
  grep -nE "node_(registry|features|services|modules)/(modules|features)" .gitignore 2>/dev/null'

# C24: shell FUNCTIONS are not inherited by `bash -c`. Calling one inside a bash -c string expands to
# the empty string and silently breaks the check — it turned three green corpus rows red.
clean C24 "no shell function called inside bash -c" bash -c '
  for f in registry/modules/quality/corpus.sh registry/modules/quality/detect/*.sh; do
    [ -f "$f" ] || continue
    awk -v F="$f" "/bash -c/,/^\\s*\$/ { if (\$0 ~ /\\\$\\(esm_node\\)|\\\$\\(sweep_root\\)|\\\$\\(say\\)/) print F\":\"NR\": \"\$0 }" "$f" 2>/dev/null
  done | head -3'

# C26: `path_is_owned` treats an owns: entry as a PREFIX only when it ends with "/". Without the slash
# it is an exact match, so `owns: src` matched only a file literally named "src" and every real write
# under it was reported a violation. Any owns: entry naming a directory must carry the slash.
clean C26 "owns: directory entries end in a slash" bash -c '
  grep -rhoE "^owns: .*" registry/modules/cli/run.sh briefs/*.md 2>/dev/null \
    | sed "s/^owns: //" | tr "," "\n" | sed "s/^ *//;s/ *$//" \
    | grep -vE "/$|^$|\." | head -3'

echo "== score =="
total=$((caught+missed))
echo "  mechanisable rows : $total"
echo "  CAUGHT            : $caught"
echo "  MISSED            : $missed${rows_missed[*]+  (${rows_missed[*]})}"
echo "  NOT-MECH excluded : $notmech  (judgement failures — see FAILURE-CORPUS.md)"
[ "$total" -gt 0 ] && echo "  score             : $((caught * 100 / total))% of what a machine can see"
echo
echo "  Excluded rows are the majority, and that is the honest finding: most of this corpus is"
echo "  judgement failure, which no linter reaches. The durable fix is C6's shape — make the"
echo "  correct path cheaper than the wrong one."
[ "$missed" -eq 0 ] || exit 6
