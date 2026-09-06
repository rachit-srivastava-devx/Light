"""F04 A2 -- the belief-register update law, exact values. A5 -- the firewall, by AST inspection.

Pure-function tests: no HTTP, no store. The expected values below were computed FOR the lane
contract (`docs/lane-contracts/F04-server-side-build-mode.md` §5 A2), not copied from this
implementation -- they are the oracle, so a wrong implementation cannot define them.

The builder may not edit this file (lane contract, front matter). If a case looks wrong, escalate.
"""

from __future__ import annotations

import ast
import inspect
import math

import pytest

from orb_relay.cognitive import belief
from orb_relay.cognitive.belief import (
    CoverageSlot,
    Evidence,
    EvidenceTier,
    PremiseRevised,
    Register,
    fold,
    initial_registers,
    invalidate,
)
from orb_relay.cognitive.belief import apply_evidence as _apply_evidence

_ABS = 1e-12


def _logit_of(value: float) -> float:
    """Independent re-derivation of logit from value, NOT a call into the implementation's own
    (private) logit helper -- so this is a check on the observable output, not a tautology.
    """
    return math.log(value / (1 - value))


def test_a2_1_scripted_evidence_sequence_on_coverage_interface() -> None:
    key = (Register.COVERAGE, CoverageSlot.INTERFACE)
    regs = initial_registers()

    regs = _apply_evidence(
        regs,
        Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=1.2, reliability=1.0, tier=EvidenceTier.TIER0),
        seq=0,
    )
    e1 = regs[key]
    assert _logit_of(e1.value) == pytest.approx(1.2, abs=1e-9)
    assert e1.value == pytest.approx(0.768524783499, abs=_ABS)
    assert e1.confidence == pytest.approx(0.4, abs=_ABS)

    regs = _apply_evidence(
        regs,
        Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=1.2, reliability=0.5, tier=EvidenceTier.TIER0),
        seq=1,
    )
    e2 = regs[key]
    assert _logit_of(e2.value) == pytest.approx(1.8, abs=1e-9)
    assert e2.value == pytest.approx(0.858148935100, abs=_ABS)
    assert e2.confidence == pytest.approx(0.6, abs=_ABS)

    # e3 is the Tier-2 clamp: raw delta = 3.0 must be capped to 0.8 (MAX_TIER2_EVIDENCE_LOGITS).
    # If the clamp is missing, logit becomes 4.8 and value 0.991837... -- a visibly different
    # number, so this case genuinely discriminates.
    regs = _apply_evidence(
        regs,
        Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=3.0, reliability=1.0, tier=EvidenceTier.TIER2),
        seq=2,
    )
    e3 = regs[key]
    assert _logit_of(e3.value) == pytest.approx(2.6, abs=1e-9)
    assert e3.value == pytest.approx(0.930861579657, abs=_ABS)
    assert e3.confidence == pytest.approx(1.0, abs=_ABS)  # kappa accumulation 0.4, 0.2, 0.4 -> 1.0


def test_a2_2_saturation_clamps_and_never_produces_nan() -> None:
    key = (Register.COVERAGE, CoverageSlot.INTERFACE)
    regs = initial_registers()
    regs = _apply_evidence(
        regs,
        Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=1.2, reliability=1.0, tier=EvidenceTier.TIER0),
        seq=0,
    )
    regs = _apply_evidence(
        regs,
        Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=1.2, reliability=0.5, tier=EvidenceTier.TIER0),
        seq=1,
    )
    regs = _apply_evidence(
        regs,
        Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=3.0, reliability=1.0, tier=EvidenceTier.TIER2),
        seq=2,
    )
    assert _logit_of(regs[key].value) == pytest.approx(2.6, abs=1e-9)

    for i in range(5):
        regs = _apply_evidence(
            regs,
            Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=2.0, reliability=1.0, tier=EvidenceTier.TIER0),
            seq=3 + i,
        )
    assert _logit_of(regs[key].value) == pytest.approx(6.0, abs=1e-9)
    assert regs[key].value == pytest.approx(0.997527376843, abs=_ABS)

    for i in range(10):
        regs = _apply_evidence(
            regs,
            Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=2.0, reliability=1.0, tier=EvidenceTier.TIER0),
            seq=8 + i,
        )
    assert regs[key].value == pytest.approx(0.997527376843, abs=_ABS)
    assert math.isnan(regs[key].value) is False


def test_a2_3_tier0_and_tier1_evidence_are_not_capped() -> None:
    key = (Register.COVERAGE, CoverageSlot.DEPS)  # fresh, independent register
    regs = initial_registers()
    regs = _apply_evidence(
        regs,
        Evidence(register=Register.COVERAGE, slot=CoverageSlot.DEPS, weight=3.0, reliability=1.0, tier=EvidenceTier.TIER0),
        seq=0,
    )
    # Asymmetry with A2.1's e3 (a Tier-2 delta of the same raw magnitude clamps to 0.8) is the point.
    assert _logit_of(regs[key].value) == pytest.approx(3.0, abs=1e-9)


def test_a2_4_reliability_is_clamped_to_the_0_1_range() -> None:
    key = (Register.AMBIGUITY, None)
    regs = initial_registers()
    prior = regs[key]

    negative = _apply_evidence(
        regs, Evidence(register=Register.AMBIGUITY, slot=None, weight=2.0, reliability=-0.5, tier=EvidenceTier.TIER0), seq=0
    )
    assert negative[key].value == pytest.approx(prior.value, abs=_ABS)
    assert negative[key].confidence == pytest.approx(prior.confidence, abs=_ABS)

    nan_reliability = _apply_evidence(
        regs,
        Evidence(register=Register.AMBIGUITY, slot=None, weight=2.0, reliability=float("nan"), tier=EvidenceTier.TIER0),
        seq=0,
    )
    assert nan_reliability[key].value == pytest.approx(prior.value, abs=_ABS)
    assert nan_reliability[key].confidence == pytest.approx(prior.confidence, abs=_ABS)

    over_one = _apply_evidence(
        regs, Evidence(register=Register.AMBIGUITY, slot=None, weight=1.2, reliability=2.0, tier=EvidenceTier.TIER0), seq=0
    )
    treated_as_one = _apply_evidence(
        regs, Evidence(register=Register.AMBIGUITY, slot=None, weight=1.2, reliability=1.0, tier=EvidenceTier.TIER0), seq=0
    )
    assert over_one[key].value == pytest.approx(treated_as_one[key].value, abs=_ABS)
    assert over_one[key].confidence == pytest.approx(treated_as_one[key].confidence, abs=_ABS)


def test_a2_5_dependency_invalidation_touches_only_named_dependents() -> None:
    acceptance_key = (Register.COVERAGE, CoverageSlot.ACCEPTANCE)
    deps_key = (Register.COVERAGE, CoverageSlot.DEPS)
    regs = initial_registers()

    # Drive Coverage[acceptance] to logit=2.0, confidence=0.8 (two r=1.0 applications: kappa*1.0
    # twice = 0.8; weight 1.0 twice = logit 2.0).
    regs = _apply_evidence(
        regs, Evidence(register=Register.COVERAGE, slot=CoverageSlot.ACCEPTANCE, weight=1.0, reliability=1.0, tier=EvidenceTier.TIER0), seq=0
    )
    regs = _apply_evidence(
        regs, Evidence(register=Register.COVERAGE, slot=CoverageSlot.ACCEPTANCE, weight=1.0, reliability=1.0, tier=EvidenceTier.TIER0), seq=1
    )
    assert _logit_of(regs[acceptance_key].value) == pytest.approx(2.0, abs=1e-9)
    assert regs[acceptance_key].confidence == pytest.approx(0.8, abs=_ABS)

    # Coverage[deps] to any distinct state.
    regs = _apply_evidence(
        regs, Evidence(register=Register.COVERAGE, slot=CoverageSlot.DEPS, weight=0.5, reliability=1.0, tier=EvidenceTier.TIER0), seq=2
    )
    deps_before = regs[deps_key]

    event = PremiseRevised(premise=CoverageSlot.DATA_OWNED, dependents=(CoverageSlot.ACCEPTANCE,))
    regs_after = invalidate(regs, event, seq=3)

    acceptance_after = regs_after[acceptance_key]
    assert acceptance_after.confidence == pytest.approx(0.32, abs=_ABS)  # 0.8 * 0.4
    assert _logit_of(acceptance_after.value) == pytest.approx(0.8, abs=1e-9)
    assert acceptance_after.value == pytest.approx(0.689974481128, abs=_ABS)

    # Invalidation touches only listed dependents -- deps is untouched, exactly.
    assert regs_after[deps_key] == deps_before


def test_a2_6_no_wall_clock_decay_structurally() -> None:
    source = inspect.getsource(belief)
    tree = ast.parse(source)

    forbidden_modules = {"time", "datetime"}
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                assert alias.name.split(".")[0] not in forbidden_modules, f"belief.py imports {alias.name}"
        elif isinstance(node, ast.ImportFrom):
            top_level = (node.module or "").split(".")[0]
            assert top_level not in forbidden_modules, f"belief.py imports from {node.module}"

    forbidden_params = {"now", "timestamp", "ts", "time"}
    checked_any = False
    for name, obj in vars(belief).items():
        if name.startswith("_") or not inspect.isfunction(obj) or obj.__module__ != belief.__name__:
            continue
        checked_any = True
        params = set(inspect.signature(obj).parameters)
        offending = params & forbidden_params
        assert not offending, f"{name} has a clock-shaped parameter: {offending}"
    assert checked_any, "no public function found in cognitive.belief -- test setup is broken"


def test_a2_7_fold_is_replay_deterministic_and_order_sensitive() -> None:
    """40-entry mixed log. Indices 3 and 7 are large, opposite-sign TIER0 deltas on the SAME
    register (Coverage[interface]), sized so swapping them changes whether the +7.0 delta
    saturates MAX_ABS_LOGIT -- two plain additive deltas on the same register are otherwise
    ORDER-INDEPENDENT (addition commutes when nothing saturates), so a naive transposition would
    not actually discriminate order. The clamp is where order starts to matter.
    """
    slots = list(CoverageSlot)
    log: list[Evidence | PremiseRevised] = []
    for i in range(40):
        if i == 3:
            log.append(Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=7.0, reliability=1.0, tier=EvidenceTier.TIER0))
        elif i == 7:
            log.append(Evidence(register=Register.COVERAGE, slot=CoverageSlot.INTERFACE, weight=-3.0, reliability=1.0, tier=EvidenceTier.TIER0))
        elif i % 11 == 10:
            slot = slots[i % len(slots)]
            log.append(PremiseRevised(premise=slot, dependents=(slots[(i + 1) % len(slots)],)))
        else:
            register = Register.COVERAGE if i % 2 == 0 else Register.AMBIGUITY
            slot = slots[i % len(slots)] if register is Register.COVERAGE else None
            weight = ((i % 5) - 2) * 0.3
            reliability = 0.3 + (i % 4) * 0.15
            log.append(Evidence(register=register, slot=slot, weight=weight, reliability=reliability, tier=EvidenceTier.TIER0))

    first = fold(log)
    second = fold(log)
    assert first == second, "fold(log) must be bit-identical on repeated replay of the same log"

    transposed_log = list(log)
    transposed_log[3], transposed_log[7] = transposed_log[7], transposed_log[3]
    transposed = fold(transposed_log)
    assert transposed != first, "swapping two same-register entries must change the result, or this fold ignores order"


def test_a2_8_every_register_and_slot_is_reachable_in_initial_registers() -> None:
    regs = initial_registers()

    seen_registers = {key[0] for key in regs}
    assert seen_registers == set(Register), f"missing registers: {set(Register) - seen_registers}"

    seen_slots = {key[1] for key in regs if key[1] is not None}
    assert seen_slots == set(CoverageSlot), f"missing slots: {set(CoverageSlot) - seen_slots}"

    for state in regs.values():
        assert state.value == pytest.approx(0.5, abs=_ABS)
        assert state.confidence == pytest.approx(0.0, abs=_ABS)
        assert state.last_evidence_seq is None


def test_a5_belief_module_imports_none_of_the_firewalled_modules() -> None:
    """Blueprint 02 §4.1's firewall ("beliefs steer, they never freeze") as a property of the
    import graph, not a comment: `cognitive/belief.py` must import none of `lld_schemas`,
    `freeze_store`, `fastapi`, `gateway_client`, or any `store.*`.
    """
    source = inspect.getsource(belief)
    tree = ast.parse(source)
    forbidden_names = {"lld_schemas", "freeze_store", "fastapi", "gateway_client", "store"}

    violations: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                if forbidden_names & set(alias.name.split(".")):
                    violations.append(alias.name)
        elif isinstance(node, ast.ImportFrom):
            parts = set((node.module or "").split("."))
            if forbidden_names & parts:
                violations.append(node.module or "")
    assert not violations, f"cognitive/belief.py imports a firewalled module: {violations}"
