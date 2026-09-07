"""F05 -- the information-gain freeze-protocol stopping rule (blueprint
`Speed-of-Thought-L8-Deep-Dive/02-DIALOGUE-PLANE-BUILD-MODE.md` §3, `docs/lane-contracts/
F05-freeze-protocol.md` §4.1).

Pure module: no I/O, no clock, no model call, no subprocess, no `fastapi`, no `store.*` import.
Same firewall discipline `cognitive/belief.py` earned in F04 -- `tests/test_f05_freeze_protocol.py`'s
T6.5 asserts the absence of those imports by parsing this module's own AST, so there is no code
path from this module to a clock, a process, or a socket.

This is the ONLY build-new piece of F05 (lane contract §1): everywhere else in this lane installs
an existing module unchanged. Every other symbol this file imports (`CoverageSlot`, `Evidence`,
`Registers`, `is_contradiction_blocking`, ...) is F04's, read-only.

Two things this lane exists to get right (lane contract §3):

1. The naive reading of blueprint §3.2 -- "the loop continues iff KEEP_ASKING" -- hangs: a
   mid-weight compositional slot at one evidence application sits with confidence below
   `THETA_COV` (blocking coverage) while every slot's own EIG has already dropped below
   `EPSILON_GAIN` (so nothing looks worth asking either). The actual continue-condition is
   `CONTINUE(B) = NOT FREEZE_ELIGIBLE(B) AND clarify_turns < SPLIT_TRIGGER_TURNS`, and
   `keep_asking` keeps exactly the role blueprint §3.2 gives it: the ambiguity-axis stop, one
   conjunct of `FREEZE_ELIGIBLE`, and the trigger that escalates the ask kind when the stall is
   reached. `clarify_turns` is monotone, incremented once per CLARIFY turn, so `next_move` reaches
   `Move.Split` at turn `SPLIT_TRIGGER_TURNS` on every path -- the loop is unrepresentable past that
   point, not merely checked.
2. `non_goals` is in `S` (it contributes to `residual_ambiguity` and to `best_question`'s
   candidate set) but NOT in `REQUIRED_SLOTS` (it is not a coverage conjunct) -- forced by
   arithmetic (`SLOT_WEIGHT[NON_GOALS] / SUM_SLOT_WEIGHT * RHO_ANSWER_ATOMIC == 0.04 < EPSILON_GAIN`
   is `non_goals`'s own EIG *maximum*, so requiring it would deadlock every freeze forever), not by
   preference; and matches the gate's own 14 checks, none of which name `non_goals`.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from enum import Enum
from math import log2

from ..cognitive.belief import CoverageSlot, Register, Registers
from .build_session import is_contradiction_blocking

# ---- Constants. Every one carries its source; [A] = assumed, blueprint's own label. -------------

EPSILON_GAIN = 0.05
"""blueprint §3.2 [A] -- the info-gain floor. Below this, a question is "not worth the breath"."""

THETA_COV = 0.80
"""blueprint §3.2 [A] -- per-slot coverage bar. With F04's kappa=0.4, r=1.0: confidence goes
0.4 -> 0.8 -> 1.0, so >= 0.80 is exactly "this slot has been evidenced at least twice"."""

THETA_AMB = 0.15
"""blueprint §3.2 [A] -- residual-ambiguity ceiling."""

SPLIT_TRIGGER_TURNS = 12
"""blueprint §3.4 [A] -- the old ClarifyProtocol.ts cap, re-aimed to a split signal (lane contract
§3.1's termination proof: CONTINUE is false whenever clarify_turns >= this)."""

NO_PROGRESS_TURNS = 3
"""blueprint §3.4 [A] -- N consecutive ambiguity deltas below EPSILON_GAIN arms the escalation
escape (the second, independent trigger -- lane contract §3.1's stall is the first)."""

MIN_SLOT_VALUE = 0.5
"""Lane contract §4.1a -- NOT in the blueprint. Under the register's own reading (§2.5), a slot
driven by *negative* evidence can reach confidence >= THETA_COV while `value` sits well below the
prior -- the register would be "confident the slot is not filled". No negative-weight evidence
producer exists in this tree today (F04 emits only +1.2 / +0.5), so this guard is unreachable in
production; it is closed structurally anyway, at the register's own prior, so it never rejects a
state reachable by positive evidence alone."""

SLOT_WEIGHT: Mapping[CoverageSlot, float] = {
    CoverageSlot.INTERFACE: 1.0,
    CoverageSlot.ACCEPTANCE: 1.0,
    CoverageSlot.DATA_OWNED: 0.6,
    CoverageSlot.DEPS: 0.6,
    CoverageSlot.REGISTRY_VERDICT: 0.6,
    CoverageSlot.NON_GOALS: 0.2,
}
"""Lane contract §4.1b -- "how much a wrong guess here costs a lane", derived from where each
slot's error is caught (acceptance/interface: nowhere upstream / at integration; data_owned/deps/
registry_verdict: mechanically, by keel; non_goals: only at human PR review)."""

SUM_SLOT_WEIGHT = 4.0
"""Exact, not coincidental (lane contract §4.1b): makes every EIG at the pristine state a short
terminating decimal, so the acceptance suite's oracle can be read and checked by a reviewer."""

REQUIRED_SLOTS: tuple[CoverageSlot, ...] = tuple(slot for slot in CoverageSlot if slot is not CoverageSlot.NON_GOALS)
"""The five of lane contract §3.2 -- CoverageSlot declaration order, minus NON_GOALS. Forced by
arithmetic (see the module docstring's point 2), not preference."""

COMPOSITIONAL_SLOTS: frozenset[CoverageSlot] = frozenset(
    {CoverageSlot.INTERFACE, CoverageSlot.DATA_OWNED, CoverageSlot.ACCEPTANCE}
)
"""Lane contract §4.1c -- read off the real ModuleBrief schema, not chosen: a slot is compositional
iff its field is a list of BaseModel objects or is itself a multi-field BaseModel record."""

CLOSED_SET_SLOTS: frozenset[CoverageSlot] = frozenset({CoverageSlot.REGISTRY_VERDICT})
"""The one slot whose ModuleBrief field is a discriminated union over closed `kind` values."""

RHO_ANSWER_COMPOSITIONAL = 0.5
"""blueprint §3.2: "a compositional slot resolves less [per answer]"."""

RHO_ANSWER_ATOMIC = 0.8
"""blueprint §3.2: "a direct self-report ... rho ~= 0.8"."""

RHO_CHOOSE = 0.9
"""Lane contract §4.1 [A] -- between a self-report and a binary confirm."""

RHO_CONFIRM = 1.0
"""blueprint §3.4: "maximally resolving (rho -> 1)"."""


class AskKind(str, Enum):
    ANSWER = "answer"
    CHOOSE = "choose"
    CONFIRM = "confirm"


def slot_uncertainty(value: float) -> float:
    """u_s = binary entropy H2(value) = -v*log2(v) - (1-v)*log2(1-v); 0.0 at v in {0,1} (explicit,
    not merely "happens to work" -- log2(0) raises).
    """
    if value <= 0.0 or value >= 1.0:
        return 0.0
    return -(value * log2(value) + (1.0 - value) * log2(1.0 - value))


def ask_kind_for(slot: CoverageSlot, *, escalated: bool) -> AskKind:
    """CONFIRM if escalated; else CHOOSE for a closed-set slot; else ANSWER. Derived from the
    ModuleBrief schema's own shape, never from taste (lane contract §4.1c).
    """
    if escalated:
        return AskKind.CONFIRM
    if slot in CLOSED_SET_SLOTS:
        return AskKind.CHOOSE
    return AskKind.ANSWER


def resolving_power(slot: CoverageSlot, kind: AskKind) -> float:
    if kind is AskKind.CONFIRM:
        return RHO_CONFIRM
    if kind is AskKind.CHOOSE:
        return RHO_CHOOSE
    return RHO_ANSWER_COMPOSITIONAL if slot in COMPOSITIONAL_SLOTS else RHO_ANSWER_ATOMIC


def _coverage_state(registers: Registers, slot: CoverageSlot):
    return registers[(Register.COVERAGE, slot)]


def residual_ambiguity(registers: Registers) -> float:
    """H(B) = sum_{s in S} w_s * (1 - c_s) * u_s / sum(w) -- over ALL SIX slots (blueprint §3.2),
    including NON_GOALS, which is why an unfilled non-goal keeps a permanent 0.05 floor on this
    value without ever blocking a freeze (lane contract §3.2/T2.4).
    """
    total = 0.0
    for slot in CoverageSlot:
        state = _coverage_state(registers, slot)
        total += SLOT_WEIGHT[slot] * (1.0 - state.confidence) * slot_uncertainty(state.value)
    return total / SUM_SLOT_WEIGHT


def expected_information_gain(registers: Registers, slot: CoverageSlot, kind: AskKind) -> float:
    """EIG = (w_s / sum(w)) * (1 - c_s) * u_s * rho(s, kind)."""
    state = _coverage_state(registers, slot)
    return (SLOT_WEIGHT[slot] / SUM_SLOT_WEIGHT) * (1.0 - state.confidence) * slot_uncertainty(state.value) * resolving_power(slot, kind)


@dataclass(frozen=True, slots=True)
class QuestionCandidate:
    slot: CoverageSlot
    kind: AskKind
    eig: float


def best_question(registers: Registers, *, escalated: bool) -> QuestionCandidate:
    """argmax_s EIG, over every CoverageSlot (including NON_GOALS -- it can still be the argmax
    once every other slot is fully covered, T2.3). Ties broken by CoverageSlot declaration order:
    `max()` over a list built in that order keeps the FIRST maximal element (it only replaces the
    incumbent on a STRICTLY greater score), so this is deterministic without a second comparison
    key (lane contract T1.5).
    """
    candidates = [
        QuestionCandidate(slot=slot, kind=(kind := ask_kind_for(slot, escalated=escalated)), eig=expected_information_gain(registers, slot, kind))
        for slot in CoverageSlot
    ]
    return max(candidates, key=lambda c: c.eig)


def keep_asking(registers: Registers) -> bool:
    """Blueprint §3.2's AMBIGUITY-AXIS stop ONLY: max_s EIG(q_s, non-escalated) >= EPSILON_GAIN.
    This is NOT the loop's continue-condition -- see the module docstring's point 1 and
    `continue_dialogue` below.
    """
    return best_question(registers, escalated=False).eig >= EPSILON_GAIN


def coverage_ok(registers: Registers) -> bool:
    """ALL s in REQUIRED_SLOTS: c_s >= THETA_COV AND value_s >= MIN_SLOT_VALUE (§4.1a's guard)."""
    return all(
        _coverage_state(registers, slot).confidence >= THETA_COV and _coverage_state(registers, slot).value >= MIN_SLOT_VALUE
        for slot in REQUIRED_SLOTS
    )


@dataclass(frozen=True, slots=True)
class EligibilityVerdict:
    eligible: bool
    coverage_ok: bool
    ambiguity_ok: bool
    contradiction_clear: bool
    depth_pass: bool
    info_gain_exhausted: bool
    residual_ambiguity: float
    best_question: QuestionCandidate
    blocking: tuple[str, ...]


def freeze_eligible(registers: Registers, *, depth_pass: bool) -> EligibilityVerdict:
    """Blueprint §3.2's FREEZE_ELIGIBLE: all five conjuncts, each reported separately so a refusal
    always names every reason it is refusing, not just the first one found (lane contract T4.3).
    """
    cov_ok = coverage_ok(registers)
    amb = residual_ambiguity(registers)
    amb_ok = amb <= THETA_AMB
    contradiction_clear = not is_contradiction_blocking(registers)
    info_gain_exhausted = not keep_asking(registers)
    eligible = cov_ok and amb_ok and contradiction_clear and depth_pass and info_gain_exhausted

    blocking: list[str] = []
    if not cov_ok:
        for slot in REQUIRED_SLOTS:
            state = _coverage_state(registers, slot)
            if state.confidence < THETA_COV or state.value < MIN_SLOT_VALUE:
                blocking.append(f"COVERAGE:{slot.value}")
    if not amb_ok:
        blocking.append("AMBIGUITY")
    if not contradiction_clear:
        blocking.append("CONTRADICTION")
    if not depth_pass:
        blocking.append("DEPTH")
    if not info_gain_exhausted:
        blocking.append("INFO_GAIN")

    return EligibilityVerdict(
        eligible=eligible,
        coverage_ok=cov_ok,
        ambiguity_ok=amb_ok,
        contradiction_clear=contradiction_clear,
        depth_pass=depth_pass,
        info_gain_exhausted=info_gain_exhausted,
        residual_ambiguity=amb,
        best_question=best_question(registers, escalated=False),
        blocking=tuple(blocking),
    )


def continue_dialogue(registers: Registers, *, depth_pass: bool, clarify_turns: int) -> bool:
    """NOT freeze_eligible(...).eligible AND clarify_turns < SPLIT_TRIGGER_TURNS (lane contract
    §3.1's proof -- this, not `keep_asking`, is the loop's actual continue-condition).
    """
    return (not freeze_eligible(registers, depth_pass=depth_pass).eligible) and clarify_turns < SPLIT_TRIGGER_TURNS


def _ambiguity_history_stalled(ambiguity_history: Sequence[float]) -> bool:
    if len(ambiguity_history) < NO_PROGRESS_TURNS + 1:
        return False
    deltas = [ambiguity_history[i] - ambiguity_history[i + 1] for i in range(len(ambiguity_history) - 1)]
    last = deltas[-NO_PROGRESS_TURNS:]
    return len(last) == NO_PROGRESS_TURNS and all(delta < EPSILON_GAIN for delta in last)


def escalation_armed(
    registers: Registers,
    *,
    depth_pass: bool,
    clarify_turns: int,
    ambiguity_history: Sequence[float],
) -> bool:
    """True when blueprint §3.4's escape should fire, for EITHER of two independent reasons:

    (i)  the lane contract §3.1 stall: the dialogue would otherwise continue (not yet eligible to
         freeze, not yet at the split ceiling) but nothing on the ambiguity axis looks worth
         asking -- escalating to a `confirm` ask on the best remaining slot doubles its resolving
         power and can push its EIG back over EPSILON_GAIN.
    (ii) the last NO_PROGRESS_TURNS deltas of `ambiguity_history` (oldest-first) are each below
         EPSILON_GAIN -- three turns in a row that moved almost nothing.
    """
    stalled = continue_dialogue(registers, depth_pass=depth_pass, clarify_turns=clarify_turns) and not keep_asking(registers)
    return stalled or _ambiguity_history_stalled(ambiguity_history)


class Move:
    """The tagged union `next_move` returns. Three nested, frozen, immutable variants -- not a
    single dataclass with optional fields -- so `isinstance(move, Move.Ask)` is the exhaustiveness
    check itself (lane contract T4.5).
    """

    @dataclass(frozen=True, slots=True)
    class Freeze:
        pass

    @dataclass(frozen=True, slots=True)
    class Split:
        reason: str

    @dataclass(frozen=True, slots=True)
    class Ask:
        question: QuestionCandidate
        escalated: bool


MoveT = Move.Freeze | Move.Split | Move.Ask


def next_move(
    registers: Registers,
    *,
    depth_pass: bool,
    clarify_turns: int,
    ambiguity_history: Sequence[float],
) -> MoveT:
    """The one entry point app.py calls. Checks FREEZE first (so an eligible state freezes on the
    turn it becomes eligible, even if `clarify_turns` has also reached the split ceiling on the
    same turn), then SPLIT, then falls through to ASK -- total over every reachable input (lane
    contract T4.5).
    """
    verdict = freeze_eligible(registers, depth_pass=depth_pass)
    if verdict.eligible:
        return Move.Freeze()
    if clarify_turns >= SPLIT_TRIGGER_TURNS:
        return Move.Split(reason=f"clarify_turns={clarify_turns} >= SPLIT_TRIGGER_TURNS={SPLIT_TRIGGER_TURNS}")
    escalated = escalation_armed(registers, depth_pass=depth_pass, clarify_turns=clarify_turns, ambiguity_history=ambiguity_history)
    candidate = best_question(registers, escalated=escalated)
    return Move.Ask(question=candidate, escalated=escalated)
