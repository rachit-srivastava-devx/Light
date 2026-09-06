from __future__ import annotations

import orb_relay.app as app_module
import pytest
from fastapi.testclient import TestClient
from orb_relay.app import app, get_gateway
from orb_relay.cost.meter import UsageDelta
from orb_relay.proxy.conversation_guard import (
    WaitCompanionPoolExhausted,
    WaitDirective,
    WaitSilenceBudgetExceeded,
    build_wait_companion,
    complete_guarded_conversation,
    repeats_prior_reply,
)
from orb_relay.proxy.gateway_client import GatewayCompletion
from orb_relay.proxy.prompts import load_wait_companion_turns
from orb_relay.store.context_store import ContextStore
from orb_relay.store.conversation_store import ConversationStore
from pydantic import ValidationError


class ScriptedGateway:
    def __init__(self, *replies: str) -> None:
        self.replies = list(replies)
        self.calls: list[dict[str, object]] = []

    async def complete(self, **kwargs: object) -> GatewayCompletion:
        self.calls.append(kwargs)
        return GatewayCompletion(self.replies.pop(0), UsageDelta(llm_tokens_in=3, llm_tokens_out=4))


@pytest.fixture
def route_client(tmp_path, monkeypatch):
    gateway = ScriptedGateway("[warm] I heard you. This takes priority over the waiting chat.")

    async def override() -> ScriptedGateway:
        return gateway

    monkeypatch.setattr(app_module, "_context_store", ContextStore(tmp_path / "context.db"))
    monkeypatch.setattr(
        app_module,
        "_conversation_store",
        ConversationStore(tmp_path / "conversation.db"),
    )
    app_module._meters.clear()
    app_module._meters_last_seen.clear()
    app.dependency_overrides[get_gateway] = override
    yield TestClient(app), gateway
    app.dependency_overrides.clear()
    app_module._meters.clear()
    app_module._meters_last_seen.clear()


def wait_payload(
    event: str,
    *,
    elapsed_ms: int = 0,
    budget_ms: int = 5_000,
) -> dict[str, object]:
    payload: dict[str, object] = {
        "purpose": "work",
        "event": event,
        "silence_budget_ms": budget_ms,
        "elapsed_since_activity_ms": elapsed_ms,
    }
    if event == "started":
        payload.update(
            estimate_source="caller_budget",
            estimated_duration_ms=11 * 60_000,
        )
    return payload


def request_payload(*, text: str, wait: dict[str, object], session_id: str = "wait-s1") -> dict:
    return {
        "tenant_id": "wait-tenant",
        "user_id": "wait-user",
        "session_id": session_id,
        "mode": "converse",
        "text": text,
        "wait": wait,
    }


def test_wait_contract_requires_real_work_purpose_and_sourced_duration() -> None:
    with pytest.raises(ValidationError):
        WaitDirective.model_validate(
            {
                "event": "started",
                "estimate_source": "caller_budget",
                "estimated_duration_ms": 60_000,
                "silence_budget_ms": 5_000,
                "elapsed_since_activity_ms": 0,
            }
        )
    with pytest.raises(ValidationError):
        WaitDirective.model_validate(
            {
                "purpose": "teach_question",
                "event": "started",
                "estimate_source": "caller_budget",
                "estimated_duration_ms": 60_000,
                "silence_budget_ms": 5_000,
                "elapsed_since_activity_ms": 0,
            }
        )


def test_started_wait_speaks_the_passed_estimate_without_a_model_or_clock() -> None:
    directive = WaitDirective.model_validate(wait_payload("started"))
    result = build_wait_companion(directive)

    assert "11 minutes" in result.text
    assert result.source == "wait_companion"
    assert result.usage == UsageDelta()
    assert result.wait is not None
    assert result.wait.estimated_duration_ms == 11 * 60_000
    assert result.wait.companion_emitted is True


def test_unknown_duration_states_uncertainty_and_the_passed_check_in() -> None:
    directive = WaitDirective(
        purpose="work",
        event="started",
        estimate_source="unknown",
        next_check_in_ms=90_000,
        silence_budget_ms=5_000,
        elapsed_since_activity_ms=0,
    )
    result = build_wait_companion(directive)

    assert "do not know exactly how long" in result.text
    assert "1 minute and 30 seconds" in result.text
    assert result.wait is not None and result.wait.next_check_in_ms == 90_000


def test_companion_pool_reuses_the_existing_listener_facing_repeat_rule() -> None:
    history: list[dict[str, str]] = []
    emitted: list[str] = []
    for index in range(len(load_wait_companion_turns())):
        directive = WaitDirective(
            purpose="work",
            event="companion_due",
            silence_budget_ms=5_000,
            elapsed_since_activity_ms=4_000,
        )
        result = build_wait_companion(directive, history)
        assert not repeats_prior_reply(result.text, history, within_prior_reply=True)
        emitted.append(result.text)
        history.append({"role": "assistant", "content": result.text})

    assert len(emitted) == len(set(emitted)) == len(load_wait_companion_turns())
    with pytest.raises(WaitCompanionPoolExhausted):
        build_wait_companion(directive, history)


def test_silence_budget_breach_is_a_failing_state() -> None:
    directive = WaitDirective(
        purpose="work",
        event="companion_due",
        silence_budget_ms=5_000,
        elapsed_since_activity_ms=6_000,
    )
    with pytest.raises(WaitSilenceBudgetExceeded) as raised:
        build_wait_companion(directive)
    assert raised.value.elapsed_ms == 6_000
    assert raised.value.budget_ms == 5_000


@pytest.mark.asyncio
async def test_user_interrupt_wins_before_any_companion_turn() -> None:
    gateway = ScriptedGateway("[warm] Your new thought wins; I am listening to that now.")
    result = await complete_guarded_conversation(
        gateway=gateway,
        tenant_id="t1",
        user_id="u1",
        session_id="s1",
        system="system",
        user_text="Wait, I need to tell you something else.",
        max_tokens=180,
        mode="converse",
        wait=WaitDirective(
            purpose="work",
            event="user_interrupt",
            silence_budget_ms=5_000,
            elapsed_since_activity_ms=5_000,
        ),
    )

    assert len(gateway.calls) == 1
    assert result.source == "model"
    assert result.wait is not None
    assert result.wait.state == "interrupted"
    assert result.wait.companion_emitted is False


def test_started_wait_is_serialized_on_the_real_respond_wire(route_client) -> None:
    client, gateway = route_client
    response = client.post(
        "/v1/respond",
        json=request_payload(text="Start the blueprint build.", wait=wait_payload("started")),
    )

    assert response.status_code == 200
    assert gateway.calls == []
    body = response.json()
    assert body["source"] == "wait_companion"
    assert "11 minutes" in body["text"]
    assert body["beats"]
    assert body["wait"] == {
        "state": "waiting",
        "event": "started",
        "companion_emitted": True,
        "interruptible": True,
        "estimate_source": "caller_budget",
        "estimated_duration_ms": 660_000,
        "next_check_in_ms": None,
        "silence_budget_ms": 5_000,
        "elapsed_since_activity_ms": 0,
    }


def test_wait_turns_rotate_without_repeating_in_one_serialized_session(route_client) -> None:
    client, _ = route_client
    first = client.post(
        "/v1/respond",
        json=request_payload(text="Start the work.", wait=wait_payload("started")),
    ).json()
    second = client.post(
        "/v1/respond",
        json=request_payload(
            text="The work is still running.",
            wait=wait_payload("companion_due", elapsed_ms=4_000),
        ),
    ).json()

    assert first["text"] != second["text"]
    assert second["source"] == "wait_companion"
    assert second["wait"]["event"] == "companion_due"


def test_user_interrupt_wins_on_the_serialized_route(route_client) -> None:
    client, gateway = route_client
    response = client.post(
        "/v1/respond",
        json=request_payload(
            text="Wait, the Friday report is the urgent one.",
            wait=wait_payload("user_interrupt", elapsed_ms=5_000),
        ),
    )

    assert response.status_code == 200
    assert len(gateway.calls) == 1
    body = response.json()
    assert body["source"] == "model"
    assert body["wait"]["state"] == "interrupted"
    assert body["wait"]["companion_emitted"] is False


def test_completion_wins_on_the_serialized_route(route_client) -> None:
    client, gateway = route_client
    response = client.post(
        "/v1/respond",
        json=request_payload(
            text="The blueprint is ready now.",
            wait=wait_payload("completed", elapsed_ms=4_000),
        ),
    )

    assert response.status_code == 200
    assert len(gateway.calls) == 1
    body = response.json()
    assert body["source"] == "model"
    assert body["wait"]["state"] == "completed"
    assert body["wait"]["companion_emitted"] is False


def test_route_surfaces_silence_breach_as_503_not_a_degraded_reply(route_client) -> None:
    client, gateway = route_client
    response = client.post(
        "/v1/respond",
        json=request_payload(
            text="The work is still running.",
            wait=wait_payload("companion_due", elapsed_ms=6_000),
        ),
    )

    assert response.status_code == 503
    assert gateway.calls == []
    assert response.json()["detail"] == {
        "code": "WaitSilenceBudgetExceeded",
        "message": "wait silence budget exceeded: 6000ms > 5000ms",
        "elapsed_ms": 6_000,
        "budget_ms": 5_000,
    }


def test_bare_refusal_is_acknowledged_without_a_model_reoffer(route_client) -> None:
    client, gateway = route_client
    response = client.post(
        "/v1/respond",
        json={
            "tenant_id": "wait-tenant",
            "user_id": "wait-user",
            "session_id": "refusal-s1",
            "mode": "converse",
            "text": "No, not that one.",
        },
    )

    assert response.status_code == 200
    assert gateway.calls == []
    body = response.json()
    assert body["source"] == "deterministic_control"
    assert body["degraded"] is False
    assert "won't push" in body["text"]
