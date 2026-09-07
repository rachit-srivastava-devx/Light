"""F09 T14-T19 -- the orb-side half of the freeze -> SOW handoff, driven for real over HTTP
(lane contract docs/lane-contracts/F09-orb-fleet-handoff.md §7.2).

A real `uvicorn` subprocess (never `TestClient` -- same reasoning `test_f05_convergence.py` gives:
it would not catch an import-time failure or a broken FLEET_BIN), driven by `httpx`, against a
disposable scripted STUB gateway. This file duplicates `test_f05_convergence.py`'s harness rather
than importing it, matching this estate's own convention (that file's docstring: "this file writes
its own small, disposable, stateful stub"; `fleet/keel/fleet/tests/f07_sow_intake.rs`'s docstring:
"no shared tests/common module").

Written by this lane's builder from the lane contract's own §7.2 table (no test file existed for
F09 before this build). Frozen once green -- do not edit further without re-reading the contract.

Requires the real fleet binary built (`cargo build --manifest-path fleet/keel/Cargo.toml`) --
every test here skips (never fails, never silently counts as a pass) if it is genuinely absent.
"""

from __future__ import annotations

import ast
import json
import os
import shutil
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
_WORKTREE_ROOT = _RELAY_PY_ROOT.parents[2]  # .../Light/.worktrees/F09-handoff
_FIXTURES_DIR = _WORKTREE_ROOT / "fleet" / "contracts" / "fixtures" / "lld"
_REAL_FLEET_BIN = _WORKTREE_ROOT / "fleet" / "keel" / "target" / "debug" / "fleet"

_COMPLETE_MODULE = json.loads((_FIXTURES_DIR / "complete_module.json").read_text(encoding="utf-8"))
_DECOMPOSER_SYSTEM_PROMPT_PREFIX = "You turn one build goal into a single ModuleBrief"
_CONVERSATIONAL_REPLY_TEXT = "Understood. Let's keep going."
# Same scripted turn text T7.1 already proved converges to Move.Freeze within <=6 turns for this
# exact fixture (data_owned's only evidence comes from the turn text, not the decomposed brief).
_DATABASE_TURN_TEXT = "turn {n}: we need a database for storage of the module's state"

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
    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.decompose_reply: str | dict = "brief"
        self.calls: list[dict] = []

    def set_brief(self, payload: dict | None = None) -> None:
        with self.lock:
            self.decompose_reply = payload if payload is not None else _COMPLETE_MODULE


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
                    text = json.dumps(state.decompose_reply)
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


def _start_relay(
    *,
    gateway_port: int,
    fleet_bin: str | None,
    tmp_path: Path,
    fleet_state: Path,
    extra_env: dict[str, str] | None = None,
):
    relay_port = _free_port()
    context_db = tmp_path / f"context-{relay_port}.db"
    env = {**os.environ, "ORB_GATEWAY_URL": f"http://127.0.0.1:{gateway_port}", "ORB_CONTEXT_DB_PATH": str(context_db)}
    if fleet_bin is not None:
        env["FLEET_BIN"] = fleet_bin
    else:
        env.pop("FLEET_BIN", None)
    env["FLEET_STATE"] = str(fleet_state)
    env.update(extra_env or {})
    proc = subprocess.Popen(
        [sys.executable, "-m", "uvicorn", "orb_relay.app:app", "--host", "127.0.0.1", "--port", str(relay_port)],
        cwd=str(_RELAY_PY_ROOT),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    _wait_for_ready(f"http://127.0.0.1:{relay_port}/healthz", timeout_s=30.0)
    return proc, f"http://127.0.0.1:{relay_port}"


def _stop_relay(proc: subprocess.Popen) -> str:
    proc.terminate()
    try:
        proc.wait(timeout=5.0)
    except subprocess.TimeoutExpired:
        proc.kill()
    return proc.stdout.read() if proc.stdout else ""


def _post(url: str, **json_body: object) -> httpx.Response:
    return httpx.post(f"{url}/v1/respond", json=json_body, timeout=20.0)


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


def _write_executable(path: Path, script: str) -> None:
    path.write_text(script, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)


def _drive_to_freeze(url: str, *, session_id: str, max_turns: int = 6) -> dict:
    """T7.1's own proven convergence recipe, reused: scripted turn text that gives `data_owned`
    coverage via `derive_turn_evidence` (the fixture's own `data_owned` is empty). Returns the
    first response body whose `freeze` is non-null; fails the test if none converges in time.
    """
    body: dict | None = None
    for turn in range(1, max_turns + 1):
        response = _post(
            url,
            tenant_id="t-f09",
            user_id="u-f09",
            session_id=session_id,
            text=_DATABASE_TURN_TEXT.format(n=turn),
            mode="build",
        )
        assert response.status_code == 200, response.text
        body = response.json()
        if body["freeze"] is not None:
            return body
    assert body is not None
    raise AssertionError(f"did not freeze within {max_turns} turns; last body: {body}")


def _empty_or_absent(dir_path: Path) -> bool:
    return not dir_path.exists() or not any(dir_path.iterdir())


# -------------------------------------------------------------------------------------------
# F09-T14 -- a real freeze turn hands off
# -------------------------------------------------------------------------------------------


def test_f09_t14_a_real_freeze_turn_hands_off(stub_gateway, tmp_path: Path) -> None:
    gateway_port, state = stub_gateway
    state.set_brief(_COMPLETE_MODULE)
    fleet_state = tmp_path / "fleet-state-t14"

    proc, url = _start_relay(
        gateway_port=gateway_port, fleet_bin=str(_REAL_FLEET_BIN), tmp_path=tmp_path, fleet_state=fleet_state
    )
    try:
        body = _drive_to_freeze(url, session_id="s-t14-freeze")
    finally:
        log = _stop_relay(proc)
        assert "Traceback" not in log, f"relay log:\n{log}"

    assert body["freeze"] is not None
    handoff = body["handoff"]
    assert handoff is not None
    assert handoff["outcome"] == "sow_ready", handoff
    assert handoff["sow_id"] is not None
    assert len(handoff["sow_id"]) == 64
    assert all(c in "0123456789abcdef" for c in handoff["sow_id"])
    assert handoff["freeze_id"] is not None
    assert handoff["freeze_id"].startswith("fz-") and len(handoff["freeze_id"]) == 19
    assert handoff["freeze_version"] == 1

    # Independently observable, not just the wire claim: a real ledger entry and a real SOW record
    # exist on disk under the state dir the relay actually used.
    node_id = body["freeze"]["node_id"]
    ledger_file = fleet_state / "freezes" / node_id / "1.json"
    assert ledger_file.exists(), f"expected a real ledger entry at {ledger_file}"
    sow_file = fleet_state / "sows" / f"{handoff['sow_id']}.json"
    assert sow_file.exists(), f"expected a real SOW record at {sow_file}"


# -------------------------------------------------------------------------------------------
# F09-T15 -- UNAVAILABLE refuses, one layer up
# -------------------------------------------------------------------------------------------


def test_f09_t15a_fleet_bin_unset_never_freezes_handoff_null_zero_files(stub_gateway, tmp_path: Path) -> None:
    gateway_port, state = stub_gateway
    state.set_brief(_COMPLETE_MODULE)
    fleet_state = tmp_path / "fleet-state-t15a"

    proc, url = _start_relay(gateway_port=gateway_port, fleet_bin=None, tmp_path=tmp_path, fleet_state=fleet_state)
    try:
        body = None
        for turn in range(1, 9):  # well past T7.1's <=6-turn freeze point, if depth were ignored
            response = _post(
                url, tenant_id="t-t15a", user_id="u-t15a", session_id="s-t15a", text=_DATABASE_TURN_TEXT.format(n=turn), mode="build"
            )
            assert response.status_code == 200
            body = response.json()
            assert body["freeze"] is None, f"turn {turn}: must never freeze with FLEET_BIN unset"
            assert body["handoff"] is None
        assert body is not None
        assert any(entry.startswith("DEPTH") for entry in body["build"]["protocol"]["blocking"])
    finally:
        log = _stop_relay(proc)
        assert "Traceback" not in log, f"relay log:\n{log}"

    assert _empty_or_absent(fleet_state / "sows")
    assert _empty_or_absent(fleet_state / "freezes")


def test_f09_t15b_chmod_x_disposable_copy_mid_session_refuses_depth(stub_gateway, tmp_path: Path) -> None:
    gateway_port, state = stub_gateway
    state.set_brief(_COMPLETE_MODULE)
    fleet_state = tmp_path / "fleet-state-t15b"

    disposable = tmp_path / "disposable-fleet-t15b"
    shutil.copy2(_REAL_FLEET_BIN, disposable)
    disposable.chmod(0o444)  # readable, NOT executable -- never the real binary's own permissions

    proc, url = _start_relay(
        gateway_port=gateway_port, fleet_bin=str(disposable), tmp_path=tmp_path, fleet_state=fleet_state
    )
    try:
        body = None
        for turn in range(1, 9):
            response = _post(
                url, tenant_id="t-t15b", user_id="u-t15b", session_id="s-t15b", text=_DATABASE_TURN_TEXT.format(n=turn), mode="build"
            )
            assert response.status_code == 200, response.text
            body = response.json()
            assert body["freeze"] is None, f"turn {turn}: must never freeze against an unexecutable FLEET_BIN"
            assert body["handoff"] is None
        assert body is not None
        assert any(entry.startswith("DEPTH") for entry in body["build"]["protocol"]["blocking"]), body["build"]["protocol"]
    finally:
        log = _stop_relay(proc)
        assert "Traceback" not in log, f"relay log:\n{log}"

    assert _empty_or_absent(fleet_state / "sows")


# -------------------------------------------------------------------------------------------
# F09-T16 -- the orb never transmits or holds a stamp
# -------------------------------------------------------------------------------------------


def test_f09_t16_the_orb_never_transmits_or_holds_a_stamp(stub_gateway, tmp_path: Path) -> None:
    gateway_port, state = stub_gateway
    state.set_brief(_COMPLETE_MODULE)
    fleet_state = tmp_path / "fleet-state-t16"

    proc, url = _start_relay(
        gateway_port=gateway_port, fleet_bin=str(_REAL_FLEET_BIN), tmp_path=tmp_path, fleet_state=fleet_state
    )
    try:
        body = _drive_to_freeze(url, session_id="s-t16-freeze")
    finally:
        _stop_relay(proc)

    keys = _recursive_key_scan(body)
    assert "stamped_by" not in keys, f"the complete response must carry zero stamped_by keys: {keys}"

    handoff = body["handoff"]
    assert handoff is not None
    assert "freeze" not in handoff, "handoff must carry no embedded freeze object"
    assert "module_brief" not in handoff and "sow_seed" not in handoff, "handoff must carry no lld.v1 sub-document"
    assert set(handoff.keys()) == {"outcome", "sow_id", "freeze_id", "node_id", "freeze_version", "detail"}, handoff


# -------------------------------------------------------------------------------------------
# F09-T17 -- a NOT_READY brief never reaches intake
# -------------------------------------------------------------------------------------------


def test_f09_t17_a_not_ready_brief_never_reaches_intake(stub_gateway, tmp_path: Path) -> None:
    gateway_port, state = stub_gateway
    # Same construction as test_f05_convergence.py's T7.9: schema-valid (decompose/pydantic accept
    # it) but keel-INVALID (an owner absent from owners.v1.json).
    bad_owner_brief = {**_COMPLETE_MODULE, "owner": "nobody-at-all"}
    state.set_brief(bad_owner_brief)
    fleet_state = tmp_path / "fleet-state-t17"

    proc, url = _start_relay(
        gateway_port=gateway_port, fleet_bin=str(_REAL_FLEET_BIN), tmp_path=tmp_path, fleet_state=fleet_state
    )
    try:
        body = None
        for turn in range(1, 9):
            response = _post(
                url, tenant_id="t-t17", user_id="u-t17", session_id="s-t17", text=_DATABASE_TURN_TEXT.format(n=turn), mode="build"
            )
            assert response.status_code == 200
            body = response.json()
            assert body["freeze"] is None, f"turn {turn}: a keel-invalid brief must never freeze"
            assert body["handoff"] is None
        assert body is not None
        assert any(entry.startswith("DEPTH") for entry in body["build"]["protocol"]["blocking"])
    finally:
        log = _stop_relay(proc)
        assert "Traceback" not in log, f"relay log:\n{log}"

    assert _empty_or_absent(fleet_state / "sows")
    assert _empty_or_absent(fleet_state / "freezes")


# -------------------------------------------------------------------------------------------
# Beyond the contract's own T14-T19: M8 ("treat any non-zero stamp exit as success") names T15
# and T17 as the tests it must turn red. Measured directly (not assumed): applying that mutation
# and re-running T15a/T15b/T17 leaves all three green, because none of them ever reaches
# `FleetHandoff.hand_off` at all -- T15a never constructs a `FleetHandoff` (FLEET_BIN unset ->
# `UnavailableHandoff`), and T15b/T17 both refuse one layer up, at `readiness_gate.evaluate`
# (a PermissionError / a real NOT_READY exit), so `depth_pass` is already False and `Move.Freeze`
# is never reached. This is the same class of finding as M4's own disputed "F09-T3" listing (see
# the final report): the mutation battery names an unreachable test.
#
# A content-based attack (graft a nested `stamped_by` key, the way keel's own F09-T8 does) turns
# out to be UNREACHABLE from this orb's real pipeline for a different, load-bearing reason: EVERY
# nested shape in `lld_schemas.py` (`NumberDerivation`, `StructuralDerivation`, ...) is `_Strict`
# (`extra="forbid"`), so a decomposer output carrying an unrecognized nested key is rejected by
# PYTHON's own pydantic validation before `decompose_outcome.brief` is ever non-None -- stricter,
# here, than the hand-written Rust/TS validators F07 found the hole in. Measured, not assumed:
# see the git history of this file for the HTTP-driven construction that was tried first and
# failed for exactly this reason (`decompose: {kind: "clarify_request", ...}`, never reaching
# BUILD). So `freeze stamp`'s STAMP_KEY_PRESENT/FREEZE_UNHASHABLE guards are real defense-in-depth
# for a caller with a WEAKER type system than this one's -- never reachable through THIS orb's
# own typed brief. The one channel that remains real and reachable is a plain non-zero exit from
# the stamp subprocess itself (an environment fault, a producer self-check failure, or -- as
# here -- any refusal at all), independent of brief content. Exercised directly, below.
# -------------------------------------------------------------------------------------------


def test_f09_extra_not_stamped_when_the_stamp_subprocess_itself_refuses(tmp_path: Path) -> None:
    """Direct (non-HTTP) exercise of `FleetHandoff.hand_off`'s NOT_STAMPED branch -- the plain
    per-exit-code interpretation `handoff.py` owns. A wrapper script refuses only the `freeze`
    subcommand unconditionally and passes every other subcommand straight through to the real
    binary, isolating this from readiness/depth_pass gating entirely (unlike T15/T17, which are
    governed one layer up -- see the note above).
    """
    from orb_relay.build.handoff import FleetHandoff
    from orb_relay.proxy.lld_schemas import validate_module_brief

    raw = json.loads((_FIXTURES_DIR / "complete_module.json").read_text(encoding="utf-8"))
    brief = validate_module_brief(raw)
    assert brief is not None, "complete_module.json must itself validate as a ModuleBrief"

    refusing_stamp = tmp_path / "refuses-stamp.sh"
    _write_executable(
        refusing_stamp,
        f'#!/bin/sh\n'
        f'if [ "$1" = "freeze" ]; then echo "freeze_stamp_refused: reason=STAMP_KEY_PRESENT" >&2; exit 7; fi\n'
        f'exec "{_REAL_FLEET_BIN}" "$@"\n',
    )
    handoff = FleetHandoff(str(refusing_stamp))
    result = handoff.hand_off(brief)
    assert result.outcome.value == "not_stamped", result
    assert result.sow_id is None
    assert result.freeze_id is None
    assert result.node_id == brief.node_id


# -------------------------------------------------------------------------------------------
# F09-T18 -- the handoff never swallows the freeze
# -------------------------------------------------------------------------------------------


def test_f09_t18_a_stamp_timeout_never_swallows_the_freeze(stub_gateway, tmp_path: Path) -> None:
    gateway_port, state = stub_gateway
    state.set_brief(_COMPLETE_MODULE)
    fleet_state = tmp_path / "fleet-state-t18"

    # Unconditionally slow, then passes through to the REAL binary -- readiness.py's own hardcoded
    # 10s budget tolerates this sleep (so the gate still reports READY and depth_pass is True), but
    # the handoff's own configurable (and here, deliberately tiny) timeout does not.
    slow_script = tmp_path / "slow-fleet-t18.sh"
    _write_executable(slow_script, f'#!/bin/sh\nsleep 3\nexec "{_REAL_FLEET_BIN}" "$@"\n')

    proc, url = _start_relay(
        gateway_port=gateway_port,
        fleet_bin=str(slow_script),
        tmp_path=tmp_path,
        fleet_state=fleet_state,
        extra_env={"FLEET_HANDOFF_TIMEOUT_S": "0.5"},
    )
    try:
        body = _drive_to_freeze(url, session_id="s-t18-freeze")
    finally:
        log = _stop_relay(proc)
        assert "Traceback" not in log, f"relay log:\n{log}"

    assert body["freeze"] is not None, "the freeze proposal must never be swallowed by a slow handoff"
    handoff = body["handoff"]
    assert handoff is not None
    assert handoff["outcome"] == "timeout", handoff
    assert _empty_or_absent(fleet_state / "sows")


# -------------------------------------------------------------------------------------------
# F09-T19 -- AST proof the orb cannot stamp
# -------------------------------------------------------------------------------------------


def _constructs_any(tree: ast.AST, names: set[str]) -> list[str]:
    hits: list[str] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        name = func.id if isinstance(func, ast.Name) else func.attr if isinstance(func, ast.Attribute) else None
        if name in names:
            hits.append(name)
    return hits


def test_f09_t19_ast_proof_the_orb_cannot_stamp() -> None:
    import orb_relay.app as app_module
    from orb_relay.build import handoff as handoff_module

    forbidden_constructors = {"Freeze", "LldV1", "SowSeed"}
    stamp_literal = "keel" + ":lld-ready"  # built at test runtime so THIS file's own source never carries it either

    for module in (handoff_module, app_module):
        source = Path(module.__file__).read_text(encoding="utf-8")
        tree = ast.parse(source)
        hits = _constructs_any(tree, forbidden_constructors)
        assert hits == [], f"{module.__name__} must never construct {forbidden_constructors}, found: {hits}"
        assert stamp_literal not in source, f"{module.__name__} must never spell out the gate's stamp literal"
