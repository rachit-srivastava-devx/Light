"""F05 T1/T2/T3/T4/T6 -- the information-gain stopping rule, at the pristine state, at its exact
freeze point, at its exact stall-and-escape point, on the never-answered branch, and its guards
(lane contract §5).

Every expected numeric value below is transcribed VERBATIM from the lane contract's own §5 tables
(T1-T4, T6, T9) -- computed by the lead, independently of this implementation, from §4.1's formulas
and `belief.py`'s published update law. **If an assertion here disagrees with the implementation,
that is a bug to fix in the implementation (or an escalation to the lead), never a reason to edit
the expected value.**

Written by this lane's builder (no test file existed for F05 before this build -- lane contract
§5's header: "write these FIRST; they are the spec"). Frozen once green.
"""

from __future__ import annotations

import ast
import subprocess
import typing
from pathlib import Path

import pytest
from pydantic import BaseModel

from orb_relay.build import freeze_protocol
from orb_relay.build.freeze_protocol import (
    AskKind,
    Move,
    best_question,
    coverage_ok,
    escalation_armed,
    expected_information_gain,
    freeze_eligible,
    keep_asking,
    next_move,
    residual_ambiguity,
    slot_uncertainty,
)
from orb_relay.cognitive.belief import (
    CoverageSlot,
    Evidence,
    EvidenceTier,
    Register,
    apply_evidence,
    fold,
    initial_registers,
)
from orb_relay.proxy.lld_schemas import ModuleBrief

_RELAY_PY_ROOT = Path(__file__).resolve().parents[1]
_WORKTREE_ROOT = _RELAY_PY_ROOT.parents[2]

_WEIGHT = 1.2  # F04's own _TURN_EVIDENCE_WEIGHT
_RELIABILITY = 1.0


def _apply_n(registers, slot: CoverageSlot, n: int, *, weight: float = _WEIGHT):
    for i in range(n):
        registers = apply_evidence(
            registers,
            Evidence(register=Register.COVERAGE, slot=slot, weight=weight, reliability=_RELIABILITY, tier=EvidenceTier.TIER0),
            seq=i,
        )
    return registers


def _state(counts: dict[CoverageSlot, int]):
    """`initial_registers()` with each named slot moved by `counts[slot]` applications of F04's
    standard positive Tier-0 evidence. Every slot not named stays at its prior."""
    registers = initial_registers()
    seq = 0
    for slot, n in counts.items():
        for _ in range(n):
            registers = apply_evidence(
                registers,
                Evidence(register=Register.COVERAGE, slot=slot, weight=_WEIGHT, reliability=_RELIABILITY, tier=EvidenceTier.TIER0),
                seq=seq,
            )
            seq += 1
    return registers


# -------------------------------------------------------------------------------------------
# T1 -- the stopping rule at the pristine state
# -------------------------------------------------------------------------------------------


def test_t1_1_residual_ambiguity_at_pristine_is_exactly_one() -> None:
    assert residual_ambiguity(initial_registers()) == 1.0


def test_t1_2_slot_weight_sums_to_exactly_four() -> None:
    assert sum(freeze_protocol.SLOT_WEIGHT.values()) == 4.0
    assert freeze_protocol.SUM_SLOT_WEIGHT == 4.0


def test_t1_3_pristine_eig_per_slot() -> None:
    pristine = initial_registers()
    expected = {
        CoverageSlot.INTERFACE: 0.125,
        CoverageSlot.ACCEPTANCE: 0.125,
        CoverageSlot.REGISTRY_VERDICT: 0.135,
        CoverageSlot.DEPS: 0.12,
        CoverageSlot.DATA_OWNED: 0.075,
        CoverageSlot.NON_GOALS: 0.04,
    }
    for slot, expected_eig in expected.items():
        kind = freeze_protocol.ask_kind_for(slot, escalated=False)
        assert expected_information_gain(pristine, slot, kind) == pytest.approx(expected_eig, abs=1e-12)


def test_t1_4_best_question_at_pristine_is_the_c1_registry_question() -> None:
    """The C1-first property (lane contract §3.2's emergent property): the argmax's first question
    matches Company-OS C1/L2's own "registry first, before any build" law, purely because
    registry_verdict is the one slot with a closed option set. A weight change that loses this must
    fail here.
    """
    candidate = best_question(initial_registers(), escalated=False)
    assert candidate.slot is CoverageSlot.REGISTRY_VERDICT
    assert candidate.kind is AskKind.CHOOSE
    assert candidate.eig == 0.135


def test_t1_5_tie_break_is_declaration_order_not_dict_luck() -> None:
    """interface and acceptance both score 0.125 at pristine. Drive registry_verdict's confidence
    to 1.0 (removing it from contention) and confirm the tie resolves to INTERFACE -- earlier in
    CoverageSlot's declaration order than ACCEPTANCE -- not by insertion-order luck of a dict/set.
    """
    registers = _state({CoverageSlot.REGISTRY_VERDICT: 3})  # confidence -> 1.0, per the T2 trajectory
    assert registers[(Register.COVERAGE, CoverageSlot.REGISTRY_VERDICT)].confidence == 1.0
    candidate = best_question(registers, escalated=False)
    assert candidate.slot is CoverageSlot.INTERFACE
    assert candidate.eig == pytest.approx(0.125, abs=1e-12)


_SLOT_TO_MODULE_BRIEF_FIELD = {
    CoverageSlot.INTERFACE: "interface",
    CoverageSlot.DATA_OWNED: "data_owned",
    CoverageSlot.ACCEPTANCE: "acceptance",
    CoverageSlot.DEPS: "deps",
    CoverageSlot.NON_GOALS: "non_goals",
}


def _is_compositional_annotation(annotation: object) -> bool:
    origin = typing.get_origin(annotation)
    if origin is list:
        (element_type,) = typing.get_args(annotation)
        return isinstance(element_type, type) and issubclass(element_type, BaseModel)
    return isinstance(annotation, type) and issubclass(annotation, BaseModel)


def test_t1_6_compositional_slots_matches_the_module_brief_schema_partition() -> None:
    """Lane contract §4.1c/T1.6: compositional iff the ModuleBrief field is `list[<BaseModel>]` or
    is itself a BaseModel; atomic iff `list[str]`. Introspected against the real schema so a future
    schema change that adds/renames a field cannot leave `COMPOSITIONAL_SLOTS` silently stale.
    """
    for slot, field_name in _SLOT_TO_MODULE_BRIEF_FIELD.items():
        annotation = ModuleBrief.model_fields[field_name].annotation
        expected_compositional = _is_compositional_annotation(annotation)
        assert (slot in freeze_protocol.COMPOSITIONAL_SLOTS) == expected_compositional, (
            f"{slot} (field {field_name!r}, annotation {annotation!r}) partition mismatch"
        )
    # registry_verdict is neither -- it is the one CLOSED_SET slot (a discriminated union).
    assert CoverageSlot.REGISTRY_VERDICT not in freeze_protocol.COMPOSITIONAL_SLOTS
    assert CoverageSlot.REGISTRY_VERDICT in freeze_protocol.CLOSED_SET_SLOTS


def test_t1_7_keep_asking_true_at_pristine() -> None:
    assert keep_asking(initial_registers()) is True


def test_t1_8_slot_uncertainty_edge_cases_and_a_named_value() -> None:
    assert slot_uncertainty(0.5) == 1.0
    assert slot_uncertainty(1.0) == 0.0
    assert slot_uncertainty(0.0) == 0.0
    assert slot_uncertainty(0.973403006423134) == pytest.approx(0.17702772704896477, abs=1e-12)


# -------------------------------------------------------------------------------------------
# T2 -- convergence: the freeze branch
# -------------------------------------------------------------------------------------------

_TRAJECTORY = [
    (0.7685247834990175, 0.4, 0.780574086303188),
    (0.9168273035060777, 0.8, 0.4132608943283298),
    (0.973403006423134, 1.0, 0.17702772704896477),
    (0.9918374288468401, 1.0, 0.06834971013683284),
]


def test_t2_1_the_trajectory_oracle_itself() -> None:
    """These four rows are the oracle every other T2/T3/T4 state is built from. If `belief.py`'s
    update law ever shifts, THIS case is the one that turns red, not a mysterious failure three
    tests later.
    """
    registers = initial_registers()
    for i, (expected_value, expected_confidence, expected_u) in enumerate(_TRAJECTORY):
        registers = apply_evidence(
            registers,
            Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=_WEIGHT, reliability=_RELIABILITY, tier=EvidenceTier.TIER0),
            seq=i,
        )
        state = registers[(Register.COVERAGE, CoverageSlot.INTERFACE)]
        assert state.value == pytest.approx(expected_value, abs=1e-12)
        assert state.confidence == pytest.approx(expected_confidence, abs=1e-12)
        assert slot_uncertainty(state.value) == pytest.approx(expected_u, abs=1e-12)


def test_t2_2_the_exact_freeze_point_at_two_applications() -> None:
    registers = _state({slot: 2 for slot in freeze_protocol.REQUIRED_SLOTS})
    assert residual_ambiguity(registers) == pytest.approx(0.12851956992238264, abs=1e-12)
    assert residual_ambiguity(registers) <= freeze_protocol.THETA_AMB
    assert coverage_ok(registers) is True  # every required c_s == 0.8, exactly on the >= boundary

    expected_eig = {
        CoverageSlot.INTERFACE: 0.010331522358208242,
        CoverageSlot.ACCEPTANCE: 0.010331522358208242,
        CoverageSlot.REGISTRY_VERDICT: 0.011158044146864903,
        CoverageSlot.DEPS: 0.009918261463879913,
        CoverageSlot.DATA_OWNED: 0.0061989134149249454,
        CoverageSlot.NON_GOALS: 0.04000000000000001,
    }
    for slot, expected in expected_eig.items():
        kind = freeze_protocol.ask_kind_for(slot, escalated=False)
        assert expected_information_gain(registers, slot, kind) == pytest.approx(expected, abs=1e-12)

    assert keep_asking(registers) is False  # max is non_goals at 0.04 < 0.05

    verdict_depth_true = freeze_eligible(registers, depth_pass=True)
    assert verdict_depth_true.eligible is True
    assert verdict_depth_true.blocking == ()

    verdict_depth_false = freeze_eligible(registers, depth_pass=False)
    assert verdict_depth_false.eligible is False
    assert any(entry.startswith("DEPTH") for entry in verdict_depth_false.blocking)


def test_t2_3_three_evidences_residual_ambiguity_is_the_pure_non_goals_floor() -> None:
    registers = _state({slot: 3 for slot in freeze_protocol.REQUIRED_SLOTS})
    assert residual_ambiguity(registers) == 0.05
    assert best_question(registers, escalated=False).eig == pytest.approx(0.04000000000000001, abs=1e-12)


def test_t2_4_the_non_goals_floor_is_structural_not_a_literal() -> None:
    """Derive the 0.05 floor from the weights (a weight change moves this assertion with it, per
    T9's M3), and confirm `best_question` never lands on NON_GOALS while any required slot is
    genuinely uncovered -- checked against the state's OWN `coverage_ok`, not a modified copy.
    """
    expected_floor = freeze_protocol.SLOT_WEIGHT[CoverageSlot.NON_GOALS] / freeze_protocol.SUM_SLOT_WEIGHT
    assert expected_floor == pytest.approx(0.05, abs=1e-12)

    for counts in (
        {},
        {slot: 1 for slot in freeze_protocol.REQUIRED_SLOTS},
        {slot: 2 for slot in freeze_protocol.REQUIRED_SLOTS},
        {slot: 3 for slot in freeze_protocol.REQUIRED_SLOTS},
    ):
        registers = _state(counts)
        assert residual_ambiguity(registers) >= expected_floor
        if not coverage_ok(registers):
            assert best_question(registers, escalated=False).slot is not CoverageSlot.NON_GOALS


def test_t2_5_non_goals_never_blocks_a_freeze() -> None:
    registers = _state({slot: 3 for slot in freeze_protocol.REQUIRED_SLOTS})
    assert freeze_eligible(registers, depth_pass=True).eligible is True


# -------------------------------------------------------------------------------------------
# T3 -- the stall, and the escape (the regression pin for lane contract §3.1)
# -------------------------------------------------------------------------------------------


def _stall_state():
    return _state(
        {
            CoverageSlot.DATA_OWNED: 1,
            CoverageSlot.INTERFACE: 3,
            CoverageSlot.ACCEPTANCE: 3,
            CoverageSlot.DEPS: 3,
            CoverageSlot.REGISTRY_VERDICT: 3,
        }
    )


def test_t3_1_residual_ambiguity_at_the_stall() -> None:
    registers = _stall_state()
    assert residual_ambiguity(registers) == pytest.approx(0.12025166776728692, abs=1e-12)
    assert residual_ambiguity(registers) <= freeze_protocol.THETA_AMB


def test_t3_2_coverage_not_ok_data_owned_under_theta() -> None:
    assert coverage_ok(_stall_state()) is False


def test_t3_3_keep_asking_is_false_at_the_stall() -> None:
    registers = _stall_state()
    assert keep_asking(registers) is False
    eig_data_owned = expected_information_gain(registers, CoverageSlot.DATA_OWNED, AskKind.ANSWER)
    assert eig_data_owned == pytest.approx(0.035125833883643459, abs=1e-12)


def test_t3_4_escalation_armed_by_the_stall_alone_no_history_needed() -> None:
    registers = _stall_state()
    assert escalation_armed(registers, depth_pass=True, clarify_turns=5, ambiguity_history=[]) is True


def test_t3_5_escalated_best_question_targets_data_owned_confirm() -> None:
    candidate = best_question(_stall_state(), escalated=True)
    assert candidate.slot is CoverageSlot.DATA_OWNED
    assert candidate.kind is AskKind.CONFIRM
    assert candidate.eig == pytest.approx(0.07025166776728692, abs=1e-12)


def test_t3_6_next_move_asks_not_freezes_not_splits() -> None:
    move = next_move(_stall_state(), depth_pass=True, clarify_turns=5, ambiguity_history=[])
    assert isinstance(move, Move.Ask)
    assert move.question.slot is CoverageSlot.DATA_OWNED
    assert move.question.kind is AskKind.CONFIRM
    assert move.question.eig == pytest.approx(0.07025166776728692, abs=1e-12)


def test_t3_7_negative_control_a_keep_asking_gated_loop_would_go_silent_here() -> None:
    """The suite would be worthless without this: prove, in the body of ONE test, that the
    ACTUAL shipped `next_move` disagrees with what a `keep_asking`-gated continue-condition would
    have done at this exact state.
    """
    registers = _stall_state()
    move = next_move(registers, depth_pass=True, clarify_turns=5, ambiguity_history=[])
    assert isinstance(move, Move.Ask)  # the shipped behavior: still asks
    assert keep_asking(registers) is False  # a keep_asking-gated loop would have already stopped


def test_t3_8_the_no_progress_history_trigger_independently() -> None:
    """From a state where `keep_asking` is True (so reason (i), the stall, cannot fire), the
    history-based reason (ii) still arms on three consecutive small deltas and does not on three
    consecutive large ones.
    """
    registers = initial_registers()
    assert keep_asking(registers) is True
    assert escalation_armed(registers, depth_pass=True, clarify_turns=0, ambiguity_history=[0.40, 0.38, 0.37, 0.36]) is True
    assert escalation_armed(registers, depth_pass=True, clarify_turns=0, ambiguity_history=[0.40, 0.30, 0.20, 0.10]) is False


# -------------------------------------------------------------------------------------------
# T4 -- termination: the never-answered branch
# -------------------------------------------------------------------------------------------


def _never_answered_state():
    return _state(
        {
            CoverageSlot.INTERFACE: 3,
            CoverageSlot.DATA_OWNED: 3,
            CoverageSlot.DEPS: 3,
            CoverageSlot.REGISTRY_VERDICT: 3,
        }
    )  # acceptance and non_goals left at their prior


def test_t4_1_residual_ambiguity_is_exactly_point_three() -> None:
    registers = _never_answered_state()
    assert residual_ambiguity(registers) == 0.3
    assert residual_ambiguity(registers) > freeze_protocol.THETA_AMB


def test_t4_2_best_question_targets_the_uncovered_acceptance_slot() -> None:
    registers = _never_answered_state()
    non_escalated = best_question(registers, escalated=False)
    assert non_escalated.slot is CoverageSlot.ACCEPTANCE
    assert non_escalated.eig == 0.125
    escalated = best_question(registers, escalated=True)
    assert escalated.eig == 0.25


def test_t4_3_refusal_names_every_reason_not_just_the_first() -> None:
    verdict = freeze_eligible(_never_answered_state(), depth_pass=True)
    assert verdict.eligible is False
    assert "COVERAGE:acceptance" in verdict.blocking
    assert "AMBIGUITY" in verdict.blocking


def test_t4_4_termination_is_driven_not_assumed() -> None:
    assert freeze_protocol.SPLIT_TRIGGER_TURNS == 12
    registers = _never_answered_state()  # frozen: the user answers nothing that moves any register
    iterations = 0
    turns = 0
    while True:
        iterations += 1
        move = next_move(registers, depth_pass=True, clarify_turns=turns, ambiguity_history=())
        if turns < 12:
            assert isinstance(move, Move.Ask), f"expected Ask at clarify_turns={turns}, got {move!r}"
        else:
            assert isinstance(move, Move.Split), f"expected Split at clarify_turns={turns}, got {move!r}"
            break
        turns += 1
    assert iterations == 13


def test_t4_5_next_move_is_total_over_the_reachable_cross_product() -> None:
    states = [initial_registers()]
    for slot in CoverageSlot:
        for n in (1, 2, 3):
            states.append(_state({slot: n}))

    for registers in states:
        for depth_pass in (True, False):
            for clarify_turns in (0, 11, 12, 13):
                move = next_move(registers, depth_pass=depth_pass, clarify_turns=clarify_turns, ambiguity_history=())
                assert isinstance(move, (Move.Freeze, Move.Split, Move.Ask))


def test_t4_6_coverage_is_a_hard_conjunct_at_the_079_080_boundary() -> None:
    """0.79 vs 0.80: an implementation that used `>` instead of `>=` for the OTHER boundary (T2.2)
    would not be caught here, but one that inverted the comparison direction entirely would be.
    """
    registers = _state({slot: 2 for slot in freeze_protocol.REQUIRED_SLOTS})
    one_slot = freeze_protocol.REQUIRED_SLOTS[0]
    state = registers[(Register.COVERAGE, one_slot)]
    assert state.confidence == 0.8
    lowered = dict(registers)
    lowered[(Register.COVERAGE, one_slot)] = type(state)(value=state.value, confidence=0.79, last_evidence_seq=state.last_evidence_seq)
    assert freeze_eligible(lowered, depth_pass=True).eligible is False


# -------------------------------------------------------------------------------------------
# T6 -- the guards
# -------------------------------------------------------------------------------------------


def test_t6_1_min_slot_value_guard_is_reachable_by_negative_evidence() -> None:
    log = [
        Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=-1.2, reliability=1.0, tier=EvidenceTier.TIER0),
        Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=-1.2, reliability=1.0, tier=EvidenceTier.TIER0),
    ]
    registers = fold(log)
    state = registers[(Register.COVERAGE, CoverageSlot.INTERFACE)]
    assert state.value == pytest.approx(0.08317269649392238, abs=1e-12)
    assert state.confidence == 0.8

    assert coverage_ok(registers) is False  # c_s >= theta_cov but value_s < MIN_SLOT_VALUE
    assert "COVERAGE:interface" in freeze_eligible(registers, depth_pass=True).blocking


def test_t6_2_contradiction_is_hard_and_reachable() -> None:
    from orb_relay.build.build_session import is_contradiction_blocking

    before = _state({slot: 2 for slot in freeze_protocol.REQUIRED_SLOTS})
    assert freeze_eligible(before, depth_pass=True).eligible is True  # T2.2's eligible state

    after = apply_evidence(
        before,
        Evidence(register=Register.CONTRADICTION, slot=None, weight=2.0, reliability=1.0, tier=EvidenceTier.TIER0),
        seq=999,
    )
    assert is_contradiction_blocking(after) is True
    verdict = freeze_eligible(after, depth_pass=True)
    assert verdict.eligible is False
    assert "CONTRADICTION" in verdict.blocking


def test_t6_3_proposal_turn_refuses_a_naked_proposal() -> None:
    from pydantic import ValidationError

    from orb_relay.proxy.lld_schemas import KilledAlt
    from orb_relay.proxy.schemas import Ask, AskKindWire, ProposalTurn

    def killed(n: int) -> list[KilledAlt]:
        return [KilledAlt(option=f"opt-{i}", why_killed="because", revive_trigger="never") for i in range(n)]

    base_kwargs = dict(
        kind="PROPOSE",
        target="some-node",
        proposal="do the thing",
        why="it is the simplest option",
        achieves=["a"],
        pros=["simple"],
        ask=Ask(kind=AskKindWire.ANSWER, question="ok?", options=[], slot=None),
    )

    with pytest.raises(ValidationError):
        ProposalTurn(**base_kwargs, cons=[], killed=killed(2))

    with pytest.raises(ValidationError):
        ProposalTurn(**base_kwargs, cons=["a con"], killed=killed(1))

    # Valid: non-empty cons, >=2 killed alternatives.
    ProposalTurn(**base_kwargs, cons=["a con"], killed=killed(2))


def test_t6_4_the_pushback_no_repeat_guard_reuses_canonical_json() -> None:
    from orb_relay.proxy.lld_schemas import KilledAlt, canonical_json
    from orb_relay.proxy.schemas import Ask, AskKindWire, ProposalTurn

    killed = [
        KilledAlt(option="a", why_killed="x", revive_trigger="y"),
        KilledAlt(option="b", why_killed="x", revive_trigger="y"),
    ]
    ask = Ask(kind=AskKindWire.ANSWER, question="ok?", options=[], slot=None)

    turn_a = ProposalTurn(
        kind="PROPOSE", target="n", proposal="p", why="w", achieves=["a"], pros=["x"], cons=["c"], killed=killed, ask=ask
    )
    # Same content, key order differs on the wire -- model_dump always returns the same key order
    # for the same model, so simulate a differently-ordered dict by round-tripping through a
    # manually reordered plain dict instead of the model itself.
    dump_a = turn_a.model_dump(mode="json")
    reordered = dict(reversed(list(dump_a.items())))
    assert canonical_json(dump_a) == canonical_json(reordered)

    turn_b = turn_a.model_copy(update={"proposal": "a different proposal"})
    assert canonical_json(turn_a.model_dump(mode="json")) != canonical_json(turn_b.model_dump(mode="json"))


def test_t6_5_the_firewall_freeze_protocol_touches_no_io_no_clock_no_process() -> None:
    source = Path(freeze_protocol.__file__).read_text(encoding="utf-8")
    tree = ast.parse(source)
    forbidden_modules = {"fastapi", "subprocess", "gateway_client", "readiness", "time", "datetime"}
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                assert alias.name.split(".")[0] not in forbidden_modules, f"forbidden import: {alias.name}"
        elif isinstance(node, ast.ImportFrom):
            module = node.module or ""
            assert module.split(".")[0] not in forbidden_modules, f"forbidden import: {module}"
            assert not module.startswith("store"), f"forbidden store.* import: {module}"
            for alias in node.names:
                assert alias.name != "readiness"


def test_t6_6_belief_py_and_f04s_own_tests_are_untouched() -> None:
    paths = [
        "orb/backend/relay-py/src/orb_relay/cognitive/belief.py",
        "orb/backend/relay-py/tests/test_f04_belief_registers.py",
        "orb/backend/relay-py/tests/test_f04_build_session_store.py",
    ]
    result = subprocess.run(
        ["git", "diff", "master", "--", *paths],
        cwd=str(_WORKTREE_ROOT),
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.stdout == "", f"expected an empty diff against master for {paths}, got:\n{result.stdout}"
