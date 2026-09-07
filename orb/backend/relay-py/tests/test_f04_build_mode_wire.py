"""F04 A1 -- build mode over the wire, against a REAL relay-py process (lane contract §5 A1).
A6 -- mode/prompt exhaustiveness. A7 -- the safety-guard mode branch.

A1 does not use `TestClient`: a real `uvicorn` subprocess on an ephemeral port, with
`ORB_GATEWAY_URL` pointed at `tests/manual/stub_gateway.py`, driven over real HTTP by `httpx`.
`TestClient` runs the ASGI app in-process and would not catch an import-time failure, a
uvicorn-only path, or a broken `ORB_GATEWAY_URL` -- and `app.py`'s module-level singletons
(`_context_store`, `_conversation_store`, `_build_session_store`, `_MODE_PROMPT_NAMES`) all execute
at import, exactly the layer that was broken before this lane (`mode="build"` was a 500).

A6/A7 use the existing in-process `TestClient` + `ScriptedGateway` seam (`tests/test_app_routes.py`'s
pattern) -- they are about mode/prompt wiring and the guard, not about the real-process regression
pin A1 exists for.

The builder may not edit this file (lane contract, front matter). If a case looks wrong, escalate.

PROVENANCE NOTE (F03 verification pass, dated in FLEET-LEARNINGS.md): `test_a1_2`, `test_a7_1`,
and `test_a7_2` were edited here -- not by the F03 builder, who correctly escalated instead of
touching this file -- by F03's INDEPENDENT VERIFIER, after confirming the root cause was these
three cases' own finite/positional assumptions (exactly one gateway call per BUILD turn; "the last
recorded request is the conversational one") colliding with F03's own accepted contract §2.4
(decompose() fires on every BUILD turn, unconditionally, a second real gateway call this file
predates). Each fix locates/covers the added call rather than weakening what the case asserts --
see each case's own docstring/comment for the specific reasoning. No other case in this file was
touched.
"""

from __future__ import annotations

import json
import os
import socket
import subprocess
import sys
import time
from pathlib import Path

import httpx
import pytest
from fastapi.testclient import TestClient

import orb_relay.app as app_module
from orb_relay.app import app, get_gateway
from orb_relay.cost.meter import UsageDelta
from orb_relay.proxy.gateway_client import GatewayCompletion
from orb_relay.proxy.prompts import load_agent_prompt
from orb_relay.proxy.schemas import ResponseMode
from orb_relay.store.build_session_store import BuildSessionStore
from orb_relay.store.context_store import ContextStore
from tests.test_conversation_guard import _DISTRESS_HARM_REGRESSION_CASES

_RELAY_PY_ROOT = Path(__file__).resolve().parents[1]  # orb/backend/relay-py
_DOMAIN_AGENTS_DIR = _RELAY_PY_ROOT.parents[1] / "domain" / "agents"
_STUB_GATEWAY_SCRIPT = Path(__file__).resolve().parent / "manual" / "stub_gateway.py"

# F03 (accepted contract §2.4, merged after this file was frozen): a BUILD turn now fires a SECOND
# real gateway call -- decompose() -- on every turn, unconditionally, alongside the conversational
# call this file's A1/A7 cases were written against when exactly one call per turn was true. The
# fixture below is REUSED (lane contract §5's own reuse table), never hand-authored, so A7's
# ScriptedGateway queues can hand decompose a brief that validates on its first attempt (no
# repair, so the extra call count this adds is exactly one and deterministic) rather than crash
# with "pop from empty list" when its unscripted call finds nothing queued.
_FIXTURES_DIR = _RELAY_PY_ROOT.parents[2] / "fleet" / "contracts" / "fixtures" / "lld"
_COMPLETE_MODULE_REPLY = (_FIXTURES_DIR / "complete_module.json").read_text(encoding="utf-8")


# -------------------------------------------------------------------------------------------
# A1 -- real uvicorn subprocess + real stub-gateway subprocess fixture
# -------------------------------------------------------------------------------------------


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


class _RealRelay:
    def __init__(self, relay_url: str, gateway_record_path: Path) -> None:
        self.relay_url = relay_url
        self._gateway_record_path = gateway_record_path

    def gateway_requests(self) -> list[dict]:
        """Every request body the stub gateway has recorded so far, oldest first."""
        if not self._gateway_record_path.exists():
            return []
        return [
            json.loads(line)
            for line in self._gateway_record_path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]

    def last_gateway_request(self) -> dict:
        requests = self.gateway_requests()
        assert requests, "the stub gateway recorded no requests"
        return requests[-1]


@pytest.fixture(scope="module")
def real_relay(tmp_path_factory: pytest.TempPathFactory):
    """A real uvicorn subprocess (A1's requirement, not TestClient) plus a real stub-gateway
    subprocess standing in for the model -- two real, separate OS processes over real HTTP, the
    same shape as the lane contract's §6 manual drive.
    """
    tmp_dir = tmp_path_factory.mktemp("f04-wire")
    gateway_port = _free_port()
    relay_port = _free_port()
    gateway_record_path = tmp_dir / "gateway.jsonl"
    context_db_path = tmp_dir / "context.db"

    gateway_proc = subprocess.Popen(
        [sys.executable, str(_STUB_GATEWAY_SCRIPT), "--port", str(gateway_port), "--record", str(gateway_record_path)]
    )
    try:
        _wait_for_ready(f"http://127.0.0.1:{gateway_port}/healthz", timeout_s=15.0)

        env = {
            **os.environ,
            "ORB_GATEWAY_URL": f"http://127.0.0.1:{gateway_port}",
            "ORB_CONTEXT_DB_PATH": str(context_db_path),
        }
        relay_proc = subprocess.Popen(
            [sys.executable, "-m", "uvicorn", "orb_relay.app:app", "--host", "127.0.0.1", "--port", str(relay_port)],
            cwd=str(_RELAY_PY_ROOT),
            env=env,
        )
        try:
            _wait_for_ready(f"http://127.0.0.1:{relay_port}/healthz", timeout_s=30.0)
            yield _RealRelay(f"http://127.0.0.1:{relay_port}", gateway_record_path)
        finally:
            relay_proc.terminate()
            try:
                relay_proc.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                relay_proc.kill()
    finally:
        gateway_proc.terminate()
        try:
            gateway_proc.wait(timeout=5.0)
        except subprocess.TimeoutExpired:
            gateway_proc.kill()


def test_a1_1_build_mode_returns_200_not_500(real_relay: _RealRelay) -> None:
    """The regression pin: today (pre-F04) this is a 500 (KeyError on ResponseMode.BUILD)."""
    response = httpx.post(
        f"{real_relay.relay_url}/v1/respond",
        json={"tenant_id": "t-a1", "user_id": "u-a1", "session_id": "s-a1-1", "text": "I want a rate limiter", "mode": "build"},
        timeout=10.0,
    )
    assert response.status_code == 200


def test_a1_2_system_prompt_byte_matches_build_v1_md_at_the_gateway_boundary(real_relay: _RealRelay) -> None:
    """Test the wire, not the component: byte equality against the file on disk, not "contains the
    word build". A first turn in a fresh session has no grounding note and is not a duplicate, so
    the sent system prompt equals the loaded prompt exactly (no suffix).

    F03 (accepted contract §2.4) makes this same BUILD turn also fire decompose()'s own real
    gateway call(s) against this SAME stub-gateway subprocess and its one accumulating record
    file, so the conversational request is no longer reliably "the last" one recorded -- not even
    the last one THIS test's own turn produced, since decompose's fail-closed repair (the stub
    always replies with fixed non-JSON text, so decompose retries once) adds a further call after
    it. Isolate the requests this turn actually caused (a before/after slice of the accumulating
    log, since the fixture is module-scoped and earlier A1 tests already wrote to it), then find
    the one(s) BY CONTENT rather than by position -- the byte-match property itself is unchanged.

    Not asserting an exact count of matches: this fixture's tenant_id/user_id ("t-a1"/"u-a1") is
    shared with every other A1 test, and the stub always replies with the SAME fixed string
    (`_STUB_REPLY_TEXT`) -- so a later A1 turn can see its own prior assistant reply verbatim in
    its own history and trip `conversation_guard.py`'s own pre-existing verbatim-repeat repair,
    giving the conversational leg itself two calls (both, correctly, carrying the identical build
    system prompt). That is a real, pre-F03 mechanism this test was never pinning either way --
    what this test owns is that build.v1.md's exact bytes DID reach the gateway boundary at least
    once this turn, which `>= 1` states without also asserting something about an orthogonal guard
    behavior this case was never written to cover.
    """
    requests_before = len(real_relay.gateway_requests())
    response = httpx.post(
        f"{real_relay.relay_url}/v1/respond",
        json={"tenant_id": "t-a1", "user_id": "u-a1", "session_id": "s-a1-2", "text": "I want a rate limiter", "mode": "build"},
        timeout=10.0,
    )
    assert response.status_code == 200
    disk_prompt = (_DOMAIN_AGENTS_DIR / "build.v1.md").read_text(encoding="utf-8").strip()
    this_turns_requests = real_relay.gateway_requests()[requests_before:]
    matching = [r for r in this_turns_requests if r.get("system") == disk_prompt]
    assert len(matching) >= 1, (
        f"expected at least one gateway request from this turn whose system prompt byte-matches "
        f"build.v1.md; found none among {len(this_turns_requests)} requests this turn made"
    )


def test_a1_3_focus_prompt_is_a_different_string_than_build(real_relay: _RealRelay) -> None:
    """A build prompt that silently equals the focus prompt would pass A1.1/A1.2-by-substring and
    be worthless -- this is the discriminating check.
    """
    build_response = httpx.post(
        f"{real_relay.relay_url}/v1/respond",
        json={"tenant_id": "t-a1", "user_id": "u-a1", "session_id": "s-a1-3-build", "text": "hello", "mode": "build"},
        timeout=10.0,
    )
    assert build_response.status_code == 200
    build_system = real_relay.last_gateway_request()["system"]

    focus_response = httpx.post(
        f"{real_relay.relay_url}/v1/respond",
        json={"tenant_id": "t-a1", "user_id": "u-a1", "session_id": "s-a1-3-focus", "text": "hello", "mode": "focus"},
        timeout=10.0,
    )
    assert focus_response.status_code == 200
    focus_system = real_relay.last_gateway_request()["system"]

    disk_focus_prompt = (_DOMAIN_AGENTS_DIR / "focus-companion.v1.md").read_text(encoding="utf-8").strip()
    assert focus_system == disk_focus_prompt
    assert focus_system != build_system


def test_a1_4_build_response_envelope_has_non_null_build_with_at_least_one_register(real_relay: _RealRelay) -> None:
    response = httpx.post(
        f"{real_relay.relay_url}/v1/respond",
        json={
            "tenant_id": "t-a1",
            "user_id": "u-a1",
            "session_id": "s-a1-4",
            "text": "the interface is allow(key) -> bool",
            "mode": "build",
        },
        timeout=10.0,
    )
    assert response.status_code == 200
    body = response.json()
    assert body["mode"] == "build"
    assert body["build"] is not None
    assert len(body["build"]["registers"]) >= 1


def test_a1_5_focus_mode_response_has_a_null_build_field(real_relay: _RealRelay) -> None:
    response = httpx.post(
        f"{real_relay.relay_url}/v1/respond",
        json={"tenant_id": "t-a1", "user_id": "u-a1", "session_id": "s-a1-5", "text": "hello there", "mode": "focus"},
        timeout=10.0,
    )
    assert response.status_code == 200
    assert response.json()["build"] is None


def test_a1_6_unrecognized_mode_is_422_not_500_and_not_a_silent_default(real_relay: _RealRelay) -> None:
    response = httpx.post(
        f"{real_relay.relay_url}/v1/respond",
        json={"tenant_id": "t-a1", "user_id": "u-a1", "session_id": "s-a1-6", "text": "hello", "mode": "banana"},
        timeout=10.0,
    )
    assert response.status_code == 422


def test_a1_7_warmup_build_opening_and_backward_compatible_mode_omission(real_relay: _RealRelay) -> None:
    build_warmup = httpx.post(
        f"{real_relay.relay_url}/v1/session/warmup",
        json={"tenant_id": "t-a1", "user_id": "u-a1-warmup", "session_id": "s-a1-7-build", "mode": "build"},
        timeout=10.0,
    )
    assert build_warmup.status_code == 200
    assert build_warmup.json()["opening"] == "Build mode. What are we making?"

    omitted_warmup = httpx.post(
        f"{real_relay.relay_url}/v1/session/warmup",
        json={"tenant_id": "t-a1", "user_id": "u-a1-warmup", "session_id": "s-a1-7-omit"},
        timeout=10.0,
    )
    assert omitted_warmup.status_code == 200
    body = omitted_warmup.json()
    assert body["opening"] is None
    # Every OTHER field byte-identical to the documented pre-F04 shape -- the same assertions
    # `test_app_routes.py::test_warmup_returns_t0_context_pack_shape` (an untouched §5 A4 witness)
    # already pins for the same input shape.
    assert body["profile"] == {"tenant_id": "t-a1", "user_id": "u-a1-warmup"}
    assert body["recent_tasks"] == []
    assert body["open_loops"] == []
    assert body["embeddings"] == []
    assert body["open_session"] == {"tenant_id": "t-a1", "session_id": "s-a1-7-omit"}
    assert "presence.here.v1" in body["phrase_manifest"]


# -------------------------------------------------------------------------------------------
# A6 -- mode/prompt exhaustiveness (no server needed -- direct import)
# -------------------------------------------------------------------------------------------


def test_a6_every_response_mode_has_a_domain_prompt_that_resolves() -> None:
    """Directly pins §3.6(b): `set(ResponseMode) == set(_MODE_PROMPT_NAMES)`, and every value
    resolves through `load_agent_prompt` without raising. Makes today's defect
    (`ResponseMode.BUILD` shipped with no handler) non-reintroducible for any future mode too.
    """
    assert set(ResponseMode) == set(app_module._MODE_PROMPT_NAMES)
    for mode in ResponseMode:
        prompt_name = app_module._MODE_PROMPT_NAMES[mode]
        assert load_agent_prompt(prompt_name)  # must not raise, and must be non-empty


# -------------------------------------------------------------------------------------------
# A7 -- the safety-guard mode branch (in-process TestClient, same seam as test_app_routes.py)
# -------------------------------------------------------------------------------------------


class ScriptedGateway:
    def __init__(self, *replies: str) -> None:
        self._replies = list(replies)
        self.calls = 0

    async def complete(self, **_: object) -> GatewayCompletion:
        self.calls += 1
        return GatewayCompletion(self._replies.pop(0), UsageDelta(llm_tokens_in=3, llm_tokens_out=4))


def client_with(gateway: object) -> TestClient:
    async def override() -> object:
        return gateway

    app.dependency_overrides[get_gateway] = override
    return TestClient(app)


@pytest.fixture(autouse=True)
def isolated_stores(tmp_path, monkeypatch) -> None:
    """A7's tests use the in-process app (TestClient) -- give them their own empty sqlite files so
    they never read/write the real backend/relay-py/.data/context.db, matching
    test_app_routes.py's isolated_context_store fixture.
    """
    monkeypatch.setattr(app_module, "_context_store", ContextStore(tmp_path / "context.db"))
    monkeypatch.setattr(app_module, "_conversation_store", app_module.ConversationStore(tmp_path / "conversation.db"))
    monkeypatch.setattr(app_module, "_build_session_store", BuildSessionStore(tmp_path / "build.db"))


def teardown_function() -> None:
    app.dependency_overrides.clear()
    app_module._meters.clear()
    app_module._meters_last_seen.clear()


def test_a7_1_build_mode_permits_a_clarify_question_that_converse_would_veto() -> None:
    """The contrast IS the assertion -- A7.1 alone would pass on a guard that vetoes nothing."""
    clarify_text = "Which storage do you want -- Postgres or SQLite?"

    # F03 (accepted contract §2.4): a BUILD turn also fires decompose()'s own gateway call,
    # unconditionally, after the conversational leg above. Queue it a brief that validates on the
    # first attempt (the shared fixture, per lane contract §5 -- never hand-authored) so it needs
    # no repair of its own, keeping the total call count exactly 2 and fully deterministic; a
    # regression that makes BUILD's guard start vetoing/repairing the conversational leg would
    # still push this past 2, so the count keeps its original regression-catching power.
    build_gateway = ScriptedGateway(clarify_text, _COMPLETE_MODULE_REPLY)
    build_client = client_with(build_gateway)
    build_response = build_client.post(
        "/v1/respond",
        json={"session_id": "s-a7-1-build", "tenant_id": "t-a7", "user_id": "u-a7", "text": "what should I use for storage", "mode": "build"},
    )
    assert build_response.status_code == 200
    build_body = build_response.json()
    assert build_body["text"] == clarify_text
    assert build_body["degraded"] is False
    # 1 conversational (unvetoed -- the guard never spent its one repair call) + 1 decompose
    # (F03 §2.4 fires every build turn; scripted to validate first-try, so it never spends ITS
    # one repair either) -- not 1, and not 3+, which is what unvetoed-but-decompose-repairing or
    # vetoed-conversational would each look like.
    assert build_gateway.calls == 2

    converse_gateway = ScriptedGateway(clarify_text, "Let's go with SQLite to start; you can swap it later.")
    converse_client = client_with(converse_gateway)
    converse_response = converse_client.post(
        "/v1/respond",
        json={"session_id": "s-a7-1-converse", "tenant_id": "t-a7", "user_id": "u-a7", "text": "what should I use for storage", "mode": "converse"},
    )
    assert converse_response.status_code == 200
    converse_body = converse_response.json()
    assert converse_body["text"] != clarify_text
    assert converse_body["degraded"] is True


def test_a7_2_build_mode_still_vetoes_shame_adjacent_output() -> None:
    """Universal control intact: build mode skips the load-direction veto, not the safety veto.
    Reuses an existing distress case from test_conversation_guard.py's corpus BY REFERENCE (the
    "maggot" red-team case) rather than inventing a new one or copying the corpus.
    """
    prompt, responses = _DISTRESS_HARM_REGRESSION_CASES[5]
    assert "maggot" in responses[0]  # pins which case this is, so an upstream reorder fails loudly

    # F03 (accepted contract §2.4): append one further reply for decompose's own, separate,
    # unconditional gateway call on this same BUILD turn -- a first-try-valid brief (the shared
    # fixture, per lane contract §5) so it needs no repair of its own. `responses` itself (the
    # shared test_conversation_guard.py corpus) is untouched; this only extends the local queue.
    gateway = ScriptedGateway(*responses, _COMPLETE_MODULE_REPLY)
    client = client_with(gateway)
    response = client.post(
        "/v1/respond",
        json={"session_id": "s-a7-2", "tenant_id": "t-a7", "user_id": "u-a7", "text": prompt, "mode": "build"},
    )
    assert response.status_code == 200
    body = response.json()
    assert "maggot" not in body["text"].lower()
    assert body["degraded"] is True
