#!/usr/bin/env bash
# H2-3: chaos/fault injection on the relay-py -> gateway-sidecar network hop, using toxiproxy
# (Shopify, MIT, v2.12.0 via `brew install toxiproxy`). Track H2 (adversarial).
#
# ZERO provider cost / ZERO egress: gateway-sidecar is started with ORB_LLM_GATEWAY_ADAPTER=memory
# (the T0 fake adapter), so nothing here ever calls a real Anthropic/Gemini endpoint.
#
# What this proves, per the H2 brief's #1 invariant ("0ms unasked-for silence", "no silent
# fallback"): for each induced fault below, does /v1/respond (a) hang forever (dead air with no
# user-visible state), (b) silently return a 200 with wrong/degraded content, or (c) fail loudly
# with a typed, bounded-time error? (c) is the only acceptable outcome.
#
# Run: bash adversarial/chaos/toxiproxy_gateway_chaos.sh
# Requires: toxiproxy-server + toxiproxy-cli on PATH (`brew install toxiproxy`), relay-py's own
# .venv (already provisioned by the product's own `scripts/setup.sh`), node_modules for
# backend/gateway-sidecar (Track A's install).

set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

GATEWAY_PORT=18082
PROXY_PORT=18182
RELAY_PORT=18765
TOXIPROXY_API_PORT=18474
export TOXIPROXY_URL="http://127.0.0.1:${TOXIPROXY_API_PORT}"
CONTEXT_DB="$(mktemp -d)/context.db"

PIDS=()
cleanup() {
  echo "[chaos] cleaning up..."
  for pid in "${PIDS[@]:-}"; do
    kill "$pid" >/dev/null 2>&1 || true
  done
  rm -f "$CONTEXT_DB"
}
trap cleanup EXIT

echo "[chaos] starting gateway-sidecar on :$GATEWAY_PORT (ORB_LLM_GATEWAY_ADAPTER=memory -- \$0 cost, no egress)"
GATEWAY_SIDECAR_PORT="$GATEWAY_PORT" ORB_LLM_GATEWAY_ADAPTER=memory NODE_ENV=development \
  node_modules/.bin/vite-node backend/gateway-sidecar/src/index.ts > /tmp/h2-chaos-gateway.log 2>&1 &
PIDS+=("$!")

echo "[chaos] starting toxiproxy-server on :$TOXIPROXY_API_PORT"
toxiproxy-server -host 127.0.0.1 -port "$TOXIPROXY_API_PORT" > /tmp/h2-chaos-toxiproxy.log 2>&1 &
PIDS+=("$!")

sleep 2
curl -sf -m 3 "http://127.0.0.1:${GATEWAY_PORT}/healthz" >/dev/null || { echo "[chaos] FAIL: gateway-sidecar did not come up"; exit 1; }

toxiproxy-cli create --listen "127.0.0.1:${PROXY_PORT}" --upstream "127.0.0.1:${GATEWAY_PORT}" gateway_hop >/dev/null

echo "[chaos] starting relay-py on :$RELAY_PORT, pointed through the toxiproxy proxy"
( cd backend/relay-py && \
  ORB_RELAY_PORT="$RELAY_PORT" ORB_GATEWAY_URL="http://127.0.0.1:${PROXY_PORT}" ORB_CONTEXT_DB_PATH="$CONTEXT_DB" \
  .venv/bin/python -m uvicorn orb_relay.app:app --host 127.0.0.1 --port "$RELAY_PORT" > /tmp/h2-chaos-relay.log 2>&1 & )
PIDS+=("$(pgrep -f "uvicorn orb_relay.app:app --host 127.0.0.1 --port $RELAY_PORT" | tail -1)")
sleep 3
curl -sf -m 3 "http://127.0.0.1:${RELAY_PORT}/readyz" >/dev/null || { echo "[chaos] FAIL: relay-py did not come up ready"; cat /tmp/h2-chaos-relay.log; exit 1; }

respond() {
  local session="$1"
  curl -s -m 25 -o /tmp/h2-chaos-body.json -w "%{http_code} %{time_total}" -X POST \
    "http://127.0.0.1:${RELAY_PORT}/v1/respond" -H 'Content-Type: application/json' \
    -d "{\"session_id\":\"$session\",\"tenant_id\":\"chaos\",\"user_id\":\"chaos\",\"text\":\"chaos probe\"}"
}

PASS=0
FAIL=0
check() {
  local name="$1" status="$2" time_s="$3" max_time="$4" expect_status="$5"
  local ok=1
  if [ "$status" != "$expect_status" ]; then ok=0; fi
  awk -v t="$time_s" -v m="$max_time" 'BEGIN{exit !(t<=m)}' || ok=0
  if [ "$ok" = "1" ]; then
    PASS=$((PASS+1))
    echo "  PASS  $name (status=$status, time=${time_s}s, bound<=${max_time}s)"
  else
    FAIL=$((FAIL+1))
    echo "  FAIL  $name (status=$status vs expected $expect_status, time=${time_s}s vs bound<=${max_time}s)"
    echo "        body: $(cat /tmp/h2-chaos-body.json)"
  fi
}

echo
echo "=== baseline: no toxic ==="
read -r status time_s <<<"$(respond baseline)"
check "baseline responds normally" "$status" "$time_s" 5 200

echo
echo "=== toxic: 3s downstream latency (simulated 4G spike) ==="
toxiproxy-cli toxic add --toxicName=lat --type=latency --attribute=latency=3000 --attribute=jitter=200 --downstream gateway_hop >/dev/null
read -r status time_s <<<"$(respond lat-test)"
check "3s latency still completes (degrades speed, not correctness)" "$status" "$time_s" 10 200
toxiproxy-cli toxic remove --toxicName=lat gateway_hop >/dev/null

echo
echo "=== toxic: full hang, upstream never responds (toxiproxy 'timeout', timeout=0) ==="
toxiproxy-cli toxic add --toxicName=hang --type=timeout --attribute=timeout=0 --downstream gateway_hop >/dev/null
read -r status time_s <<<"$(respond hang-test)"
# The failure MUST be bounded and typed -- an infinite hang (curl's own -m 25 firing, status "000")
# is the worst-case finding this chaos probe exists to catch.
check "hung upstream fails LOUDLY and BOUNDED, never an infinite hang" "$status" "$time_s" 20 503
toxiproxy-cli toxic remove --toxicName=hang gateway_hop >/dev/null

echo
echo "=== toxic: mid-stream connection reset (RST) ==="
toxiproxy-cli toxic add --toxicName=rst --type=reset_peer --attribute=timeout=200 --downstream gateway_hop >/dev/null
read -r status time_s <<<"$(respond rst-test)"
check "connection reset surfaces as a fast, typed error" "$status" "$time_s" 5 503
toxiproxy-cli toxic remove --toxicName=rst gateway_hop >/dev/null

echo
echo "=== toxic: severe bandwidth starvation (1 KB/s) ==="
toxiproxy-cli toxic add --toxicName=bw --type=bandwidth --attribute=rate=1 --downstream gateway_hop >/dev/null
read -r status time_s <<<"$(respond bw-test)"
check "bandwidth-starved response still completes for a small payload" "$status" "$time_s" 15 200
toxiproxy-cli toxic remove --toxicName=bw gateway_hop >/dev/null

echo
echo "================================================================"
echo "H2-3 CHAOS RESULT: $PASS passed, $FAIL failed (of $((PASS+FAIL)) probes)"
[ "$FAIL" = "0" ]
