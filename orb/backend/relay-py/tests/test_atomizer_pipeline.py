"""Atomizer pipeline: structured decoding -> validate -> repair-once -> fail-closed.

The cases that matter are the failure paths (docs/adr/LESSONS.md L2 — a suite that only feeds
well-formed input proves nothing): malformed JSON, schema violations, semantic-rubric violations,
and the two-strikes fallback.
"""

import json

import pytest

from orb_relay.proxy.atomizer import (
    FALLBACK_STEP,
    MAX_REPAIR_ATTEMPTS,
    AtomizeSource,
    atomize,
)
from orb_relay.proxy.gateway_client import GatewayCompletion, GatewayError
from orb_relay.proxy.schemas import AtomizerErrorCode
from orb_relay.cost.meter import UsageDelta


class ScriptedGateway:
    """Returns a queued reply per call and records the prompts it was given, so a test can assert
    the repair prompt actually named the violation.
    """

    def __init__(self, *replies: str) -> None:
        self._replies = list(replies)
        self.prompts: list[str] = []

    async def complete(
        self, *, tenant_id: str, system: str, user_text: str, max_tokens: int
    ) -> GatewayCompletion:
        self.prompts.append(user_text)
        if not self._replies:
            raise AssertionError("gateway called more times than the test scripted")
        return GatewayCompletion(self._replies.pop(0), UsageDelta(llm_tokens_in=3, llm_tokens_out=4))


class RaisingGateway:
    def __init__(self, error: Exception) -> None:
        self._error = error

    async def complete(self, **_: object) -> GatewayCompletion:
        raise self._error


def good_payload(n: int = 2) -> str:
    steps = [
        {"step_text": "Open the tax portal", "est_min": 1, "done_signal": "portal is on screen"},
        {"step_text": "Type your PAN into the login box", "est_min": 2, "done_signal": "PAN is filled"},
    ][:n]
    return json.dumps({"steps": steps, "steps_total": len(steps)})


async def test_clean_first_attempt_is_source_model() -> None:
    gateway = ScriptedGateway(good_payload())
    result = await atomize("file my taxes", gateway=gateway, tenant_id="t1")
    assert result.source is AtomizeSource.MODEL
    assert result.output.steps_total == 2
    assert result.rejections == ()
    assert len(gateway.prompts) == 1


async def test_markdown_fenced_json_is_accepted_on_first_attempt() -> None:
    # Regression test: live-verified against a real Gemini 2.5 Flash-Lite call, which wraps JSON
    # responses in a markdown code fence even when the system prompt says "no prose" — this used
    # to burn the repair attempt and fail closed on every single real Gemini call.
    fenced = "```json\n" + good_payload() + "\n```"
    gateway = ScriptedGateway(fenced)
    result = await atomize("file my taxes", gateway=gateway, tenant_id="t1")
    assert result.source is AtomizeSource.MODEL
    assert result.rejections == ()
    assert len(gateway.prompts) == 1


async def test_malformed_json_is_repaired_then_accepted() -> None:
    gateway = ScriptedGateway("not json at all", good_payload())
    result = await atomize("file my taxes", gateway=gateway, tenant_id="t1")
    assert result.source is AtomizeSource.MODEL_REPAIRED
    assert len(result.rejections) == 1
    assert result.rejections[0].code is AtomizerErrorCode.SCHEMA_INVALID
    # the repair prompt must name the specific violation, not just re-ask
    assert "rejected" in gateway.prompts[1]
    assert result.usage.llm_tokens_in == 6
    assert result.usage.llm_tokens_out == 8


async def test_two_failures_fall_closed_to_the_deterministic_step() -> None:
    gateway = ScriptedGateway("garbage", "still garbage")
    result = await atomize("file my taxes", gateway=gateway, tenant_id="t1")
    assert result.source is AtomizeSource.FALLBACK
    assert result.output.steps == [FALLBACK_STEP]
    assert result.output.steps_total == 1
    assert len(result.rejections) == MAX_REPAIR_ATTEMPTS + 1


async def test_never_retries_more_than_once() -> None:
    gateway = ScriptedGateway("bad", "bad")  # only two scripted; a third call raises
    await atomize("t", gateway=gateway, tenant_id="t1")
    assert len(gateway.prompts) == MAX_REPAIR_ATTEMPTS + 1


async def test_rejects_non_atomic_step_with_two_actions() -> None:
    payload = json.dumps(
        {
            "steps": [
                {
                    "step_text": "Open the portal and then log in",
                    "est_min": 1,
                    "done_signal": "logged in",
                }
            ],
            "steps_total": 1,
        }
    )
    gateway = ScriptedGateway(payload, good_payload(1))
    result = await atomize("t", gateway=gateway, tenant_id="t1")
    assert result.rejections[0].code is AtomizerErrorCode.STEP_NOT_ATOMIC
    assert result.source is AtomizeSource.MODEL_REPAIRED


async def test_rejects_compound_action_joined_by_and() -> None:
    payload = json.dumps(
        {
            "steps": [
                {
                    "step_text": "Open email client and find Sarah's contact",
                    "est_min": 1,
                    "done_signal": "Sarah's contact is open",
                }
            ],
            "steps_total": 1,
        }
    )
    gateway = ScriptedGateway(payload, good_payload(1))
    result = await atomize("email Sarah", gateway=gateway, tenant_id="t1")
    assert result.rejections[0].code is AtomizerErrorCode.STEP_NOT_ATOMIC
    assert "compound-action" in result.rejections[0].detail


async def test_rejects_step_containing_a_decision() -> None:
    payload = json.dumps(
        {
            "steps": [
                {"step_text": "Decide which form you need", "est_min": 1, "done_signal": "form chosen"}
            ],
            "steps_total": 1,
        }
    )
    gateway = ScriptedGateway(payload, good_payload(1))
    result = await atomize("t", gateway=gateway, tenant_id="t1")
    assert result.rejections[0].code is AtomizerErrorCode.STEP_NOT_ATOMIC
    assert "decision marker" in result.rejections[0].detail


async def test_rejects_first_step_too_costly_to_start() -> None:
    payload = json.dumps(
        {
            "steps": [
                {"step_text": "Read the entire tax guide", "est_min": 9, "done_signal": "guide read"}
            ],
            "steps_total": 1,
        }
    )
    gateway = ScriptedGateway(payload, good_payload(1))
    result = await atomize("t", gateway=gateway, tenant_id="t1")
    assert result.rejections[0].code is AtomizerErrorCode.STEP_NOT_ATOMIC
    assert "initiation-cost" in result.rejections[0].detail


async def test_rejects_first_step_with_unbounded_scope() -> None:
    payload = json.dumps(
        {
            "steps": [
                {
                    "step_text": "Clear everything off the desk surface",
                    "est_min": 2,
                    "done_signal": "desk surface is clear",
                }
            ],
            "steps_total": 1,
        }
    )
    gateway = ScriptedGateway(payload, good_payload(1))
    result = await atomize("organize my desk", gateway=gateway, tenant_id="t1")
    assert result.rejections[0].code is AtomizerErrorCode.STEP_NOT_ATOMIC
    assert "broad-scope" in result.rejections[0].detail


async def test_rejects_json_array_instead_of_object() -> None:
    gateway = ScriptedGateway("[1,2,3]", good_payload(1))
    result = await atomize("t", gateway=gateway, tenant_id="t1")
    assert result.rejections[0].code is AtomizerErrorCode.SCHEMA_INVALID
    assert "expected object" in result.rejections[0].detail


async def test_budget_error_propagates_rather_than_falling_back() -> None:
    """A 402 is a real spend guard (C12), not model misbehaviour — swallowing it into a fallback
    step would silently continue a session the cost plane refused to fund.
    """
    gateway = RaisingGateway(GatewayError(402, "BUDGET_EXHAUSTED", "no allowance"))
    with pytest.raises(GatewayError) as exc:
        await atomize("t", gateway=gateway, tenant_id="t1")
    assert exc.value.status == 402
