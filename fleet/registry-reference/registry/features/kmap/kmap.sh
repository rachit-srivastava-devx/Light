#!/usr/bin/env bash
# shellcheck disable=SC2034,SC2100,SC2195,SC2206,SC2318
# kmap.sh — adapter over TWO engines: code-graph (Rust)+tokei for zones/coverage/
# verify/graph, and the tree-sitter+SQLite map (mapdb.py, via the sibling
# kmap-index.sh) for impact/history/index. The two do not overlap in what they
# answer: code-graph never learned Bash (fleet is mostly Bash) and its "impact"
# is an import/file graph, not a call graph; the SQLite map is the one with a
# real Bash/Python/TS call graph and commit history, so impact/history/index
# route there. zones/coverage/verify/graph stay on code-graph+tokei — nothing
# about that half of the adapter changed.
# Assumptions: bash 3.2, jq, shasum, and the adopted tools are on PATH or overridden
# with KMAP_CODE_GRAPH_CMD/KMAP_TOKEI_CMD. No graph traversal, closure, cycle detection,
# import parsing, or source enumeration is implemented here. Freshness uses tokei's file
# inventory plus BSD stat mtimes; content edits that preserve mtime are deliberately not handled.
set -u

D="$(cd "$(dirname "$0")/../../.." && pwd)"
HERE="$(cd "$(dirname "$0")" && pwd)"
. "$D/registry/lib/toon.sh"
. "$D/registry/lib/receipt.sh"
. "$D/registry/lib/err.sh"

ROOT="${FLEET_KMAP_DIR:-$D/kmap}"
LEDGER="${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}"
ENGINE="${KMAP_CODE_GRAPH_CMD:-code-graph}"
TOKEI="${KMAP_TOKEI_CMD:-tokei}"
KMAP_INDEX_SH="${KMAP_INDEX_SH:-$HERE/kmap-index.sh}"
NOW="${FLEET_NOW:-}"
JSON=0; FULL=0
WORDS=()

usage() { printf '%s\n' \
  'kmap.sh [--json] [--full] [subcommand] [args]' \
  '  (no args)                                        registered repos and freshness' \
  '  scan --repo PATH [--name ID]                      code-graph index + project registration' \
  '  graph [--repo NAME]                                code-graph file dependency export' \
  '  index [--repo PATH]                                build/update the call-graph map (default: this repo)' \
  '  impact PATH-or-SYMBOL [--repo PATH] [--depth N]    call-graph blast radius (Bash/Python/TS)' \
  '  history SYMBOL [--repo PATH]                       commit history for a symbol' \
  '  zones [--repo NAME]                               tokei inventory + fleet risk zones' \
  '  coverage [--repo NAME]                            tokei source/test inventory' \
  '  verify [--repo NAME]                               code-graph cycle check'; }

now_iso() { [ -n "$NOW" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }
words_of() { WORDS=( $1 ); }
first_word() { printf '%s' "$1" | awk '{print $1}'; }
require_engine() {
  local b; b="$(first_word "$ENGINE")"
  case "$b" in */*) [ -x "$b" ] || die 3 code_graph_missing "code-graph is required" 'cargo install code-graph-cli';; *) command -v "$b" >/dev/null 2>&1 || die 3 code_graph_missing \
    "code-graph is required" 'cargo install code-graph-cli';; esac
}
require_tokei() {
  local b; b="$(first_word "$TOKEI")"
  case "$b" in */*) [ -x "$b" ] || die 3 tokei_missing "tokei is required for file inventory" 'brew install tokei';; *) command -v "$b" >/dev/null 2>&1 || die 3 tokei_missing \
    "tokei is required for file inventory" 'brew install tokei';; esac
}
repo_abs() { [ -d "${1-}" ] || die 2 repo_missing "repo '$1' is not a directory" 'pass an existing --repo PATH'; (cd "$1" && pwd -P); }
valid_name() { case "${1-}" in ''|*[!A-Za-z0-9._-]*) return 1 ;; esac; }
meta_file() { printf '%s/%s/meta.json' "$ROOT" "$1"; }
require_repo() { [ -s "$(meta_file "$1")" ] || die 2 repo_not_registered "repo '$1' is not registered" 'run kmap.sh scan --repo PATH --name NAME'; }
meta() { jq -r --arg k "$2" '.[$k] // empty' "$(meta_file "$1")"; }
CHOSEN=''
select_repo() {
  local requested="${1-}" path count=0 name=''; CHOSEN="$requested"; [ -n "$requested" ] && return 0
  for path in "$ROOT"/*/meta.json; do [ -f "$path" ] || continue; count=$((count+1)); name="$(basename "$(dirname "$path")")"; done
  [ "$count" -eq 0 ] && { CHOSEN=''; return 0; }; [ "$count" -eq 1 ] && { CHOSEN="$name"; return 0; }
  die 7 repo_ambiguous 'more than one registered repo; --repo is required' 'pass --repo NAME'
}
emit_header() { toon_preamble kmap 'Rust-backed dependency knowledge map' "$(now_iso)"; }
receipt() {
  invariant_i1 not-applicable not-applicable || return 6
  FLEET_LEDGER="$LEDGER" receipt_append "$(now_iso)" "$1" kmap "$1" not-applicable not-applicable "$2" 0 0 0 >/dev/null || return 1
}

# Run a trusted operator command string; all semantic graph work stays in code-graph.
engine_run() {
  local out="$1" err="$2"; shift 2; words_of "$ENGINE"
  "${WORDS[@]}" "$@" >"$out" 2>"$err"; local ec=$?
  [ "$ec" -eq 0 ] || die 1 engine_failed "code-graph failed (exit $ec): $(tail -1 "$err")" 'run the same code-graph command with --help'
}
json_ok() { jq empty "$1" >/dev/null 2>&1 || die 4 engine_output_unparseable \
  'adopted tool output was not valid JSON' 'run code-graph with --format json and inspect stderr'; }

# Run the tree-sitter+SQLite engine (mapdb.py via kmap-index.sh). This is the
# only place that understands its exit-code vocabulary: 0 ok, 6 stale (an
# indexed file changed since index time — refuse, never answer from a stale
# or empty index), 2 refused (usually: this repo has no index yet). Anything
# else is treated as an engine crash, same as engine_run does for code-graph.
kmap_engine_run() {
  local out="$1" err="$2"; shift 2
  "$KMAP_INDEX_SH" --json "$@" >"$out" 2>"$err"
  local ec=$?
  case "$ec" in
    0) json_ok "$out"; return 0 ;;
    2) return 2 ;;
    6) return 6 ;;
    *) die 1 kmap_engine_failed "kmap engine failed (exit $ec): $(tail -1 "$err")" 'inspect the mapdb.py traceback' ;;
  esac
}
# A stale index must never answer confidently (T24 Defect 1): refuse loudly
# with the reasons mapdb.py gave, and the exact command to rebuild.
kmap_stale_die() {
  local out="$1" reasons; reasons="$(jq -r '(.reasons // []) | join("; ")' "$out" 2>/dev/null)"
  die 6 index_stale "kmap index is stale (${reasons:-reason unknown}); refusing to answer" \
    'rebuild it: ./fleet kmap index --repo <repo>'
}
# No index for this repo yet: name the exact command to build one. The user
# is never told about kmap-index.sh directly (that is an internal delegate).
kmap_index_missing_die() {
  local out="$1" repo="$2" msg; msg="$(jq -r '.error // empty' "$out" 2>/dev/null)"
  case "$msg" in
    path_unregistered*|'') die 2 kmap_index_missing "no kmap index found for repo '$repo'" \
      "build it: ./fleet kmap index --repo $repo" ;;
    *) die 2 kmap_engine_refused "kmap engine refused: $msg" \
      "build or rebuild the index: ./fleet kmap index --repo $repo" ;;
  esac
}

# Inventory is emitted by tokei, never by find/grep/file walking in this adapter.
inventory() {
  local repo="$1" out="$2" err; err="$out.err"; require_tokei; words_of "$TOKEI"
  "${WORDS[@]}" --files --output json "$repo" >"$out" 2>"$err"; local ec=$?
  [ "$ec" -eq 0 ] || die 1 tokei_failed "tokei failed (exit $ec): $(tail -1 "$err")" 'run tokei --help and inspect the repo'
  json_ok "$out"; rm -f "$err"
}
fresh_hash() {
  local repo="$1" inv="$2" tmp="$inv.mtime" rel m inv_hash mtime_hash; : >"$tmp"
  jq -r '.[]? | .reports[]? | .name? // empty' "$inv" >"$tmp.list" || die 4 tokei_inventory_unparseable 'tokei JSON has no readable file reports' 'run tokei --files --output json by hand'
  while IFS= read -r rel; do
    [ -n "$rel" ] || continue
    m="$(cd "$repo" 2>/dev/null && (stat -f %m "$rel" 2>/dev/null || stat -c %Y "$rel" 2>/dev/null) || printf 'missing')"
    printf '%s\t%s\n' "$rel" "$m" >>"$tmp"
  done <"$tmp.list"
  inv_hash="$(shasum -a 256 <"$inv" | awk '{print $1}')"; mtime_hash="$(shasum -a 256 <"$tmp" | awk '{print $1}')"
  printf '%s%s' "$inv_hash" "$mtime_hash" | shasum -a 256 | awk '{print $1}'
  rm -f "$tmp" "$tmp.list"
}
preview() { local n limit; n="$(wc -c <"$1" | tr -d ' ')"; limit=160; [ "$FULL" -eq 1 ] && limit=600; printf '%s\t' "$n"; head -c "$limit" "$1" | tr '\n' ' '; }
emit_payload() {
  local kind="$1" repo="$2" file="$3" bytes body; bytes="$(wc -c <"$file" | tr -d ' ')"
  if [ "$JSON" -eq 1 ]; then jq -Rn --arg kind "$kind" --arg repo "$repo" --argjson bytes "$bytes" \
    --rawfile payload "$file" '{kind:$kind,repo:$repo,bytes:$bytes,payload:$payload}'; return; fi
  emit_header; toon_open "$kind" 1 'repo,bytes,payload'; body="$(preview "$file")"; toon_row "$repo" "$bytes" "$body"; toon_next "rerun with --json for the complete adopted-tool payload" "rerun with --full for a larger preview"
}

cmd_default() {
  emit_header; local n=0 name path status repo inv hash json_rows; [ -d "$ROOT" ] || { toon_empty repos; toon_next 'run kmap.sh scan --repo PATH --name NAME'; return; }
  json_rows="$(mktemp "${TMPDIR:-/tmp}/kmap-default.XXXXXX")"
  for path in "$ROOT"/*/meta.json; do
    [ -f "$path" ] || continue; name="$(basename "$(dirname "$path")")"; status="$(meta "$name" repo_path)"; n=$((n+1));
    if [ -d "$status" ]; then
      repo="$status"; inv="$(mktemp "${TMPDIR:-/tmp}/kmap-default-inventory.XXXXXX")"; inventory "$repo" "$inv"; hash="$(fresh_hash "$repo" "$inv")"; rm -f "$inv"; [ "$hash" = "$(meta "$name" inventory_hash)" ] && status=fresh || status=stale
    else status=path-gone; fi
    if [ "$JSON" -eq 1 ]; then jq -n --arg name "$name" --arg status "$status" '{name:$name,status:$status}' >>"$json_rows"; continue; fi
    [ "$n" -eq 1 ] && toon_open repos 0 'name,status'
    toon_row "$name" "$status"
  done
  if [ "$JSON" -eq 1 ]; then jq -s --arg root "$ROOT" '{root:$root,repos:.}' "$json_rows"; rm -f "$json_rows"; return; fi
  rm -f "$json_rows"
  if [ "$n" -eq 0 ]; then toon_empty repos; fi
  toon_next 'run kmap.sh graph --repo NAME' 'run kmap.sh scan --repo PATH --name NAME'
}

cmd_scan() {
  local repo='' name='' force=0 abs dir inv idx err hash
  while [ "$#" -gt 0 ]; do case "$1" in --repo) need_val "$1" "${2-}"; repo="$2"; shift 2;; --name) need_val "$1" "${2-}"; name="$2"; shift 2;; --force) force=1; shift;; --help) usage; return;; *) reject_unknown_flag "$1";; esac; done
  [ -n "$repo" ] || die 2 missing_repo 'scan requires --repo PATH' 'pass --repo PATH'; abs="$(repo_abs "$repo")"; [ -n "$name" ] || name="$(basename "$abs")"; valid_name "$name" || die 2 invalid_name "invalid repo name '$name'" 'use letters, digits, dot, underscore, or hyphen'
  dir="$ROOT/$name"; [ -e "$dir" ] && [ "$force" -ne 1 ] && die 2 repo_name_collision "repo '$name' already exists" 'use --force to replace it'; mkdir -p "$dir"
  require_engine; inv="$dir/inventory.json"; inventory "$abs" "$inv"; jq -e 'has("Java") or has("Kotlin") or has("C") or has("C++") or has("C#") or has("Swift") or has("Ruby") or has("PHP")' "$inv" >/dev/null 2>&1 && die 4 unsupported_language 'code-graph does not index this repo language set' 'use a verified code-graph-supported language indexer'; hash="$(fresh_hash "$abs" "$inv")"; idx="$dir/index.json"; err="$idx.err"
  engine_run "$idx" "$err" index "$abs" --json; json_ok "$idx"; rm -f "$err"
  engine_run "$dir/project.json" "$dir/project.err" project add "$name" "$abs"; rm -f "$dir/project.err"
  jq -n --arg name "$name" --arg repo "$abs" --arg scanned "$(now_iso)" --arg hash "$hash" \
    '{name:$name,repo_path:$repo,scanned_at:$scanned,inventory_hash:$hash,engine:"code-graph"}' >"$dir/meta.json"
  receipt scan 0 || die 1 receipt_failed 'could not append scan receipt' 'set FLEET_LEDGER to a writable ledger'
  if [ "$JSON" -eq 1 ]; then jq -n --slurpfile index "$idx" --arg name "$name" --arg repo "$abs" '{name:$name,repo:$repo,index:$index[0]}'; else emit_header; toon_open scan 1 'name,repo,engine'; toon_row "$name" "$abs" 'code-graph'; toon_next "run kmap.sh impact PATH"; fi
}

cmd_graph() {
  local name='' arg repo out err; while [ "$#" -gt 0 ]; do case "$1" in --repo) need_val "$1" "${2-}"; name="$2"; shift 2;; --full) FULL=1; shift;; --help) usage; return;; *) reject_unknown_flag "$1";; esac; done
  select_repo "$name"; name="$CHOSEN"; [ -n "$name" ] || { emit_header; toon_empty graph; toon_next 'run kmap.sh scan --repo PATH --name NAME'; return; }; require_repo "$name"; repo="$(meta "$name" repo_path)"; [ -d "$repo" ] || die 2 repo_path_gone "repo path '$repo' no longer exists" 'rescan the repo at its new path'; require_engine
  out="$(mktemp "${TMPDIR:-/tmp}/kmap-graph.XXXXXX")"; err="$out.err"; engine_run "$out" "$err" export "$repo" --format dot --granularity file; rm -f "$err"; emit_payload graph "$name" "$out"; rm -f "$out"
}


# --- impact / history / index: the tree-sitter+SQLite engine (T24) ---------
# code-graph's own "impact" is retired: it is an import/file graph (not a
# call graph) and it silently skips Bash, which is most of this repo. These
# three subcommands are the ones mapdb.py actually answers, so they route to
# it via the sibling kmap-index.sh, defaulting --repo to this repo (D) so a
# user never has to pre-register a project to ask "what depends on this".

cmd_impact() {
  local target="${1-}" repo="$D" depth='' kind target_dir target_base out err rc target_count hits_n
  [ -n "$target" ] || die 2 missing_path 'impact requires PATH or SYMBOL' 'pass a file path or a bare symbol name'
  shift
  while [ "$#" -gt 0 ]; do case "$1" in
    --repo) need_val "$1" "${2-}"; repo="$2"; shift 2;;
    --depth) need_val "$1" "${2-}"; depth="$2"; shift 2;;
    --full) FULL=1; shift;;
    --help) usage; return;;
    *) reject_unknown_flag "$1";;
  esac; done
  repo="$(repo_abs "$repo")"
  case "$target" in
    */*) kind=file ;;
    *) if [ -e "$target" ]; then kind=file; else kind=symbol; fi ;;
  esac
  if [ "$kind" = file ]; then
    target_dir="$(dirname "$target")"; target_base="$(basename "$target")"
    [ -d "$target_dir" ] && target="$(cd "$target_dir" && pwd -P)/$target_base"
  fi
  out="$(mktemp "${TMPDIR:-/tmp}/kmap-impact.XXXXXX")"; err="$out.err"
  if [ "$kind" = file ]; then
    kmap_engine_run "$out" "$err" impact --file "$target" --repo "$repo" ${depth:+--depth "$depth"}
  else
    kmap_engine_run "$out" "$err" impact --symbol "$target" --repo "$repo" ${depth:+--depth "$depth"}
  fi
  rc=$?
  case "$rc" in
    6) kmap_stale_die "$out" ;;
    2) kmap_index_missing_die "$out" "$repo" ;;
  esac
  # T24 Defect 1: a target the index does not recognise at all must never
  # look like a normal "0 dependents" success (the old engine's silent
  # false-negative failure mode). target_count is mapdb.py's own count of how
  # many index rows matched the target identity, distinct from hits (how
  # many things depend on it) — 0 target_count means "not found", not "found
  # but has no dependents".
  target_count="$(jq -r '.target_count // 0' "$out")"
  if [ "$target_count" = 0 ]; then
    die 2 target_not_found "no symbol or file matching '$target' found in the kmap index for '$repo'" \
      "check spelling, or rebuild the index: ./fleet kmap index --repo $repo"
  fi
  hits_n="$(jq -r '.hits | length' "$out")"
  if [ "$JSON" -eq 1 ]; then cat "$out"; rm -f "$out" "$err"; return; fi
  emit_header
  toon_open impact "$hits_n" 'depth,confidence,path,symbol'
  jq -r '.hits[] | [.depth,.confidence,.path,(.symbol // .path)] | @tsv' "$out" | \
    while IFS=$'\t' read -r d c p s; do toon_row "$d" "$c" "$p" "$s"; done
  toon_next "impact: $hits_n dependents (target_count=$target_count, depth<=${depth:-8})" 'use --json for the complete payload'
  rm -f "$out" "$err"
}

cmd_history() {
  local target="${1-}" repo="$D" out err rc commits_n
  [ -n "$target" ] || die 2 missing_symbol 'history requires SYMBOL' 'pass a symbol name'
  shift
  while [ "$#" -gt 0 ]; do case "$1" in
    --repo) need_val "$1" "${2-}"; repo="$2"; shift 2;;
    --full) FULL=1; shift;;
    --help) usage; return;;
    *) reject_unknown_flag "$1";;
  esac; done
  repo="$(repo_abs "$repo")"
  out="$(mktemp "${TMPDIR:-/tmp}/kmap-history.XXXXXX")"; err="$out.err"
  kmap_engine_run "$out" "$err" history --symbol "$target" --repo "$repo"
  rc=$?
  case "$rc" in
    6) kmap_stale_die "$out" ;;
    2) kmap_index_missing_die "$out" "$repo" ;;
  esac
  commits_n="$(jq -r '.commits | length' "$out")"
  if [ "$JSON" -eq 1 ]; then cat "$out"; rm -f "$out" "$err"; return; fi
  emit_header
  if [ "$commits_n" -eq 0 ]; then
    toon_empty history
    toon_next "no commit touched symbol '$target' in '$repo'" "confirm the symbol name, or rebuild the index: ./fleet kmap index --repo $repo"
    rm -f "$out" "$err"; return
  fi
  toon_open history "$commits_n" 'hash,path,message'
  jq -r '.commits[] | [.hash,.path,((.message // "") | gsub("\n";" "))] | @tsv' "$out" | \
    while IFS=$'\t' read -r h p m; do toon_row "$h" "$p" "$m"; done
  toon_next "symbol=$target commits=$commits_n" 'use --json for the complete payload'
  rm -f "$out" "$err"
}

cmd_index() {
  local repo=''
  while [ "$#" -gt 0 ]; do case "$1" in
    --repo) need_val "$1" "${2-}"; repo="$2"; shift 2;;
    --help) usage; return;;
    *) reject_unknown_flag "$1";;
  esac; done
  [ -n "$repo" ] || repo="$D"
  repo="$(repo_abs "$repo")"
  local out err rc symbols edges commits
  out="$(mktemp "${TMPDIR:-/tmp}/kmap-index.XXXXXX")"; err="$out.err"
  "$KMAP_INDEX_SH" --json index --repo "$repo" >"$out" 2>"$err"; rc=$?
  [ "$rc" -eq 0 ] || die 1 kmap_index_failed "kmap-index.sh index failed (exit $rc): $(tail -1 "$err")" 'inspect the mapdb.py traceback'
  json_ok "$out"
  receipt index 0 || die 1 receipt_failed 'could not append index receipt' 'set FLEET_LEDGER to a writable ledger'
  if [ "$JSON" -eq 1 ]; then cat "$out"; rm -f "$out" "$err"; return; fi
  symbols="$(jq -r '.symbols' "$out")"; edges="$(jq -r '.edges' "$out")"; commits="$(jq -r '.commits' "$out")"
  emit_header
  toon_open index 1 'repo,symbols,edges,commits'
  toon_row "$repo" "$symbols" "$edges" "$commits"
  toon_next 'run ./fleet kmap impact SYMBOL-or-PATH' 'run ./fleet kmap history SYMBOL' 'use --json for the complete payload'
  rm -f "$out" "$err"
}

cmd_zones() {
  local name='' repo inv list rel zone reason; while [ "$#" -gt 0 ]; do case "$1" in --repo) need_val "$1" "${2-}"; name="$2"; shift 2;; --full) FULL=1; shift;; --rules) need_val "$1" "${2-}"; [ -f "$2" ] || die 2 rules_missing "rules file '$2' not found" 'pass an existing --rules FILE'; shift 2;; --help) usage; return;; *) reject_unknown_flag "$1";; esac; done
  select_repo "$name"; name="$CHOSEN"; [ -n "$name" ] || { emit_header; toon_empty zones; toon_next 'run kmap.sh scan --repo PATH --name NAME'; return; }; require_repo "$name"; repo="$(meta "$name" repo_path)"; inv="$(mktemp "${TMPDIR:-/tmp}/kmap-zones.XXXXXX")"; inventory "$repo" "$inv"; list="$inv.list"; jq -r '.[]? | .reports[]? | .name? // empty' "$inv" >"$list" || die 4 tokei_inventory_unparseable 'tokei JSON has no readable file reports' 'run tokei --files --output json by hand'
  emit_header; toon_open zones 0 'path,zone,reason'; while IFS= read -r rel; do zone=agent-ok; reason=default; case "/$rel/" in */auth/*|*/authn/*|*/authz/*) zone=auth; reason='path segment';; */billing/*|*/payments/*|*/checkout/*|*/pricing/*) zone=money; reason='path segment';; */migrations/*|*.sql) zone=schema-migration; reason='migration or SQL path';; esac; [ "$FULL" -eq 1 ] || [ "$zone" != agent-ok ] || continue; toon_row "$rel" "$zone" "$reason"; done <"$list"; rm -f "$inv" "$list"; toon_next 'use --full to include agent-ok files'
}

cmd_coverage() {
  local name='' repo inv list total tests source; while [ "$#" -gt 0 ]; do case "$1" in --repo) need_val "$1" "${2-}"; name="$2"; shift 2;; --help) usage; return;; *) reject_unknown_flag "$1";; esac; done
  select_repo "$name"; name="$CHOSEN"; [ -n "$name" ] || { [ "$JSON" -eq 1 ] && printf '{"repos":[]}\n' || { emit_header; toon_empty coverage; toon_next 'run kmap.sh scan --repo PATH --name NAME'; }; return; }; require_repo "$name"; repo="$(meta "$name" repo_path)"; inv="$(mktemp "${TMPDIR:-/tmp}/kmap-coverage.XXXXXX")"; inventory "$repo" "$inv"; list="$inv.list"; jq -r '.[]? | .reports[]? | .name? // empty' "$inv" >"$list" || die 4 tokei_inventory_unparseable 'tokei JSON has no readable file reports' 'run tokei --files --output json by hand'; total=0; tests=0; source=0
  while IFS= read -r rel; do [ -n "$rel" ] || continue; total=$((total+1)); case "/$rel/" in */test/*|*/tests/*|*test*.js|*test*.ts|*spec*.js|*spec*.ts) tests=$((tests+1));; *) source=$((source+1));; esac; done <"$list"; rm -f "$inv" "$list"
  if [ "$JSON" -eq 1 ]; then jq -n --arg repo "$name" --argjson files "$total" --argjson source "$source" --argjson test "$tests" '{repo:$repo,basis:"tokei inventory",files:$files,source_files:$source,test_files:$test}'; else emit_header; toon_open coverage 1 'repo,source_files,test_files'; toon_row "$name" "$source" "$tests"; toon_next 'coverage is inventory-only; use code-graph refs for symbol-level evidence'; fi
}

cmd_verify() {
  local name='' repo out err; while [ "$#" -gt 0 ]; do case "$1" in --repo) need_val "$1" "${2-}"; name="$2"; shift 2;; --help) usage; return;; *) reject_unknown_flag "$1";; esac; done
  select_repo "$name"; name="$CHOSEN"; [ -n "$name" ] || { emit_header; toon_empty verify; toon_next 'run kmap.sh scan --repo PATH --name NAME'; return; }; require_repo "$name"; repo="$(meta "$name" repo_path)"; require_engine; out="$(mktemp "${TMPDIR:-/tmp}/kmap-verify.XXXXXX")"; err="$out.err"; engine_run "$out" "$err" circular "$repo" --format json; rm -f "$err"; json_ok "$out"; emit_payload verify "$name" "$out"; rm -f "$out"
}

main() {
  while [ "$#" -gt 0 ]; do case "$1" in --json) JSON=1; shift;; --full) FULL=1; shift;; --help) usage; return 0;; --) shift; break;; *) break;; esac; done
  if [ "$#" -eq 0 ]; then cmd_default; return; fi
  case "$1" in scan) shift; cmd_scan "$@";; graph) shift; cmd_graph "$@";; index) shift; cmd_index "$@";; impact) shift; cmd_impact "$@";; history) shift; cmd_history "$@";; zones) shift; cmd_zones "$@";; coverage) shift; cmd_coverage "$@";; verify) shift; cmd_verify "$@";; *) reject_unknown_flag "$1";; esac
}
main "$@"
