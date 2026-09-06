from orb_relay.store.conversation_store import MAX_CONVERSATION_TURNS, ConversationStore


def test_recent_turns_preserve_role_order_and_scope(tmp_path) -> None:
    store = ConversationStore(tmp_path / "context.db")
    store.append(
        tenant_id="t1",
        user_id="u1",
        session_id="s1",
        role="user",
        text="I am reading docs",
        created_at=1.0,
    )
    store.append(
        tenant_id="t1",
        user_id="u1",
        session_id="s1",
        role="assistant",
        text="Which part?",
        created_at=2.0,
    )
    store.append(
        tenant_id="t1",
        user_id="u1",
        session_id="s2",
        role="user",
        text="Other session",
        created_at=3.0,
    )

    turns = store.recent(tenant_id="t1", user_id="u1", session_id="s1")

    assert [(turn.role, turn.text) for turn in turns] == [
        ("user", "I am reading docs"),
        ("assistant", "Which part?"),
    ]


def test_turn_history_is_bounded(tmp_path) -> None:
    store = ConversationStore(tmp_path / "context.db")
    for index in range(MAX_CONVERSATION_TURNS + 3):
        store.append(
            tenant_id="t1",
            user_id="u1",
            session_id="s1",
            role="user",
            text=f"turn {index}",
            created_at=float(index),
        )

    turns = store.recent(tenant_id="t1", user_id="u1", session_id="s1")

    assert len(turns) == MAX_CONVERSATION_TURNS
    assert turns[0].text == "turn 3"
    assert turns[-1].text == f"turn {MAX_CONVERSATION_TURNS + 2}"


class TestCarryOverAcrossAppReloads:
    """Context used to die on every app reload.

    History was keyed strictly on `session_id` and the mobile app mints a new one per mount, so a
    reload started from nothing. Reported live: *"it does not have context of what I spoke
    previously the context or continuing the conversation end to end."*

    A fixed session id was tried before and reverted — it resurrected a stale task from an earlier
    sitting. So this is bounded on BOTH sides, and these tests pin both bounds.
    """

    def _store(self, tmp_path):
        return ConversationStore(str(tmp_path / "carry.db"))

    def test_a_reload_seconds_later_continues_the_conversation(self, tmp_path) -> None:
        store = self._store(tmp_path)
        store.append(
            tenant_id="t",
            user_id="u",
            session_id="local-1",
            role="user",
            text="my kitchen is a disaster",
            created_at=1000.0,
        )
        store.append(
            tenant_id="t",
            user_id="u",
            session_id="local-1",
            role="assistant",
            text="that sounds like a lot",
            created_at=1001.0,
        )
        # The app reloads: brand-new session id, five seconds later.
        carried = store.recent(tenant_id="t", user_id="u", session_id="local-2", now=1006.0)
        assert [turn.text for turn in carried] == [
            "my kitchen is a disaster",
            "that sounds like a lot",
        ]

    def test_a_stale_session_is_NOT_resurrected(self, tmp_path) -> None:
        store = self._store(tmp_path)
        store.append(
            tenant_id="t",
            user_id="u",
            session_id="local-1",
            role="user",
            text="the bathroom door",
            created_at=1000.0,
        )
        # Next morning. This is the exact bug the previous fix was reverted for.
        stale = store.recent(
            tenant_id="t", user_id="u", session_id="local-2", now=1000.0 + 12 * 3600
        )
        assert stale == []

    def test_once_the_new_session_speaks_it_is_authoritative(self, tmp_path) -> None:
        store = self._store(tmp_path)
        store.append(
            tenant_id="t",
            user_id="u",
            session_id="local-1",
            role="user",
            text="old topic",
            created_at=1000.0,
        )
        store.append(
            tenant_id="t",
            user_id="u",
            session_id="local-2",
            role="user",
            text="new topic",
            created_at=1005.0,
        )
        turns = store.recent(tenant_id="t", user_id="u", session_id="local-2", now=1006.0)
        # No merging: prior sessions are consulted ONLY while this one is empty.
        assert [turn.text for turn in turns] == ["new topic"]

    def test_omitting_now_preserves_the_previous_behaviour_exactly(self, tmp_path) -> None:
        store = self._store(tmp_path)
        store.append(
            tenant_id="t",
            user_id="u",
            session_id="local-1",
            role="user",
            text="anything",
            created_at=1000.0,
        )
        # Every existing caller passes no `now`, so carry-over must be strictly opt-in.
        assert store.recent(tenant_id="t", user_id="u", session_id="local-2") == []

    def test_carry_over_can_be_disabled_per_call(self, tmp_path) -> None:
        store = self._store(tmp_path)
        store.append(
            tenant_id="t",
            user_id="u",
            session_id="local-1",
            role="user",
            text="anything",
            created_at=1000.0,
        )
        assert (
            store.recent(
                tenant_id="t", user_id="u", session_id="local-2", now=1001.0, carry_over_window_s=0
            )
            == []
        )

    def test_another_user_never_leaks_in(self, tmp_path) -> None:
        store = self._store(tmp_path)
        store.append(
            tenant_id="t",
            user_id="someone-else",
            session_id="their-1",
            role="user",
            text="their private thing",
            created_at=1000.0,
        )
        assert store.recent(tenant_id="t", user_id="u", session_id="local-2", now=1001.0) == []

    def test_another_tenant_never_leaks_in(self, tmp_path) -> None:
        store = self._store(tmp_path)
        store.append(
            tenant_id="other-tenant",
            user_id="u",
            session_id="s",
            role="user",
            text="other tenant data",
            created_at=1000.0,
        )
        assert store.recent(tenant_id="t", user_id="u", session_id="local-2", now=1001.0) == []
