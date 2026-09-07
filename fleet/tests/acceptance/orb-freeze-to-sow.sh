#!/usr/bin/env bash
# F09: the freeze -> SOW handoff, driven for real (lane contract docs/lane-contracts/
# F09-orb-fleet-handoff.md §7.3). This is the lane's headline claim and the only test that proves
# it: a real freeze turn in a live orb build-mode session produces a real fleet SOW record on
# disk that keel itself loads and mutates. A log line is not the property -- every observable
# below is a real, independently-loadable artifact. `keel/fleet/tests/f09_freeze_stamp.rs` and
# `orb/backend/relay-py/tests/test_f09_handoff.py` already cover each language's own unit/
# integration surface in-process; this script is the single, real, cross-process drive -- two
# genuine OS processes (a real `uvicorn` relay, a real disposable stub gateway), `curl` driving
# the turns, matching F05's own verified pattern (test_f05_convergence.py) and F07's own
# acceptance-script convention (sow-lld-intake.sh).
#
# Every background process is started directly in THIS shell (never inside a function invoked via
# command substitution) -- a function that both backgrounds a job and is itself called as `x="$(f)"`
# makes the substitution's pipe wait on descriptors the backgrounded grandchild can inherit, which
# hung this script's first draft silently (measured: the relay booted and answered `curl` by hand
# in 30ms, yet the script produced zero output for 5+ minutes). Recorded in the final report.
#
# Status captured directly, never `$?` after a pipe (verify.sh:20, E1/S11).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"                       # -> fleet/
WORKTREE_ROOT="$(cd "$ROOT/.." && pwd)"                           # -> Light/.worktrees/<lane>
FLEET_BIN="${FLEET_BIN:-${CARGO_TARGET_DIR:-$ROOT/keel/target}/debug/fleet}"
RELAY_PY_ROOT="$WORKTREE_ROOT/orb/backend/relay-py"
RELAY_VENV_PY="$RELAY_PY_ROOT/.venv/bin/python3"
FIXTURE="$ROOT/contracts/fixtures/lld/complete_module.json"

fail() { echo "FAIL  $1"; exit 6; }

if [ ! -x "$FLEET_BIN" ]; then
  echo "fleet binary not found at $FLEET_BIN -- build it first:"
  echo "  cargo build --manifest-path \"$ROOT/keel/Cargo.toml\""
  exit 3
fi
if [ ! -x "$RELAY_VENV_PY" ]; then
  echo "relay-py venv not found at $RELAY_VENV_PY -- set it up first (documented S0 step):"
  echo "  cd \"$WORKTREE_ROOT/orb\" && python3 -m venv backend/relay-py/.venv && backend/relay-py/.venv/bin/pip install --quiet -e \"backend/relay-py[dev]\""
  exit 3
fi
if [ ! -f "$FIXTURE" ]; then
  echo "control fixture not found at $FIXTURE"
  exit 3
fi

WORK="$(mktemp -d)"
STUB_SCRIPT="$WORK/stub_gateway.py"
CHECK_FROZEN_SCRIPT="$WORK/check_frozen.py"
PIDS=""
cleanup() {
  for pid in $PIDS; do kill "$pid" >/dev/null 2>&1; done
  wait >/dev/null 2>&1
  rm -rf "$WORK"
}
trap cleanup EXIT

# --- a disposable, scripted stub gateway (never tests/manual/stub_gateway.py -- same reason
# test_f05_convergence.py's own docstring gives: this needs a SCRIPTED reply, not one fixed
# string). Pure stdlib, no venv needed.
cat > "$STUB_SCRIPT" <<'PYEOF'
import http.server, json, socketserver, sys

with open(sys.argv[1], encoding="utf-8") as f:
    BRIEF = json.load(f)

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        self._reply(200, {"status": "ok"})

    def do_POST(self):
        if self.path != "/v1/complete":
            self._reply(404, {"error": "NOT_FOUND"})
            return
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b"{}"
        try:
            body = json.loads(raw.decode("utf-8"))
        except Exception:
            self._reply(400, {"error": "BAD_REQUEST"})
            return
        system = body.get("system", "")
        if isinstance(system, str) and system.startswith("You turn one build goal into a single ModuleBrief"):
            text = json.dumps(BRIEF)
        else:
            text = "Understood. Let's keep going."
        self._reply(200, {
            "content": [{"type": "text", "text": text}],
            "usage": {"input_tokens": 5, "output_tokens": 5},
            "model": "stub-model", "adapter": "stub", "is_fake_adapter": True,
        })

    def _reply(self, status, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

class Server(socketserver.TCPServer):
    allow_reuse_address = True

with open(sys.argv[2], "w", encoding="utf-8") as pf:
    server = Server(("127.0.0.1", 0), Handler)
    pf.write(str(server.server_address[1]))
server.serve_forever()
PYEOF

# Prints ONE 'yes'/'no' line (whether `freeze` is non-null) -- bash cannot parse JSON on its own;
# this extracts exactly one field and decides nothing itself.
cat > "$CHECK_FROZEN_SCRIPT" <<'PYEOF'
import json, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
print("yes" if d.get("freeze") is not None else "no")
PYEOF

field_of() { # field_of <json-file> <python-expr-on-d> -> prints the value, or "" for None
  python3 -c "
import json, sys
d = json.load(open(sys.argv[1], encoding='utf-8'))
v = $2
print(v if v is not None else '')
" "$1"
}

free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'; }

wait_for_http() { # wait_for_http <url> <timeout_s>
  local url="$1" timeout_s="$2" waited=0
  while [ "$waited" -lt "$timeout_s" ]; do
    curl -s -o /dev/null -m 2 "$url" && return 0
    sleep 0.2
    waited=$((waited + 1))  # coarse (0.2s ticks counted as 1) -- good enough, bounded either way
  done
  return 1
}

wait_for_file() { # wait_for_file <path> <timeout_s>
  local path="$1" timeout_s="$2" waited=0
  while [ "$waited" -lt "$((timeout_s * 5))" ]; do
    [ -s "$path" ] && return 0
    sleep 0.2
    waited=$((waited + 1))
  done
  return 1
}

# drive_to_freeze <relay_url> <session_id> <out_response_file> -> 0 if frozen within 6 turns
drive_to_freeze() {
  local url="$1" session="$2" outfile="$3" turn
  for turn in 1 2 3 4 5 6; do
    curl -s -m 20 -X POST "$url/v1/respond" \
      -H 'Content-Type: application/json' \
      -d "{\"tenant_id\":\"t-f09-drive\",\"user_id\":\"u-f09-drive\",\"session_id\":\"$session\",\"text\":\"turn $turn: we need a database for storage of the module's state\",\"mode\":\"build\"}" \
      -o "$outfile"
    [ -s "$outfile" ] || fail "empty response from $url/v1/respond on turn $turn"
    if [ "$(python3 "$CHECK_FROZEN_SCRIPT" "$outfile")" = "yes" ]; then
      return 0
    fi
  done
  return 1
}

count_sows() { find "$1/sows" -maxdepth 1 -name '*.json' 2>/dev/null | wc -l | tr -d ' '; }

# ============================================================================================
# The stub gateway -- ONE process for the whole script, started directly (no wrapping function).
# ============================================================================================
GATEWAY_PORT_FILE="$WORK/gateway.port"
python3 "$STUB_SCRIPT" "$FIXTURE" "$GATEWAY_PORT_FILE" >"$WORK/gateway.log" 2>&1 &
GATEWAY_PID=$!
PIDS="$PIDS $GATEWAY_PID"
wait_for_file "$GATEWAY_PORT_FILE" 10 || { cat "$WORK/gateway.log"; fail "stub gateway never reported its port"; }
GATEWAY_PORT="$(cat "$GATEWAY_PORT_FILE")"

# ============================================================================================
# (1)+(2)+(3)+(4) the positive drive: real binary, real gateway, real relay -- freeze, hand off,
# observe the SOW four independent ways (none of them a log line; `fleet sow list`/`show` do not
# exist in this tree, so these are the real observables).
# ============================================================================================
STATE_POS="$WORK/state-positive"
mkdir -p "$STATE_POS"
RELAY_POS_PORT="$(free_port)"
RELAY_POS_LOG="$WORK/relay-positive.log"
FLEET_STATE="$STATE_POS" FLEET_BIN="$FLEET_BIN" \
  ORB_GATEWAY_URL="http://127.0.0.1:$GATEWAY_PORT" \
  ORB_CONTEXT_DB_PATH="$WORK/context-positive.db" \
  "$RELAY_VENV_PY" -m uvicorn orb_relay.app:app --host 127.0.0.1 --port "$RELAY_POS_PORT" >"$RELAY_POS_LOG" 2>&1 &
RELAY_POS_PID=$!
PIDS="$PIDS $RELAY_POS_PID"
RELAY_URL_POS="http://127.0.0.1:$RELAY_POS_PORT"
wait_for_http "$RELAY_URL_POS/healthz" 30 || { cat "$RELAY_POS_LOG"; fail "positive relay on port $RELAY_POS_PORT never became ready"; }

RESP_POS="$WORK/resp-positive.json"
drive_to_freeze "$RELAY_URL_POS" "s-f09-positive" "$RESP_POS" \
  || { cat "$RESP_POS" 2>/dev/null; fail "positive drive: did not freeze within 6 turns"; }

node_id="$(field_of "$RESP_POS" "d['freeze']['node_id']")"
[ -n "$node_id" ] || fail "positive drive: response carries no freeze.node_id"
handoff_outcome="$(field_of "$RESP_POS" "d['handoff']['outcome'] if d.get('handoff') else None")"
[ "$handoff_outcome" = "sow_ready" ] || { cat "$RESP_POS"; fail "positive drive: handoff.outcome=$handoff_outcome, expected sow_ready"; }
sow_id="$(field_of "$RESP_POS" "d['handoff']['sow_id']")"
reported_freeze_id="$(field_of "$RESP_POS" "d['handoff']['freeze_id']")"
reported_version="$(field_of "$RESP_POS" "d['handoff']['freeze_version']")"
[ -n "$sow_id" ] || fail "positive drive: handoff carries no sow_id"

# (a) exactly one new SOW record, cross-checked against the real ledger entry.
[ "$(count_sows "$STATE_POS")" -eq 1 ] || fail "expected exactly 1 record under $STATE_POS/sows/, found $(count_sows "$STATE_POS")"
SOW_RECORD="$STATE_POS/sows/$sow_id.json"
[ -f "$SOW_RECORD" ] || fail "SOW record not found at $SOW_RECORD"
LEDGER_FREEZE="$STATE_POS/freezes/$node_id/1.json"
[ -f "$LEDGER_FREEZE" ] || fail "ledger freeze entry not found at $LEDGER_FREEZE"

prov_node_id="$(field_of "$SOW_RECORD" "d['lld_provenance']['node_id']")"
prov_freeze_id="$(field_of "$SOW_RECORD" "d['lld_provenance']['freeze_id']")"
prov_content_hash="$(field_of "$SOW_RECORD" "d['lld_provenance']['content_hash']")"
ledger_node_id="$(field_of "$LEDGER_FREEZE" "d['freeze']['node_id']")"
ledger_freeze_id="$(field_of "$LEDGER_FREEZE" "d['freeze']['freeze_id']")"
ledger_content_hash="$(field_of "$LEDGER_FREEZE" "d['freeze']['content_hash']")"

[ "$prov_node_id" = "$ledger_node_id" ] || fail "sow lld_provenance.node_id ($prov_node_id) != ledger freeze.node_id ($ledger_node_id)"
[ "$prov_freeze_id" = "$ledger_freeze_id" ] || fail "sow lld_provenance.freeze_id ($prov_freeze_id) != ledger freeze.freeze_id ($ledger_freeze_id)"
[ "$prov_content_hash" = "$ledger_content_hash" ] || fail "sow lld_provenance.content_hash != ledger freeze.content_hash"
[ "$prov_freeze_id" = "$reported_freeze_id" ] || fail "sow lld_provenance.freeze_id ($prov_freeze_id) != the freeze_id the response reported ($reported_freeze_id)"
[ "$reported_version" = "1" ] || fail "expected freeze_version=1 on a fresh state dir, got $reported_version"

# (b) keel's own reader loads AND MUTATES the record -- the strongest available "this is a real
# SOW, not a file the test wrote".
ACCEPT_OUT="$WORK/accept.out"
FLEET_STATE="$STATE_POS" "$FLEET_BIN" sow accept --id "$sow_id" >"$ACCEPT_OUT" 2>&1
accept_rc=$?
accept_text="$(cat "$ACCEPT_OUT")"
[ "$accept_rc" -eq 0 ] || { echo "$accept_text"; fail "fleet sow accept --id $sow_id exited $accept_rc, expected 0"; }
case "$accept_text" in
  *"SOW_ACCEPTED id=$sow_id"*) : ;;
  *) fail "accept output did not confirm id=$sow_id: $accept_text" ;;
esac

# (c) the accept receipt is appended to the ledger.
CHAIN="$STATE_POS/ledger/chain.jsonl"
[ -f "$CHAIN" ] || fail "no ledger chain at $CHAIN"
grep -q "SOW_ACCEPTED" "$CHAIN" || fail "no SOW_ACCEPTED receipt found in $CHAIN"

# (d) round-trip identity: the record's OWN persisted task text, fed back through `sow --task`,
# yields the SAME 64-hex id (sow::id_for_task is blake3(task), deterministic) -- proving the
# record's identity derives from the real compiled text, not from anything this script invented.
task_text="$(field_of "$SOW_RECORD" "d['task']")"
[ -n "$task_text" ] || fail "sow record carries no task text"
ROUNDTRIP_OUT="$WORK/roundtrip.out"
FLEET_STATE="$STATE_POS" "$FLEET_BIN" sow --task "$task_text" >/dev/null 2>"$ROUNDTRIP_OUT"
roundtrip_rc=$?
[ "$roundtrip_rc" -eq 9 ] || { cat "$ROUNDTRIP_OUT"; fail "round-trip sow --task exited $roundtrip_rc, expected 9"; }
roundtrip_id="$(grep -o 'SOW_READY_AWAITING_REVIEW id=[0-9a-f]\{64\}' "$ROUNDTRIP_OUT" | head -1 | cut -d= -f2)"
[ "$roundtrip_id" = "$sow_id" ] || fail "round-trip id ($roundtrip_id) != original id ($sow_id)"
[ "$(count_sows "$STATE_POS")" -eq 1 ] || fail "round-trip must not create a second record, found $(count_sows "$STATE_POS")"

echo "ok  positive drive: froze, handed off, sow_id=$sow_id accepted, round-trip identity confirmed"

kill "$RELAY_POS_PID" >/dev/null 2>&1
wait "$RELAY_POS_PID" 2>/dev/null

# ============================================================================================
# (5) negative control: an unexecutable disposable copy of the binary -> the session never
# freezes and zero sow records appear. Then restore the executable bit and confirm a FRESH
# session freezes and hands off normally (the gate is not sticky).
# ============================================================================================
DISPOSABLE_BIN="$WORK/disposable-fleet"
cp "$FLEET_BIN" "$DISPOSABLE_BIN"
chmod -x "$DISPOSABLE_BIN"

STATE_NEG="$WORK/state-negative"
mkdir -p "$STATE_NEG"
RELAY_NEG_PORT="$(free_port)"
RELAY_NEG_LOG="$WORK/relay-negative.log"
FLEET_STATE="$STATE_NEG" FLEET_BIN="$DISPOSABLE_BIN" \
  ORB_GATEWAY_URL="http://127.0.0.1:$GATEWAY_PORT" \
  ORB_CONTEXT_DB_PATH="$WORK/context-negative.db" \
  "$RELAY_VENV_PY" -m uvicorn orb_relay.app:app --host 127.0.0.1 --port "$RELAY_NEG_PORT" >"$RELAY_NEG_LOG" 2>&1 &
RELAY_NEG_PID=$!
PIDS="$PIDS $RELAY_NEG_PID"
RELAY_URL_NEG="http://127.0.0.1:$RELAY_NEG_PORT"
wait_for_http "$RELAY_URL_NEG/healthz" 30 || { cat "$RELAY_NEG_LOG"; fail "negative relay on port $RELAY_NEG_PORT never became ready"; }

RESP_NEG="$WORK/resp-negative.json"
if drive_to_freeze "$RELAY_URL_NEG" "s-f09-negative" "$RESP_NEG"; then
  cat "$RESP_NEG"
  fail "negative control: session froze against an unexecutable FLEET_BIN"
fi
[ "$(count_sows "$STATE_NEG")" -eq 0 ] || fail "negative control: expected 0 sow records, found $(count_sows "$STATE_NEG")"
[ ! -d "$STATE_NEG/freezes" ] || [ -z "$(find "$STATE_NEG/freezes" -mindepth 2 2>/dev/null)" ] || fail "negative control: expected 0 ledger freeze entries"
echo "ok  negative control: an unexecutable FLEET_BIN never freezes, zero sow records"

kill "$RELAY_NEG_PID" >/dev/null 2>&1
wait "$RELAY_NEG_PID" 2>/dev/null

chmod +x "$DISPOSABLE_BIN"
STATE_RECOVER="$WORK/state-recover"
mkdir -p "$STATE_RECOVER"
RELAY_RECOVER_PORT="$(free_port)"
RELAY_RECOVER_LOG="$WORK/relay-recover.log"
FLEET_STATE="$STATE_RECOVER" FLEET_BIN="$DISPOSABLE_BIN" \
  ORB_GATEWAY_URL="http://127.0.0.1:$GATEWAY_PORT" \
  ORB_CONTEXT_DB_PATH="$WORK/context-recover.db" \
  "$RELAY_VENV_PY" -m uvicorn orb_relay.app:app --host 127.0.0.1 --port "$RELAY_RECOVER_PORT" >"$RELAY_RECOVER_LOG" 2>&1 &
RELAY_RECOVER_PID=$!
PIDS="$PIDS $RELAY_RECOVER_PID"
RELAY_URL_RECOVER="http://127.0.0.1:$RELAY_RECOVER_PORT"
wait_for_http "$RELAY_URL_RECOVER/healthz" 30 || { cat "$RELAY_RECOVER_LOG"; fail "recovery relay on port $RELAY_RECOVER_PORT never became ready"; }

RESP_RECOVER="$WORK/resp-recover.json"
drive_to_freeze "$RELAY_URL_RECOVER" "s-f09-recover" "$RESP_RECOVER" \
  || { cat "$RESP_RECOVER" 2>/dev/null; fail "recovery: a freshly-executable binary still did not freeze -- the gate is sticky"; }
recover_outcome="$(field_of "$RESP_RECOVER" "d['handoff']['outcome'] if d.get('handoff') else None")"
[ "$recover_outcome" = "sow_ready" ] || fail "recovery: handoff.outcome=$recover_outcome, expected sow_ready"
[ "$(count_sows "$STATE_RECOVER")" -eq 1 ] || fail "recovery: expected exactly 1 sow record, found $(count_sows "$STATE_RECOVER")"
echo "ok  recovery: restoring the executable bit unsticks the gate -- a fresh session freezes and hands off normally"

echo "ok  orb-freeze-to-sow: freeze -> stamp -> sow -> accept -> round-trip, plus the negative control, all real"
exit 0
