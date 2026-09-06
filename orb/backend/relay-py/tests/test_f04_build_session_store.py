"""F04 A3 -- BuildSessionStore persistence and restart (lane contract §5 A3).

The builder may not edit this file (lane contract, front matter). If a case looks wrong, escalate.
"""

from __future__ import annotations

from orb_relay.cognitive.belief import CoverageSlot, Evidence, EvidenceTier, Register, fold
from orb_relay.store.build_session_store import BuildSessionStore


def _evidence(slot: CoverageSlot, weight: float = 1.0) -> Evidence:
    return Evidence(register=Register.COVERAGE, slot=slot, weight=weight, reliability=1.0, tier=EvidenceTier.TIER0)


def test_a3_1_append_evidence_returns_a_monotonic_per_session_seq(tmp_path) -> None:
    store = BuildSessionStore(tmp_path / "build.db")

    seq0 = store.append_evidence(tenant_id="t1", user_id="u1", session_id="s1", entry=_evidence(CoverageSlot.INTERFACE))
    seq1 = store.append_evidence(tenant_id="t1", user_id="u1", session_id="s1", entry=_evidence(CoverageSlot.DEPS))
    seq2 = store.append_evidence(tenant_id="t1", user_id="u1", session_id="s1", entry=_evidence(CoverageSlot.ACCEPTANCE))
    assert (seq0, seq1, seq2) == (0, 1, 2)

    # A different session_id starts its own sequence at 0, not continuing s1's.
    other_seq0 = store.append_evidence(tenant_id="t1", user_id="u1", session_id="s2", entry=_evidence(CoverageSlot.INTERFACE))
    assert other_seq0 == 0


def test_a3_2_registers_is_exactly_fold_of_log_not_a_second_implementation(tmp_path) -> None:
    store = BuildSessionStore(tmp_path / "build.db")
    store.append_evidence(tenant_id="t1", user_id="u1", session_id="s1", entry=_evidence(CoverageSlot.INTERFACE))
    store.append_evidence(tenant_id="t1", user_id="u1", session_id="s1", entry=_evidence(CoverageSlot.DEPS, weight=0.7))

    log = store.log(tenant_id="t1", user_id="u1", session_id="s1")
    assert store.registers(tenant_id="t1", user_id="u1", session_id="s1") == fold(log)


def test_a3_3_restart_survival_is_byte_identical(tmp_path) -> None:
    db_path = tmp_path / "build.db"
    store = BuildSessionStore(db_path)
    slots = [
        CoverageSlot.INTERFACE,
        CoverageSlot.DATA_OWNED,
        CoverageSlot.ACCEPTANCE,
        CoverageSlot.DEPS,
        CoverageSlot.NON_GOALS,
    ]
    for i, slot in enumerate(slots):
        store.append_evidence(
            tenant_id="t1", user_id="u1", session_id="s1", entry=_evidence(slot, weight=0.3 + i * 0.1)
        )
    before_restart = store.registers(tenant_id="t1", user_id="u1", session_id="s1")

    # A brand-new BuildSessionStore against the SAME file path -- what a relay restart is,
    # in-process.
    restarted_store = BuildSessionStore(db_path)
    after_restart = restarted_store.registers(tenant_id="t1", user_id="u1", session_id="s1")

    assert after_restart == before_restart


def test_a3_4_tenant_isolation(tmp_path) -> None:
    store = BuildSessionStore(tmp_path / "build.db")
    store.append_evidence(
        tenant_id="tenant-a", user_id="u1", session_id="shared-session", entry=_evidence(CoverageSlot.INTERFACE, weight=2.0)
    )
    store.append_evidence(
        tenant_id="tenant-b", user_id="u1", session_id="shared-session", entry=_evidence(CoverageSlot.INTERFACE, weight=-2.0)
    )

    log_a = store.log(tenant_id="tenant-a", user_id="u1", session_id="shared-session")
    log_b = store.log(tenant_id="tenant-b", user_id="u1", session_id="shared-session")
    assert len(log_a) == 1
    assert len(log_b) == 1
    assert log_a[0] != log_b[0]  # neither tenant's log contains the other's evidence

    regs_a = store.registers(tenant_id="tenant-a", user_id="u1", session_id="shared-session")
    regs_b = store.registers(tenant_id="tenant-b", user_id="u1", session_id="shared-session")
    assert regs_a[(Register.COVERAGE, CoverageSlot.INTERFACE)] != regs_b[(Register.COVERAGE, CoverageSlot.INTERFACE)]


def test_a3_5_store_is_append_only_structurally(tmp_path) -> None:
    store = BuildSessionStore(tmp_path / "build.db")
    for forbidden in ("update", "delete", "edit", "prune"):
        assert not hasattr(store, forbidden), f"BuildSessionStore exposes {forbidden}"
    set_prefixed = [name for name in dir(store) if name.startswith("set_")]
    assert not set_prefixed, f"BuildSessionStore exposes set_* methods: {set_prefixed}"
