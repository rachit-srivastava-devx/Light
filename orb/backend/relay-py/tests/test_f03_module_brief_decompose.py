"""F03 -- module-brief atomizer: `decompose()` composed into `/v1/respond`'s BUILD branch (lane
contract `docs/lane-contracts/F03-module-brief-atomizer.md` §6). The nine cases below are the
contract; case IDs are load-bearing in the test names (§6's own words).

The builder may not edit this file (lane contract, front matter) other than creating it verbatim
from §6. If a case looks wrong, escalate.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

import orb_relay.app as app_module
from orb_relay.app import app, get_gateway
from orb_relay.build.build_session import derive_turn_evidence
from orb_relay.cognitive.belief import CoverageSlot, Register, initial_registers
from orb_relay.cost.meter import DEFAULT_SESSION_RESERVATION_PAISE, Rates, UsageDelta
from orb_relay.proxy.gateway_client import GatewayCompletion, GatewayError
from orb_relay.proxy.lld_decomposer import decompose as _real_decompose
from orb_relay.proxy.lld_schemas import validate_module_brief
from orb_relay.store.build_session_store import BuildSessionStore
from orb_relay.store.context_store import ContextStore

# NOTE: `decompose`/`DecomposeKind` (lld_decomposer) and `DecomposeOutcome` (proxy/schemas) already
# exist or are new respectively -- this file deliberately never imports `DecomposeOutcome` at
# module level (it isn't needed: tests inspect the JSON response body, never construct the wire
# model directly), so the whole file stays IMPORTABLE at the pre-F03 lane base. That is what lets
# T5a/T5c genuinely PASS (not error-out on collection) before any production change exists.

_RELAY_PY_ROOT = Path(__file__).resolve().parents[1]  # orb/backend/relay-py
_WORKTREE_ROOT = Path(__file__).resolve().parents[4]  # .../Light/.worktrees/F03-atomizer
_FIXTURES_DIR = _WORKTREE_ROOT / "fleet" / "contracts" / "fixtures" / "lld"
_LANE_BASE_SHA = "6fd66f3"

COMPLETE_MODULE = json.loads((_FIXTURES_DIR / "complete_module.json").read_text(encoding="utf-8"))
ONE_ALTERNATIVE = json.loads((_FIXTURES_DIR / "one_alternative.json").read_text(encoding="utf-8"))

# Executed, not assumed (contract §5): both fixtures behave exactly as the lane contract states at
# contract time -- a valid brief and a brief invalid in exactly one named way.
assert validate_module_brief(COMPLETE_MODULE) is not None, "complete_module.json must validate"
assert validate_module_brief(ONE_ALTERNATIVE) is None, "one_alternative.json must NOT validate"

_TENANT_ID = "t-f03"


def _base_request(session_id: str) -> dict[str, object]:
    """`user_id` is derived from `session_id` so every distinct scenario in this file gets its own
    (tenant_id, user_id) pair.

    `ConversationStore.recent()` deliberately CARRIES OVER a user's very recent turns into a
    session that has none of its own yet (the F01 app-reload-continuity fix). Without this, two
    unrelated scenarios in the SAME test function that happen to share a (tenant_id, user_id) --
    e.g. T6's four independent mode checks, or T8c's "with"/"without" comparison -- would leak one
    call's stored reply into the next call's `history`, which can trip
    `conversation_guard.py`'s verbatim-repeat repair (a SECOND, unscripted gateway call) purely
    because two scenarios coincidentally used the same reply text. Scoping `user_id` to
    `session_id` makes every scenario independent by construction instead of by coincidence of
    reply-text wording.
    """
    return {"tenant_id": _TENANT_ID, "user_id": f"u-{session_id}"}


def _build_payload(session_id: str, text: str) -> dict[str, object]:
    return {**_base_request(session_id), "session_id": session_id, "text": text, "mode": "build"}


class ScriptedGateway:
    """Same shape as `tests/test_app_routes.py`'s `ScriptedGateway` -- one queued reply per call,
    every call's `user_text` recorded so a test can assert the repair prompt named the violation.
    """

    def __init__(self, *replies: str) -> None:
        self._replies = list(replies)
        self.calls = 0
        self.prompts: list[str] = []

    async def complete(self, **kwargs: object) -> GatewayCompletion:
        self.calls += 1
        self.prompts.append(str(kwargs.get("user_text", "")))
        if not self._replies:
            raise AssertionError("gateway called more times than the test scripted")
        return GatewayCompletion(
            self._replies.pop(0), UsageDelta(llm_tokens_in=3, llm_tokens_out=4)
        )


def client_with(gateway: object) -> TestClient:
    async def override() -> object:
        return gateway

    app.dependency_overrides[get_gateway] = override
    return TestClient(app)


@pytest.fixture(autouse=True)
def isolated_stores(tmp_path, monkeypatch) -> None:
    """Never read/write the real backend/relay-py/.data/context.db -- matches
    test_app_routes.py's isolated_context_store / test_f04_build_mode_wire.py's isolated_stores.
    """
    monkeypatch.setattr(app_module, "_context_store", ContextStore(tmp_path / "context.db"))
    monkeypatch.setattr(
        app_module, "_conversation_store", app_module.ConversationStore(tmp_path / "conversation.db")
    )
    monkeypatch.setattr(
        app_module, "_build_session_store", BuildSessionStore(tmp_path / "build.db")
    )


def teardown_function() -> None:
    app.dependency_overrides.clear()
    app_module._meters.clear()
    app_module._meters_last_seen.clear()


# ---------------------------------------------------------------------------------------------
# T1 -- composition: a build turn produces a real ModuleBrief through the real app
# ---------------------------------------------------------------------------------------------


def test_f03_t1_composition_build_turn_produces_a_real_module_brief() -> None:
    gateway = ScriptedGateway("Got it, let's shape this.", json.dumps(COMPLETE_MODULE))
    client = client_with(gateway)
    response = client.post(
        "/v1/respond", json=_build_payload("s-t1", "I want to build a rate limiter")
    )
    assert response.status_code == 200
    body = response.json()
    assert body["decompose"] is not None
    assert body["decompose"]["kind"] == "brief"
    # Re-validated through F02's own validator, not merely shape-checked by this test.
    assert validate_module_brief(body["decompose"]["brief"]) is not None
    assert body["decompose"]["brief"]["node_id"] == COMPLETE_MODULE["node_id"]


# ---------------------------------------------------------------------------------------------
# T2 -- fail-closed: a garbage decode never yields a brief
# ---------------------------------------------------------------------------------------------


def test_f03_t2_fail_closed_garbage_decode_never_yields_a_brief() -> None:
    gateway = ScriptedGateway("Sure, tell me more.", "not json at all", "still garbage")
    client = client_with(gateway)
    response = client.post("/v1/respond", json=_build_payload("s-t2", "let's build something"))
    assert response.status_code == 200
    body = response.json()
    assert body["decompose"]["kind"] == "clarify_request"
    assert body["decompose"]["brief"] is None
    assert body["decompose"]["missing"]
    # Checked over the SERIALIZED envelope, not one field -- a brief smuggled into a sibling key
    # still fails this.
    assert "node_id" not in json.dumps(body["decompose"])


# ---------------------------------------------------------------------------------------------
# T3 -- the repair prompt names the actual violation (the §0.2 fix)
# ---------------------------------------------------------------------------------------------


def test_f03_t3_repair_prompt_names_the_actual_violation() -> None:
    gateway = ScriptedGateway(
        "Sure, tell me more.", json.dumps(ONE_ALTERNATIVE), json.dumps(COMPLETE_MODULE)
    )
    client = client_with(gateway)
    response = client.post(
        "/v1/respond", json=_build_payload("s-t3", "let's build the alt-checker module")
    )
    assert response.status_code == 200
    # gateway.calls = 1 conversational reply + 2 decompose attempts (first + one repair).
    assert gateway.calls - 1 == 2
    second_decompose_prompt = gateway.prompts[-1]
    assert "alternatives" in second_decompose_prompt
    assert "module brief failed lld.v1 schema validation" not in second_decompose_prompt
    assert response.json()["decompose"]["kind"] == "brief"


# ---------------------------------------------------------------------------------------------
# T4 -- repair is bounded at exactly one
# ---------------------------------------------------------------------------------------------


def test_f03_t4_repair_is_bounded_at_exactly_one() -> None:
    gateway = ScriptedGateway(
        "Sure, tell me more.", json.dumps(ONE_ALTERNATIVE), json.dumps(ONE_ALTERNATIVE)
    )
    client = client_with(gateway)
    response = client.post(
        "/v1/respond", json=_build_payload("s-t4", "let's build the alt-checker module")
    )
    assert response.status_code == 200
    assert gateway.calls - 1 == 2  # not 3 -- exactly one repair attempt, then fail closed
    assert response.json()["decompose"]["kind"] == "clarify_request"


# ---------------------------------------------------------------------------------------------
# T5 -- focus mode untouched, asserted in BOTH directions
# ---------------------------------------------------------------------------------------------


def test_f03_t5a_atomize_route_never_calls_decompose_and_response_shape_is_unchanged(
    monkeypatch,
) -> None:
    real_atomize = app_module.atomize
    atomize_calls: list[int] = []
    decompose_calls: list[int] = []

    async def spy_atomize(*args: object, **kwargs: object) -> object:
        atomize_calls.append(1)
        return await real_atomize(*args, **kwargs)

    async def spy_decompose(*args: object, **kwargs: object) -> object:
        decompose_calls.append(1)
        raise AssertionError("decompose must never be called from /v1/atomize")

    monkeypatch.setattr(app_module, "atomize", spy_atomize)
    # raising=False: `app_module.decompose` does not exist at the pre-F03 lane base -- this must
    # still succeed there (T5a is a green-on-arrival LOCK, not a red case).
    monkeypatch.setattr(app_module, "decompose", spy_decompose, raising=False)

    good_payload = json.dumps(
        {
            "steps": [{"step_text": "Open the tax portal", "est_min": 1, "done_signal": "portal on screen"}],
            "steps_total": 1,
        }
    )
    gateway = ScriptedGateway(good_payload)
    client = client_with(gateway)
    response = client.post(
        "/v1/atomize",
        json={"session_id": "s-t5a", "tenant_id": "t-f03", "user_id": "u-f03", "task": "file my taxes"},
    )
    assert response.status_code == 200
    assert len(atomize_calls) == 1
    assert len(decompose_calls) == 0
    from orb_relay.app import AtomizeResponse

    assert set(response.json().keys()) == set(AtomizeResponse.model_fields.keys())


def test_f03_t5b_build_mode_calls_decompose_and_never_atomize(monkeypatch) -> None:
    atomize_calls: list[int] = []
    decompose_calls: list[int] = []

    async def spy_atomize(*args: object, **kwargs: object) -> object:
        atomize_calls.append(1)
        raise AssertionError("atomize must never be called from a BUILD-mode /v1/respond turn")

    async def spy_decompose(*args: object, **kwargs: object) -> object:
        decompose_calls.append(1)
        return await _real_decompose(*args, **kwargs)

    monkeypatch.setattr(app_module, "atomize", spy_atomize)
    monkeypatch.setattr(app_module, "decompose", spy_decompose, raising=False)

    gateway = ScriptedGateway("Sure, tell me more.", json.dumps(COMPLETE_MODULE))
    client = client_with(gateway)
    response = client.post("/v1/respond", json=_build_payload("s-t5b", "let's build a thing"))
    assert response.status_code == 200
    assert len(atomize_calls) == 0
    assert len(decompose_calls) == 1


def test_f03_t5c_atomizer_and_semantic_checks_are_byte_identical_to_the_lane_base() -> None:
    result = subprocess.run(
        [
            "git",
            "diff",
            "--numstat",
            f"{_LANE_BASE_SHA}...HEAD",
            "--",
            "orb/backend/relay-py/src/orb_relay/proxy/atomizer.py",
            "orb/backend/relay-py/src/orb_relay/proxy/semantic_checks.py",
        ],
        cwd=_WORKTREE_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.stdout == "", result.stdout


# ---------------------------------------------------------------------------------------------
# T6 -- mode stays explicit, never inferred
# ---------------------------------------------------------------------------------------------


def test_f03_t6_mode_stays_explicit_never_inferred() -> None:
    focus_client = client_with(ScriptedGateway("A short reply."))
    focus_response = focus_client.post(
        "/v1/respond",
        json={**_base_request("s-t6-focus"), "session_id": "s-t6-focus", "text": "hello", "mode": "focus"},
    )
    assert focus_response.status_code == 200
    assert focus_response.json().get("decompose") is None

    omitted_client = client_with(ScriptedGateway("A short reply."))
    omitted_response = omitted_client.post(
        "/v1/respond", json={**_base_request("s-t6-omit"), "session_id": "s-t6-omit", "text": "hello"}
    )
    assert omitted_response.status_code == 200
    assert omitted_response.json().get("decompose") is None

    bad_client = client_with(ScriptedGateway("unused"))
    bad_response = bad_client.post(
        "/v1/respond",
        json={**_base_request("s-t6-bad"), "session_id": "s-t6-bad", "text": "hello", "mode": "banana"},
    )
    assert bad_response.status_code == 422

    build_client = client_with(ScriptedGateway("A short reply.", json.dumps(COMPLETE_MODULE)))
    build_response = build_client.post(
        "/v1/respond", json=_build_payload("s-t6-build", "let's build a thing")
    )
    assert build_response.status_code == 200
    assert build_response.json()["decompose"] is not None


# ---------------------------------------------------------------------------------------------
# T7 -- the F04 composition: a brief moves the real registers; a clarify request moves nothing
# ---------------------------------------------------------------------------------------------


def test_f03_t7a_a_validated_brief_moves_coverage_registers_strictly() -> None:
    # Text chosen so `derive_turn_evidence` (which always fires) does NOT itself match the
    # interface/acceptance keyword sets -- so the whole delta below is attributable to
    # `derive_brief_evidence`, not a coincidental double-hit.
    text = "let's design this module"
    before = initial_registers()

    gateway = ScriptedGateway("Let's shape this.", json.dumps(COMPLETE_MODULE))
    client = client_with(gateway)
    response = client.post("/v1/respond", json=_build_payload("s-t7a", text))
    assert response.status_code == 200

    after = app_module._build_session_store.registers(
        tenant_id=_TENANT_ID, user_id="u-s-t7a", session_id="s-t7a"
    )
    interface_key = (Register.COVERAGE, CoverageSlot.INTERFACE)
    acceptance_key = (Register.COVERAGE, CoverageSlot.ACCEPTANCE)
    assert after[interface_key].confidence > before[interface_key].confidence
    assert after[acceptance_key].confidence > before[acceptance_key].confidence


def test_f03_t7b_a_clarify_request_launders_no_coverage(tmp_path) -> None:
    text = "let's design this module"
    gateway = ScriptedGateway("Sure, tell me more.", "garbage", "still garbage")
    client = client_with(gateway)
    response = client.post("/v1/respond", json=_build_payload("s-t7b", text))
    assert response.status_code == 200
    assert response.json()["decompose"]["kind"] == "clarify_request"

    log_after_clarify = app_module._build_session_store.log(
        tenant_id=_TENANT_ID, user_id="u-s-t7b", session_id="s-t7b"
    )

    # Control: a turn where ONLY derive_turn_evidence ran, constructed directly (bypassing
    # decompose and the HTTP route entirely) so this is an independent reference, not a second
    # HTTP call that could itself be contaminated by the same laundering bug this test looks for.
    control_store = BuildSessionStore(tmp_path / "t7b-control.db")
    control_store.append_evidence(
        tenant_id=_TENANT_ID, user_id="u-control", session_id="control", entry=derive_turn_evidence(text)
    )
    log_control = control_store.log(tenant_id=_TENANT_ID, user_id="u-control", session_id="control")

    assert len(log_after_clarify) == len(log_control) == 1


def test_f03_t7c_contradiction_register_is_unchanged_by_either_outcome() -> None:
    prior_contradiction = initial_registers()[(Register.CONTRADICTION, None)]
    text = "let's design this module"

    brief_client = client_with(ScriptedGateway("Let's shape this.", json.dumps(COMPLETE_MODULE)))
    brief_client.post("/v1/respond", json=_build_payload("s-t7c-brief", text))
    brief_registers = app_module._build_session_store.registers(
        tenant_id=_TENANT_ID, user_id="u-s-t7c-brief", session_id="s-t7c-brief"
    )
    assert brief_registers[(Register.CONTRADICTION, None)] == prior_contradiction

    clarify_client = client_with(ScriptedGateway("Sure.", "garbage", "still garbage"))
    clarify_client.post("/v1/respond", json=_build_payload("s-t7c-clarify", text))
    clarify_registers = app_module._build_session_store.registers(
        tenant_id=_TENANT_ID, user_id="u-s-t7c-clarify", session_id="s-t7c-clarify"
    )
    assert clarify_registers[(Register.CONTRADICTION, None)] == prior_contradiction


# ---------------------------------------------------------------------------------------------
# T8 -- budget admission is not bypassed, and the added spend is a measured number
# ---------------------------------------------------------------------------------------------


def test_f03_t8a_decompose_admission_refused_with_zero_gateway_calls_when_exhausted() -> None:
    # Enough for the conversational leg's real (scripted) cost (1 paise at default rates) and
    # nothing left over for decompose's own reservation.
    app_module._meters[("t-f03", "s-t8a")] = app_module.SessionMeter(
        tenant_id="t-f03", session_id="s-t8a", user_id="u-f03", reservation_paise=1
    )
    gateway = ScriptedGateway("A short reply.", json.dumps(COMPLETE_MODULE))
    client = client_with(gateway)
    response = client.post(
        "/v1/respond", json=_build_payload("s-t8a", "let's build a rate limiter")
    )
    assert response.status_code == 402
    assert gateway.calls == 1  # the conversational reply landed; decompose's gateway was never reached


def test_f03_t8b_gateway_error_on_decompose_degrades_without_leaking_the_reservation(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        app_module, "_rates", Rates(paise_per_1k_llm_tokens_in=1000, paise_per_1k_llm_tokens_out=1000)
    )
    decompose_calls: list[int] = []

    async def flaky_decompose(*args: object, **kwargs: object) -> object:
        decompose_calls.append(1)
        raise GatewayError(502, "UPSTREAM_FAILURE", "model unavailable")

    monkeypatch.setattr(app_module, "decompose", flaky_decompose, raising=False)

    gateway = ScriptedGateway("A short reply.")
    client = client_with(gateway)
    response = client.post(
        "/v1/respond", json=_build_payload("s-t8b", "let's build a rate limiter")
    )
    assert response.status_code == 200
    body = response.json()
    assert body["decompose"] is None
    assert decompose_calls == [1]
    # Only the conversational leg's settled cost: ceil(3*1000/1000 + 4*1000/1000) = 7 paise. The
    # decompose reservation was released, not settled -- so it contributes nothing here.
    assert body["spent_paise"] == 7


def test_f03_t8c_records_the_decompose_spend_delta_as_a_pct_of_the_session_reservation(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        app_module, "_rates", Rates(paise_per_1k_llm_tokens_in=1000, paise_per_1k_llm_tokens_out=1000)
    )

    with_client = client_with(ScriptedGateway("A short reply.", json.dumps(COMPLETE_MODULE)))
    with_response = with_client.post(
        "/v1/respond", json=_build_payload("s-t8c-with", "let's build a rate limiter")
    )
    assert with_response.status_code == 200
    spent_with_decompose = with_response.json()["spent_paise"]

    without_client = client_with(ScriptedGateway("A short reply."))
    without_response = without_client.post(
        "/v1/respond",
        json={
            **_base_request("s-t8c-without"),
            "session_id": "s-t8c-without",
            "text": "let's build a rate limiter",
            "mode": "focus",
        },
    )
    assert without_response.status_code == 200
    spent_without_decompose = without_response.json()["spent_paise"]

    delta_paise = spent_with_decompose - spent_without_decompose
    delta_pct = (delta_paise / DEFAULT_SESSION_RESERVATION_PAISE) * 100
    # RECORD, not merely assert (§6.8c) -- run with `-s` to capture this line for the evidence file.
    print(
        f"T8C_SPENT_WITH_DECOMPOSE_PAISE={spent_with_decompose} "
        f"T8C_SPENT_WITHOUT_DECOMPOSE_PAISE={spent_without_decompose} "
        f"T8C_DELTA_PAISE={delta_paise} T8C_DELTA_PCT_OF_RESERVATION={delta_pct:.2f}"
    )
    assert delta_paise >= 0
    assert delta_pct < 25  # §2.4's revive trigger -- observed, not assumed
