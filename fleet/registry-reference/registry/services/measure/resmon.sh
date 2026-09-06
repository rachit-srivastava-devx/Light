#!/usr/bin/env bash
# resmon.sh — process monitoring adapter.
#
# psutil owns cross-platform process semantics and the persistent sample loop. The aggregate TSV
# remains six columns so existing report consumers keep working; resmon-process.tsv carries the
# per-process CPU, RSS, recursive children, and open-file measurements. A dedicated supervisor is
# intentionally not wired for this single laptop: launchd would restart a job, but would not cap a
# fan-out storm or clean unrelated orphaned ci.sh children. Bounded dispatch/process-group cleanup
# is the prevention boundary; this monitor supplies evidence for it.
set -uo pipefail

D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
STATE="${FLEET_RESMON_DIR:-$D/var/resmon}"
OUT="$STATE/resmon.tsv"
DETAIL="$STATE/resmon-process.tsv"
PIDFILE="$STATE/resmon.pid"
PY="${FLEET_PYTHON:-$D/var/venv/bin/python}"
INTERVAL=10

cmd="${1-report}"; shift || true
while [ $# -gt 0 ]; do
  case "$1" in
    --interval) [ -n "${2-}" ] || { echo "resmon: --interval needs a value" >&2; exit 2; }
                INTERVAL="$2"; shift 2 ;;
    *) echo "resmon: unknown flag '$1'" >&2; exit 2 ;;
  esac
done
mkdir -p "$STATE"

case "$cmd" in
  start)
    [ -x "$PY" ] || { echo "resmon: psutil Python missing ($PY)" >&2; exit 3; }
    if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE" 2>/dev/null)" 2>/dev/null; then
      echo "resmon: already running (pid $(cat "$PIDFILE"))"; exit 0
    fi
    "$PY" "$D/registry/features/monitoring/resmon.py" --out "$OUT" --detail "$DETAIL" --interval "$INTERVAL" \
      >/dev/null 2>&1 &
    sampler_pid=$!
    echo "$sampler_pid" >"$PIDFILE"
    sleep 0.1
    if ! kill -0 "$sampler_pid" 2>/dev/null; then
      rm -f "$PIDFILE"
      echo "resmon: psutil sampler exited during startup; inspect permissions" >&2
      exit 3
    fi
    echo "resmon: psutil sampling every ${INTERVAL}s -> $OUT (pid $sampler_pid)"
    ;;
  stop)
    if [ -f "$PIDFILE" ]; then
      sampler_pid="$(cat "$PIDFILE")"
      kill "$sampler_pid" 2>/dev/null || true
      rm -f "$PIDFILE"
      echo "resmon: stopped"
    else echo "resmon: not running"; fi
    ;;
  report)
    [ -s "$OUT" ] || { echo "resmon: no samples yet (run: registry/services/measure/resmon.sh start)"; exit 3; }
    awk -F'\t' '
      /^#/ { next }
      {
        n[$2]++; cpu[$2]+=$4; rss[$2]+=$5
        if ($4+0 > pcpu[$2]) pcpu[$2] = $4+0
        if ($5+0 > prss[$2]) prss[$2] = $5+0
        if ($3+0 > pn[$2])   pn[$2]   = $3+0
        if ($6+0 > pload)    pload    = $6+0
        loadsum += $6; loadn++
      }
      END {
        printf "  %-9s %7s %7s %8s %8s %7s\n", "class", "peak_np", "mean_cpu", "peak_cpu", "mean_mib", "peak_mib"
        printf "  %-9s %7s %7s %8s %8s %7s\n", "---------", "-------", "-------", "--------", "--------", "-------"
        for (k in n) printf "  %-9s %7d %7.1f %8.1f %8d %7d\n", k, pn[k], cpu[k]/n[k], pcpu[k], int(rss[k]/n[k]), prss[k]
        printf "\n  samples: %d   load avg mean %.2f, peak %.2f\n", loadn, (loadn?loadsum/loadn:0), pload
      }' "$OUT" | sort -k1,1
    echo
    echo "  window: $(awk -F'\t' '!/^#/{print $1}' "$OUT" | head -1) .. $(awk -F'\t' '!/^#/{print $1}' "$OUT" | tail -1)"
    echo "  detail: $DETAIL (per-process CPU/RSS/children/open_files)"
    echo "  NOTE: cpu_pct is summed across processes in a class, so it can exceed 100 on a multi-core box."
    ;;
  *) echo "resmon: unknown command '$cmd' (start|stop|report)" >&2; exit 2 ;;
esac
