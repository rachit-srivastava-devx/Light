"""F05 T7 -- end to end, over the relay, from a raw client (lane contract §5 T7).

A real `uvicorn` subprocess (never `TestClient` -- it would not catch an import-time failure or a
broken `FLEET_BIN`, same reasoning `test_f04_build_mode_wire.py`'s A1 gives), driven by `httpx`,
against a scripted STUB gateway. `tests/manual/stub_gateway.py` is NOT reused here: it always
replies with one fixed non-JSON string, but this suite needs to script DIFFERENT replies for the
conversational leg vs. the decompose leg, and to change what decompose returns turn over turn --
so this file writes its own small, disposable, STATEFUL stub (a `ThreadingHTTPServer` in a
background thread of this SAME test process; only the RELAY needs to be a separate OS process,
since it is the one with import-time module-level singletons under test), matching the lane
contract §6's own instruction ("write a disposable scripted-reply stub, as F03's builder and
verifier both did").

Written by this lane's builder (no test file existed for F05 before this build). Frozen once green.
"""

from __future__ import annotations

import json
import os
import socket
import stat
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import httpx
import pytest

_RELAY_PY_ROOT = Path(__file__).resolve().parents[1]  # orb/backend/relay-py
_WORKTREE_ROOT = _RELAY_PY_ROOT.parents[2]  # .../Light/.worktrees/F05-freeze-protocol
_FIXTURES_DIR = _WORKTREE_ROOT / "fleet" / "contracts" / "fixtures" / "lld"
_REAL_FLEET_BIN = _WORKTREE_ROOT / "fleet" / "keel" / "target" / "debug" / "fleet"

_COMPLETE_MODULE = json.loads((_FIXTURES_DIR / "complete_module.json").read_text(encoding="utf-8"))
_DECOMPOSER_SYSTEM_PROMPT_PREFIX = "You turn one build goal into a single ModuleBrief"
_CONVERSATIONAL_REPLY_TEXT = "Understood. Let's keep going."

pytestmark = pytest.mark.skipif(
    not _REAL_FLEET_BIN.exists(), reason="fleet binary not built (cargo build --manifest-path fleet/keel/Cargo.toml)"
)


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _wait_for_ready(url: str, timeout_s: float) -> None:
    deadline = time.monotonic() + timeout_s
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            response = httpx.get(url, timeout=1.0)
            if response.status_code == 200:
                return
        except httpx.HTTPError as exc:
            last_error = exc
        time.sleep(0.1)
    raise RuntimeError(f"{url} never became ready: {last_error}")


class _ScriptState:
    """Shared, mutable behavior for the stub gateway below. Tests run sequentially in one pytest
    process (no xdist in this suite), so plain attributes + a lock are enough -- no IPC needed.
    """

    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.decompose_reply: str | dict = "brief"  # "brief" | "clarify" | an explicit dict payload
        self.calls: list[dict] = []

    def set_brief(self, payload: dict | None = None) -> None:
        with self.lock:
            self.decompose_reply = payload if payload is not None else _COMPLETE_MODULE

    def set_clarify(self) -> None:
        with self.lock:
            self.decompose_reply = "clarify"


def _make_stub_handler(state: _ScriptState) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format_: str, *args: object) -> None:  # noqa: A002 - stdlib signature
            pass

        def do_GET(self) -> None:  # noqa: N802 - stdlib method name
            if self.path == "/healthz":
                self._reply(200, {"status": "ok"})
                return
            self._reply(404, {"error": "NOT_FOUND"})

        def do_POST(self) -> None:  # noqa: N802 - stdlib method name
            if self.path != "/v1/complete":
                self._reply(404, {"error": "NOT_FOUND"})
                return
            length = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(length) if length else b"{}"
            try:
                body = json.loads(raw.decode("utf-8"))
            except json.JSONDecodeError:
                self._reply(400, {"error": "BAD_REQUEST", "message": "invalid JSON"})
                return

            with state.lock:
                state.calls.append(body)
                system = body.get("system", "")
                if isinstance(system, str) and system.startswith(_DECOMPOSER_SYSTEM_PROMPT_PREFIX):
                    reply = state.decompose_reply
                    text = "not a JSON module brief, sorry" if reply == "clarify" else json.dumps(reply)
                else:
                    text = _CONVERSATIONAL_REPLY_TEXT

            self._reply(
                200,
                {
                    "content": [{"type": "text", "text": text}],
                    "usage": {"input_tokens": 5, "output_tokens": 5},
                    "model": "stub-model",
                    "adapter": "stub",
                    "is_fake_adapter": True,
                },
            )

        def _reply(self, status: int, payload: dict[str, object]) -> None:
            body = json.dumps(payload).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    return Handler


@pytest.fixture(scope="module")
def stub_gateway():
    state = _ScriptState()
    server = ThreadingHTTPServer(("127.0.0.1", 0), _make_stub_handler(state))
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield port, state
    finally:
        server.shutdown()
        thread.join(timeout=5.0)


def _start_relay(*, gateway_port: int, fleet_bin: str, tmp_path: Path, extra_env: dict[str, str] | None = None):
    relay_port = _free_port()
    context_db = tmp_path / f"context-{relay_port}.db"
    env = {
        **os.environ,
        "ORB_GATEWAY_URL": f"http://127.0.0.1:{gateway_port}",
        "ORB_CONTEXT_DB_PATH": str(context_db),
        "FLEET_BIN": fleet_bin,
        **(extra_env or {}),
    }
    proc = subprocess.Popen(
        [sys.executable, "-m", "uvicorn", "orb_relay.app:app", "--host", "127.0.0.1", "--port", str(relay_port)],
        cwd=str(_RELAY_PY_ROOT),
        env=env,
    )
    _wait_for_ready(f"http://127.0.0.1:{relay_port}/healthz", timeout_s=30.0)
    return proc, f"http://127.0.0.1:{relay_port}"


def _stop_relay(proc: subprocess.Popen) -> None:
    proc.terminate()
    try:
        proc.wait(timeout=5.0)
    except subprocess.TimeoutExpired:
        proc.kill()


@pytest.fixture(scope="module")
def real_relay(stub_gateway, tmp_path_factory: pytest.TempPathFactory):
    gateway_port, _state = stub_gateway
    tmp_path = tmp_path_factory.mktemp("f05-real-relay")
    proc, url = _start_relay(gateway_port=gateway_port, fleet_bin=str(_REAL_FLEET_BIN), tmp_path=tmp_path)
    try:
        yield url
    finally:
        _stop_relay(proc)


def _post(url: str, **json_body: object) -> httpx.Response:
    return httpx.post(f"{url}/v1/respond", json=json_body, timeout=15.0)


def _recursive_key_scan(value: object) -> set[str]:
    keys: set[str] = set()
    if isinstance(value, dict):
        for k, v in value.items():
            keys.add(k)
            keys |= _recursive_key_scan(v)
    elif isinstance(value, list):
        for item in value:
            keys |= _recursive_key_scan(item)
    return keys


# -------------------------------------------------------------------------------------------
# T7.1 / T7.2 / T7.3 -- the freeze branch
# -------------------------------------------------------------------------------------------


def test_t7_1_and_t7_2_and_t7_3_the_freeze_branch(stub_gateway, real_relay) -> None:
    """Script decompose to always return the control brief. Because `complete_module.json` itself
    has an EMPTY `data_owned` (verified directly against the fixture below), brief-evidence alone
    never covers that slot -- so this test's own scripted USER TEXT deliberately mentions "database
    storage" every turn, giving `data_owned` its coverage via the independent turn-evidence path
    (`derive_turn_evidence`), exactly as a real conversation would if the user actually answered
    that slot in words rather than in the decomposed brief.
    """
    assert _COMPLETE_MODULE["data_owned"] == []
    _, state = stub_gateway
    state.set_brief(_COMPLETE_MODULE)

    session_id = "s-t71-freeze"
    body = None
    turns = 0
    for turns in range(1, 7):  # <= 6 turns per T7.1
        response = _post(
            real_relay,
            tenant_id="t-t71",
            user_id="u-t71",
            session_id=session_id,
            text=f"turn {turns}: we need a database for storage of the module's state",
            mode="build",
        )
        assert response.status_code == 200
        body = response.json()
        if body["freeze"] is not None:
            break

    assert body is not None
    assert body["freeze"] is not None, f"did not freeze within 6 turns; last body: {body}"
    assert turns <= 6
    assert turns < 12  # SPLIT_TRIGGER_TURNS -- "bounded" measured, not claimed

    # T7.2 -- a FreezeProposal, not a Freeze.
    freeze = body["freeze"]
    assert freeze["proposed_by"] == "orb:dialogue"
    assert "stamped_by" not in _recursive_key_scan(body)

    # T7.3 -- content_hash is of the BRIEF, recomputed independently in this test.
    sys.path.insert(0, str(_RELAY_PY_ROOT / "src"))
    from orb_relay.proxy.lld_schemas import content_hash, validate_module_brief

    brief = validate_module_brief(freeze["brief"])
    assert brief is not None
    assert freeze["content_hash"] == content_hash(brief.model_dump(mode="json"))


# -------------------------------------------------------------------------------------------
# Beyond the contract's own T7.1-T7.8: depth is a REAL, consulted conjunct over the wire, not a
# name that happens to always agree with coverage. Added after T9's own M9 mutation (hardcode
# depth_pass=True in app.py) passed clean against every T7 case above -- none of them separates
# "coverage+ambiguity satisfied" from "keel actually said READY", because every scripted brief in
# T7.1-T7.3 genuinely passes keel. This case does not.
# -------------------------------------------------------------------------------------------


def test_t7_9_depth_pass_is_really_consulted_not_assumed(stub_gateway, real_relay) -> None:
    """A brief that is schema-VALID (so decompose accepts it and coverage/ambiguity converge
    exactly as in T7.1) but keel-INVALID (an owner absent from owners.v1.json, same construction as
    T5.2) must never freeze, no matter how many turns accumulate -- the M9 witness T7.1-T7.8 lack.
    """
    _, state = stub_gateway
    bad_owner_brief = {**_COMPLETE_MODULE, "owner": "nobody-at-all"}
    state.set_brief(bad_owner_brief)

    session_id = "s-t79-bad-owner"
    body = None
    for turn in range(1, 9):  # well past T7.1's <=6-turn freeze point, if depth were ignored
        response = _post(
            real_relay,
            tenant_id="t-t79",
            user_id="u-t79",
            session_id=session_id,
            text=f"turn {turn}: we need a database for storage of the module's state",
            mode="build",
        )
        assert response.status_code == 200
        body = response.json()
        assert body["freeze"] is None, f"turn {turn}: froze on a brief keel must reject (bad owner)"

    assert body is not None
    # By turn 8, coverage+ambiguity are satisfied (same arithmetic as T7.1) -- the ONLY remaining
    # blocker must be DEPTH, proving this is a real refusal, not a coincidence of never converging.
    assert any(entry.startswith("DEPTH") for entry in body["build"]["protocol"]["blocking"])


# -------------------------------------------------------------------------------------------
# T7.4 -- the never-answered branch
# -------------------------------------------------------------------------------------------


def test_t7_4_the_never_answered_branch_asks_then_splits_never_freezes(stub_gateway, real_relay) -> None:
    """FEATURES.md's second half, driven for real: 14 build turns where decompose never once
    produces a brief. Asserts every response is 200 and `freeze` never fires.

    The ask/split boundary itself lands at turn 11, not the pure-function unit test's turn 12
    (T4.4): `app.py`'s `clarify_turns` is derived from `_conversation_store`, which PHYSICALLY
    prunes past `MAX_CONVERSATION_TURNS` (24 messages / 12 turns) -- so past that point the true
    turn count is unrecoverable, not merely unavailable this query, and app.py's own comment at the
    derivation site documents the one-turn-earlier fallback this forces (report
    `SPLIT_TRIGGER_TURNS` itself once the cap is reached, which happens at turn 12 already). This
    is a wiring-level approximation only -- `next_move`'s own pure-function contract (T4.4) still
    pins the textbook turn-12 boundary directly, unaffected by this store's retention window.
    """
    _, state = stub_gateway
    state.set_clarify()

    session_id = "s-t74-never"
    moves: list[str] = []
    for turn in range(1, 15):  # drive 14 turns
        response = _post(
            real_relay,
            tenant_id="t-t74",
            user_id="u-t74",
            session_id=session_id,
            text=f"turn {turn} of a conversation that never settles on anything concrete",
            mode="build",
        )
        assert response.status_code == 200
        body = response.json()
        assert body["freeze"] is None
        moves.append(body["build"]["protocol"]["move"])

    assert len(moves) == 14
    assert moves[:11] == ["ask"] * 11
    assert moves[11:] == ["split"] * 3


# -------------------------------------------------------------------------------------------
# T7.5 -- the stall does not hang, over the wire
# -------------------------------------------------------------------------------------------


def test_t7_5_the_stall_escapes_over_the_wire(stub_gateway, real_relay) -> None:
    """Construct lane contract §3.1's stall shape live: `data_owned` gets exactly one evidence
    (turn 1's own scripted text) and nothing further ever touches it again (every scripted brief
    from here on has an EMPTY `data_owned`, and every later turn's text avoids that slot's
    keywords), while every OTHER required slot keeps accumulating from the (otherwise complete)
    scripted brief each turn.
    """
    _, state = stub_gateway
    brief_without_data_owned = {**_COMPLETE_MODULE, "data_owned": []}
    state.set_brief(brief_without_data_owned)

    session_id = "s-t75-stall"
    texts = [
        "turn 1: we will keep data in a database for storage",  # data_owned's only evidence
        "turn 2: here is the interface signature for the endpoint",
        "turn 3: the acceptance criteria given when then",
        "turn 4: note the dependency on the auth service",
        "turn 5: nothing new to add, just checking in",
    ]
    body = None
    for text in texts:
        response = _post(real_relay, tenant_id="t-t75", user_id="u-t75", session_id=session_id, text=text, mode="build")
        assert response.status_code == 200
        body = response.json()
        assert body["freeze"] is None  # data_owned never covered -> never eligible

    assert body is not None
    protocol = body["build"]["protocol"]
    assert protocol["escalated"] is True
    assert protocol["best_question_kind"] == "confirm"
    assert protocol["best_question_slot"] == "data_owned"


# -------------------------------------------------------------------------------------------
# T7.6 -- focus mode is untouched
# -------------------------------------------------------------------------------------------

# The complete field set `ConversationResponse` declares (transcribed from `app.py`'s class body --
# see that class for the authoritative list). F04's `build`/`decompose` and F05's `proposal`/
# `freeze` are ALL additive-default-None fields; asserting the exact key set (rather than only
# "the new keys are null") also catches an accidental field REMOVAL or RENAME, not just an addition.
_EXPECTED_RESPONSE_KEYS = {
    "tenant_id", "session_id", "text", "mode", "beats", "degraded", "degrade_reason",
    "token_ceiling", "source", "spent_paise", "latency_ms", "wait", "build", "decompose",
    "proposal", "freeze",
}


def test_t7_6_focus_mode_is_untouched(real_relay) -> None:
    response = _post(real_relay, tenant_id="t-t76", user_id="u-t76", session_id="s-t76", text="hello there", mode="focus")
    assert response.status_code == 200
    body = response.json()
    assert body["build"] is None
    assert body["proposal"] is None
    assert body["freeze"] is None
    assert set(body.keys()) == _EXPECTED_RESPONSE_KEYS


# -------------------------------------------------------------------------------------------
# T7.7 -- the gate is not on the spoken path
# -------------------------------------------------------------------------------------------


def _write_executable(path: Path, script: str) -> None:
    path.write_text(script, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)


def test_t7_7_a_slow_gate_still_returns_200_with_the_same_spoken_text(stub_gateway, tmp_path: Path) -> None:
    gateway_port, state = stub_gateway
    state.set_brief(_COMPLETE_MODULE)

    slow_script = tmp_path / "slow-fleet.sh"
    _write_executable(slow_script, "#!/bin/sh\nsleep 3\necho 'gate lld-ready: READY (slow) -- 14/14 checks passed'\nexit 0\n")

    real_proc, real_url = _start_relay(gateway_port=gateway_port, fleet_bin=str(_REAL_FLEET_BIN), tmp_path=tmp_path)
    slow_proc, slow_url = _start_relay(gateway_port=gateway_port, fleet_bin=str(slow_script), tmp_path=tmp_path)
    try:
        real_response = _post(
            real_url, tenant_id="t-t77", user_id="u-t77", session_id="s-t77-real", text="hello, first turn", mode="build"
        )
        slow_response = _post(
            slow_url, tenant_id="t-t77", user_id="u-t77", session_id="s-t77-slow", text="hello, first turn", mode="build"
        )
        assert real_response.status_code == 200
        assert slow_response.status_code == 200
        assert slow_response.json()["text"] == real_response.json()["text"]
    finally:
        _stop_relay(real_proc)
        _stop_relay(slow_proc)


# -------------------------------------------------------------------------------------------
# T7.8 -- no brief, no subprocess
# -------------------------------------------------------------------------------------------


def test_t7_8_no_brief_means_no_gate_subprocess(stub_gateway, tmp_path: Path) -> None:
    gateway_port, state = stub_gateway
    state.set_clarify()  # decompose never produces a brief on any of these turns

    invocation_log = tmp_path / "invocations.log"
    invocation_log.write_text("", encoding="utf-8")
    recorder_script = tmp_path / "recorder-fleet.sh"
    _write_executable(
        recorder_script,
        f'#!/bin/sh\necho "invoked $*" >> "{invocation_log}"\necho "gate lld-ready: READY (x) -- 14/14 checks passed"\nexit 0\n',
    )

    proc, url = _start_relay(gateway_port=gateway_port, fleet_bin=str(recorder_script), tmp_path=tmp_path)
    try:
        for turn in range(5):
            response = _post(
                url, tenant_id="t-t78", user_id="u-t78", session_id="s-t78", text=f"turn {turn}: still deciding", mode="build"
            )
            assert response.status_code == 200
            assert response.json()["freeze"] is None
    finally:
        _stop_relay(proc)

    assert invocation_log.read_text(encoding="utf-8") == ""
