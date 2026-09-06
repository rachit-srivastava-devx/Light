#!/usr/bin/env bash
# Compare BM25-only and hybrid retrieval on a read-only copy of the real memory corpus.
set -u

D="$(cd "$(dirname "$0")/../../.." && pwd)"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SOURCE_DB="$D/state/memory/agentrecall/fleet-memory.sqlite3"
PY="${FLEET_PYTHON:-$D/var/venv/bin/python}"
[ -x "$PY" ] || PY=python3
STORE="$SCRIPT_DIR/memory_store.py"
PAIRS="$SCRIPT_DIR/probe-pairs.json"
TMP_ROOT=""

cleanup() { [ -n "$TMP_ROOT" ] && rm -rf "$TMP_ROOT" 2>/dev/null || true; }
trap cleanup EXIT HUP INT TERM

[ -f "$SOURCE_DB" ] || { printf 'probe-bench: missing production database: %s\n' "$SOURCE_DB" >&2; exit 1; }
[ -f "$PAIRS" ] || { printf 'probe-bench: missing pair corpus: %s\n' "$PAIRS" >&2; exit 1; }

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/fleet-memory-bench.XXXXXX")"
ec=$?
[ "$ec" -eq 0 ] || { printf 'probe-bench: mktemp failed\n' >&2; exit "$ec"; }
COPY_DB="$TMP_ROOT/fleet-memory.sqlite3"

/usr/bin/sqlite3 "$SOURCE_DB" ".backup '$COPY_DB'"
ec=$?
[ "$ec" -eq 0 ] || { printf 'probe-bench: database copy failed\n' >&2; exit "$ec"; }

# The benchmark does not append receipts; this keeps any future receipt write in the temp tree.
# Legacy rows are read from the real corpus and migrated into the database COPY only.
export AGENT_RECALL_ROOT="$D/state/memory/agentrecall"
export FLEET_MEMORY_DB="$COPY_DB"
export FLEET_LEDGER="$TMP_ROOT/RECEIPTS.jsonl"

out="$("$PY" "$STORE" benchmark --pairs "$PAIRS" --limit 10)"
ec=$?
[ "$ec" -eq 0 ] || { printf '%s\n' "$out" >&2; exit "$ec"; }

jq -e '.corpus.memories == .corpus.embedded and .corpus.missing_embeddings == 0 and .distinct_arms == true' <<< "$out" >/dev/null
ec=$?
[ "$ec" -eq 0 ] || { printf 'probe-bench: embedding or arm-distinctness invariant failed\n%s\n' "$out" >&2; exit 6; }

printf 'database_copy=%s\n' "$COPY_DB"
printf 'model=%s dimensions=%s fusion=%s\n' "$(jq -r .model <<< "$out")" "$(jq -r .dimensions <<< "$out")" "$(jq -r .fusion <<< "$out")"
printf 'corpus memories=%s embedded=%s missing=%s\n' "$(jq -r .corpus.memories <<< "$out")" "$(jq -r .corpus.embedded <<< "$out")" "$(jq -r .corpus.missing_embeddings <<< "$out")"
printf 'distinct_arms=%s\n' "$(jq -r .distinct_arms <<< "$out")"
printf 'identical_probe_pairs (both arms receive the same positive probe; negative is the unrelated control):\n'
jq -r '.pairs[] | "\(.id)\tpositive=\(.probe)\tnegative=\(.unrelated)"' <<< "$out"
printf '%-6s %-7s %-8s %-8s %-8s %-8s %-8s %-8s\n' PAIR MEMORY BM25 HYBRID BM25_FP HYBRID_FP BM25_RANK HYBRID_RANK
jq -r '.pairs[] | ["pair", .id, (if .bm25_hit then "HIT" else "MISS" end), (if .hybrid_hit then "HIT" else "MISS" end), (if .bm25_false_positive then "YES" else "NO" end), (if .hybrid_false_positive then "YES" else "NO" end), (.bm25_rank // "-"), (.hybrid_rank // "-")] | @tsv' <<< "$out" | while IFS=$'\t' read -r pair ident bm25 hybrid bm25_fp hybrid_fp bm25_rank hybrid_rank; do
  printf '%-6s %-7s %-8s %-8s %-8s %-8s %-8s %-8s\n' "$pair" "$ident" "$bm25" "$hybrid" "$bm25_fp" "$hybrid_fp" "$bm25_rank" "$hybrid_rank"
done
printf 'BM25     recall=%s/%s (%.1f%%) precision=%.1f%% false_positives=%s\n' \
  "$(jq -r .metrics.bm25.recall_hits <<< "$out")" "$(jq -r .metrics.bm25.recall_total <<< "$out")" "$(jq -r '(.metrics.bm25.recall * 100)' <<< "$out")" "$(jq -r '(.metrics.bm25.precision * 100)' <<< "$out")" "$(jq -r .metrics.bm25.false_positives <<< "$out")"
printf 'HYBRID   recall=%s/%s (%.1f%%) precision=%.1f%% false_positives=%s\n' \
  "$(jq -r .metrics.hybrid.recall_hits <<< "$out")" "$(jq -r .metrics.hybrid.recall_total <<< "$out")" "$(jq -r '(.metrics.hybrid.recall * 100)' <<< "$out")" "$(jq -r '(.metrics.hybrid.precision * 100)' <<< "$out")" "$(jq -r .metrics.hybrid.false_positives <<< "$out")"
printf 'timing_ms cold_query=%s warm_query_median=%s\n' "$(jq -r .timings_ms.cold_query <<< "$out")" "$(jq -r .timings_ms.warm_query_median <<< "$out")"
