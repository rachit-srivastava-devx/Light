import pytest
from orb_relay.store.freeze_store import FreezeStore

def test_u4_t3_ledger_is_append_only(tmp_path):
    s = FreezeStore(tmp_path / "fz.db")
    s.append_freeze(tenant_id="t", user_id="u", session_id="s", freeze_id="fz-0000000000000001",
                    node_id="n1", version=1, content_hash="a" * 64, supersedes=None,
                    payload="{}", created_at=1.0)
    # There must be no way to mutate or remove a stamped freeze.
    for forbidden in ("update", "delete", "edit", "set_superseded", "prune"):
        assert not hasattr(s, forbidden), f"FreezeStore exposes {forbidden}"
    s.append_freeze(tenant_id="t", user_id="u", session_id="s", freeze_id="fz-0000000000000002",
                    node_id="n1", version=2, content_hash="b" * 64,
                    supersedes="fz-0000000000000001", payload="{}", created_at=2.0)
    rows = s.list_freezes(tenant_id="t", user_id="u", session_id="s")
    assert [r.version for r in rows] == [1, 2]          # v1 survives; supersede is a link, not a delete
    assert rows[1].supersedes == "fz-0000000000000001"

def test_u4_t4_ledger_never_prunes(tmp_path):
    """ConversationStore prunes at 24 turns. The freeze ledger must NOT inherit that."""
    s = FreezeStore(tmp_path / "fz.db")
    for i in range(1, 41):
        s.append_freeze(tenant_id="t", user_id="u", session_id="s", freeze_id=f"fz-{i:016d}",
                        node_id=f"n{i}", version=1, content_hash=f"{i:064d}", supersedes=None,
                        payload="{}", created_at=float(i))
    assert len(s.list_freezes(tenant_id="t", user_id="u", session_id="s")) == 40

@pytest.mark.asyncio
async def test_u4_t5_decomposer_fails_closed_to_clarify_not_a_guessed_brief(monkeypatch):
    """atomize() falls back to FALLBACK_STEP. decompose() must NOT have an equivalent —
    a guessed ModuleBrief costs a whole lane, not 30 seconds."""
    from orb_relay.proxy import lld_decomposer
    class AlwaysGarbage:
        async def complete(self, **_):
            class C: text = "not json at all"; usage = lld_decomposer.UsageDelta()
            return C()
    result = await lld_decomposer.decompose("build a thing", gateway=AlwaysGarbage(), tenant_id="t")
    assert result.kind == "clarify_request"
    assert result.brief is None
    assert result.missing, "a clarify request must name what it needs"
