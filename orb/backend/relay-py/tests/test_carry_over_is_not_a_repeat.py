"""Carried-over history is context, not evidence that the user repeated themselves.

Two features landed independently and collided:

  * cross-session carry-over — a fresh `session_id` with no turns of its own inherits the user's
    most recent prior session, because the mobile app mints a new id on every mount and every
    reload was starting the conversation from nothing;
  * a duplicate-input notice — when the user's message repeats their previous one, a line is
    appended to the system prompt telling the model to acknowledge it.

Together they told a user who opened the app and said their usual opening line that they were
repeating themselves and might not have heard the last reply. Wrong, and unpleasant: the first thing
said in a new session is a first utterance.

How it was actually located, since it presented as an unrelated contract test failing on exact
prompt equality: a control pair. Three fresh sessions for one user produced the notice from turn 2
onward; the identical requests with distinct user ids produced it never. That isolated the cause to
carry-over rather than to the duplicate check.

These live in their own file because another session owns `test_app_routes.py`.
"""

from __future__ import annotations

import pytest
from fastapi.testclient import TestClient
from orb_relay import app as app_module
from orb_relay.app import app, get_gateway
from orb_relay.cost.meter import UsageDelta
from orb_relay.proxy.gateway_client import GatewayCompletion

NOTICE = "repeats their previous message"


class _Inspecting:
    """Captures what the model was actually sent. `last_kwargs` is the only assertion surface —
    what the prompt CONTAINS is the delivered behaviour; what a dev log says about it is not."""

    def __init__(self) -> None:
        self.last_kwargs: dict[str, object] = {}
        self.calls = 0

    async def complete(self, **kwargs: object) -> GatewayCompletion:
        self.last_kwargs = kwargs
        self.calls += 1
        # Distinct replies per call: a fixture that returns one fixed string is itself a verbatim
        # repeater, which has already once made a test assert against its own stub's defect.
        return GatewayCompletion(
            f"Reply number {self.calls}.", UsageDelta(llm_tokens_in=3, llm_tokens_out=4)
        )


@pytest.fixture(autouse=True)
def isolated_stores(tmp_path, monkeypatch) -> None:
    """Never touch the real .data/context.db. Learned the hard way in this very investigation: the
    same probe run as a plain script instead of a pytest test wrote 24 rows into the live dev store,
    which then made the probe's own second run report the opposite result."""
    monkeypatch.setattr(
        app_module,
        "_conversation_store",
        app_module.ConversationStore(tmp_path / "conversation.db"),
    )
    monkeypatch.setattr(
        app_module, "_context_store", app_module.ContextStore(tmp_path / "context.db")
    )


def _say(gateway: _Inspecting, *, session: str, text: str, user: str = "u1") -> str:
    async def override() -> object:
        return gateway

    app.dependency_overrides[get_gateway] = override
    try:
        response = TestClient(app).post(
            "/v1/respond",
            json={
                "session_id": session,
                "tenant_id": "t1",
                "user_id": user,
                "text": text,
                "mode": "converse",
            },
        )
        assert response.status_code == 200, response.text
        return str(gateway.last_kwargs["system"])
    finally:
        app.dependency_overrides.clear()


def test_a_new_session_saying_the_same_thing_is_not_a_repeat() -> None:
    """The defect. Same user, same words, brand-new session id — exactly an app reload."""
    gateway = _Inspecting()
    _say(gateway, session="s-first", text="hello there")
    system = _say(gateway, session="s-second-after-reload", text="hello there")
    assert NOTICE not in system, (
        "a fresh session's first utterance was treated as a repeat — carry-over history was read as "
        "this session's own previous turn"
    )


def test_a_genuine_repeat_within_one_session_still_gets_the_notice() -> None:
    """The guard on the fix: gating the check must not switch the feature off. The owner really does
    repeat themselves when unheard — that was reported twice as 'speaking no reply'."""
    gateway = _Inspecting()
    _say(gateway, session="s-same", text="hello there")
    system = _say(gateway, session="s-same", text="hello there")
    assert NOTICE in system


def test_carried_over_context_still_reaches_the_model() -> None:
    """The fix must not throw away the carry-over itself — that was the whole point of it. The new
    session's request must still carry the earlier turns as history."""
    gateway = _Inspecting()
    _say(gateway, session="s-a", text="my kitchen is a disaster")
    _say(gateway, session="s-b-after-reload", text="yeah exactly")
    messages = gateway.last_kwargs.get("history") or gateway.last_kwargs.get("messages") or []
    flattened = " ".join(
        str(m.get("content", "")) if isinstance(m, dict) else str(m) for m in messages
    )
    assert "kitchen" in flattened, (
        "carry-over stopped delivering prior context; the reload amnesia this was built to fix "
        f"would be back. history={messages!r}"
    )


def test_another_user_is_never_a_repeat_source() -> None:
    """Tenant/user isolation, restated at the route level rather than only in the store."""
    gateway = _Inspecting()
    _say(gateway, session="s-x", text="hello there", user="user-one")
    system = _say(gateway, session="s-y", text="hello there", user="user-two")
    assert NOTICE not in system


class TestHasOwnTurns:
    """The store predicate the gate is built on, tested directly so a route-level pass cannot hide a
    wrong answer here."""

    def test_a_fresh_session_has_no_turns_of_its_own(self, tmp_path) -> None:
        store = app_module.ConversationStore(tmp_path / "c.db")
        assert not store.has_own_turns(tenant_id="t", user_id="u", session_id="fresh")

    def test_after_appending_it_does(self, tmp_path) -> None:
        store = app_module.ConversationStore(tmp_path / "c.db")
        store.append(
            tenant_id="t", user_id="u", session_id="s", role="user", text="hi", created_at=1.0
        )
        assert store.has_own_turns(tenant_id="t", user_id="u", session_id="s")

    def test_it_does_not_leak_across_sessions_users_or_tenants(self, tmp_path) -> None:
        store = app_module.ConversationStore(tmp_path / "c.db")
        store.append(
            tenant_id="t", user_id="u", session_id="s", role="user", text="hi", created_at=1.0
        )
        assert not store.has_own_turns(tenant_id="t", user_id="u", session_id="other")
        assert not store.has_own_turns(tenant_id="t", user_id="other", session_id="s")
        assert not store.has_own_turns(tenant_id="other", user_id="u", session_id="s")
