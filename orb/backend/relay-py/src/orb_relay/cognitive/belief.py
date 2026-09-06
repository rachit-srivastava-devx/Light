"""Depth-completeness belief registers — server-side, Python (F04).

Ported from `apps/mobile/src/cognitive/{contracts.ts,BeliefModel.ts}`'s log-odds update law, and
re-aimed from *user state* (attention/energy) to *spec state* (coverage/ambiguity/depth/
alternatives/contradiction/reuse) per
`blueprints/Speed-of-Thought-L8-Deep-Dive/02-DIALOGUE-PLANE-BUILD-MODE.md` §4.

Pure module: no I/O, no clock, no model call, no import of `fastapi`, `lld_schemas`,
`gateway_client`, or any `store.*`. This is blueprint §4.1's firewall ("beliefs steer the
conversation; they never freeze anything") enforced STRUCTURALLY, not by convention —
`tests/test_f04_belief_registers.py`'s A5 asserts the absence of those imports by parsing this
module's own AST, so there is no code path from a belief value to a freeze verdict because this
module cannot name one. Freeze is a pure function of `ModuleBrief` fields (F05's surface); nothing
here is ever an input to it.

Two deliberate, disclosed deviations from the blueprint's literal §4.1/§4.2 text (see the lane
contract's §2.5 for the full argument):

1. `last_evidence_ts: EpochMs | None` (the TS reference's field) is replaced by
   `last_evidence_seq: int | None` — the index of the evidence-log entry that last moved this
   register. A timestamp inside a structure whose defining property is bit-identical replay is a
   live impurity: either injected (two replays of the same log then differ) or clock-read (the fold
   is not pure). A sequence number preserves everything `last_evidence_ts` was actually used for
   here (provenance, "has this ever seen evidence") while making wall-clock decay
   *unrepresentable* rather than merely unimplemented.
2. `BeliefModel.ts`'s `decay()` (wall-clock reversion toward the prior) is NOT ported. Blueprint
   §4.2: "A spec does not fade on the clock: a slot filled four minutes ago is still filled." There
   is no `now` parameter anywhere in this module to pass a clock into —
   `tests/test_f04_belief_registers.py`'s A2.6 asserts this by inspecting every public function's
   signature and by scanning this module's imports for `time`/`datetime`, rather than trusting this
   comment (`enforcement-over-documentation`).

Decay's *replacement* — dependency-directed invalidation (§3.1b `invalidate`, λ = 0.6) — IS new
here and has no TS counterpart: when a filled slot's premise is revised, only its *named
dependents* are knocked down, never a blanket decay of everything.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from enum import Enum
from math import copysign, exp, isfinite
from math import log as _ln

# ---------------------------------------------------------------------------------------------
# Constants — ported verbatim from apps/mobile/src/cognitive/contracts.ts. Same names, same
# values, so a reader can diff the two files by eye.
# ---------------------------------------------------------------------------------------------

PRIOR_VALUE = 0.5
"""BeliefModel.ts:23 — probability 0.5 == 0 logits. A register with no evidence sits here at zero
confidence."""

MAX_ABS_LOGIT = 6.0
"""BeliefModel.ts:31 — clamping bound for log-odds. Without it, repeated same-direction evidence
drives `value` to exactly 0 or 1, `logit` returns +-inf, and every later update is NaN: the
register silently stops responding to evidence forever."""

CONFIDENCE_GAIN_KAPPA = 0.4
"""contracts.ts:59 — `conf <- min(1, conf + kappa*r)`."""

MAX_TIER2_EVIDENCE_LOGITS = 0.8
"""contracts.ts:57 — `|w*r| <= 0.8` logits per event, Tier-2 (model-sourced) evidence only."""

DEPENDENCY_INVALIDATION_LAMBDA = 0.6
"""blueprint 02 §4.2's "lambda = 0.6 [A]". NEW — no TS counterpart (TS has wall-clock decay
instead; see the module docstring for why that is not ported)."""


# ---------------------------------------------------------------------------------------------
# Registers (blueprint §4.1, re-aimed from the 9 user beliefs — none of the focus names survive).
# ---------------------------------------------------------------------------------------------


class Register(str, Enum):
    COVERAGE = "Coverage"  # vector over slots — see CoverageSlot
    AMBIGUITY = "Ambiguity"
    DEPTH = "Depth"
    ALTERNATIVES = "Alternatives"
    CONTRADICTION = "Contradiction"  # HARD: F05 consumes this; never enforced here (§2.6's firewall)
    REUSE_RESOLVED = "ReuseResolved"


class CoverageSlot(str, Enum):
    """The required-slot set. A named P0 stand-in for `lld_schemas.ModuleBrief`'s field
    vocabulary (interface/data_owned/acceptance/deps/non_goals) plus the registry verdict —
    matched by NAME only; this module adds no field to that schema and does not import it (the
    firewall above forbids importing `lld_schemas` at all).
    """

    INTERFACE = "interface"
    DATA_OWNED = "data_owned"
    ACCEPTANCE = "acceptance"
    DEPS = "deps"
    NON_GOALS = "non_goals"
    REGISTRY_VERDICT = "registry_verdict"


class EvidenceTier(str, Enum):
    """The 3-tier evidence cascade (contracts.ts) — only TIER2 involves a model, and only TIER2's
    influence is capped (`MAX_TIER2_EVIDENCE_LOGITS`)."""

    TIER0 = "tier0"  # rules / arithmetic, ~0ms, deterministic, uncapped
    TIER1 = "tier1"  # exemplar/cosine similarity, uncapped
    TIER2 = "tier2"  # schema-locked LLM evidence, capped


RegisterKey = tuple[Register, CoverageSlot | None]
"""One flat key space, no special-casing: `(Register.COVERAGE, some_slot)` for the six coverage
slots, `(register, None)` for the five scalar registers."""


@dataclass(frozen=True, slots=True)
class RegisterState:
    value: float  # 0..1
    confidence: float  # 0..1
    last_evidence_seq: int | None  # the evidence-log index that last moved this register — §2.5


@dataclass(frozen=True, slots=True)
class Evidence:
    register: Register
    slot: CoverageSlot | None
    weight: float
    reliability: float  # declared 0..1; CLAMPED on entry inside apply_evidence (see below)
    tier: EvidenceTier


@dataclass(frozen=True, slots=True)
class PremiseRevised:
    """blueprint §4.2's truth-maintenance event. Appended to the same evidence log as `Evidence`."""

    premise: CoverageSlot
    dependents: tuple[CoverageSlot, ...]  # from the caller's dep graph (build_session.py's P0 stand-in)


Registers = Mapping[RegisterKey, RegisterState]
LogEntry = Evidence | PremiseRevised

_ALL_COVERAGE_KEYS: tuple[RegisterKey, ...] = tuple((Register.COVERAGE, slot) for slot in CoverageSlot)
_SCALAR_REGISTERS: tuple[Register, ...] = (
    Register.AMBIGUITY,
    Register.DEPTH,
    Register.ALTERNATIVES,
    Register.CONTRADICTION,
    Register.REUSE_RESOLVED,
)
_ALL_SCALAR_KEYS: tuple[RegisterKey, ...] = tuple((register, None) for register in _SCALAR_REGISTERS)


def _logit(p: float) -> float:
    clamped = min(max(p, 1e-6), 1 - 1e-6)
    return _ln(clamped / (1 - clamped))


def _sigmoid(x: float) -> float:
    return 1.0 / (1.0 + exp(-x))


def initial_registers() -> Registers:
    """Every register key at `(value=0.5, confidence=0.0, last_evidence_seq=None)` — the prior,
    unseen state (A2.8: every `Register` and every `CoverageSlot` is reachable from here).
    """
    prior = RegisterState(value=PRIOR_VALUE, confidence=0.0, last_evidence_seq=None)
    return {key: prior for key in (*_ALL_COVERAGE_KEYS, *_ALL_SCALAR_KEYS)}


def apply_evidence(regs: Registers, evidence: Evidence, seq: int) -> Registers:
    """§3.1a — ported verbatim from `BeliefModel.ts:86-118`, re-aimed at spec registers.

    ```
    r      = 0.0 if not isfinite(reliability) else min(1.0, max(0.0, reliability))
    delta  = weight * r
    if tier is TIER2: delta = copysign(min(abs(delta), MAX_TIER2_EVIDENCE_LOGITS), delta)
    logit' = clamp(logit(value) + delta, -MAX_ABS_LOGIT, +MAX_ABS_LOGIT)
    value' = sigmoid(logit')
    conf'  = min(1.0, confidence + CONFIDENCE_GAIN_KAPPA * r)
    ```

    Only every OTHER key in `regs` is guaranteed unchanged (same object, not merely equal) — this
    function only ever touches `(evidence.register, evidence.slot)`.
    """
    key: RegisterKey = (evidence.register, evidence.slot)
    current = regs[key]

    reliability = 0.0 if not isfinite(evidence.reliability) else min(1.0, max(0.0, evidence.reliability))
    delta = evidence.weight * reliability
    if evidence.tier is EvidenceTier.TIER2:
        delta = copysign(min(abs(delta), MAX_TIER2_EVIDENCE_LOGITS), delta)

    next_logit = min(max(_logit(current.value) + delta, -MAX_ABS_LOGIT), MAX_ABS_LOGIT)
    next_state = RegisterState(
        value=_sigmoid(next_logit),
        confidence=min(1.0, current.confidence + CONFIDENCE_GAIN_KAPPA * reliability),
        last_evidence_seq=seq,
    )
    return {**regs, key: next_state}


def invalidate(regs: Registers, event: PremiseRevised, seq: int) -> Registers:
    """§3.1b — NEW, blueprint §4.2, no TS counterpart.

    ```
    for s in event.dependents:                 # and ONLY these
        conf_s'  = conf_s * (1 - lambda)
        logit_s' = logit_s + (logit(PRIOR_VALUE) - logit_s) * lambda      # logit(0.5) == 0.0
        value_s' = sigmoid(logit_s')
    ```

    A register NOT in `event.dependents` is returned identical (same object, not merely equal).
    Because `lambda` in (0, 1), confidence only ever contracts toward zero here — it can never
    RISE, so a wrong belief cannot be made to look more certain by "invalidating" it.
    """
    prior_logit = _logit(PRIOR_VALUE)  # == 0.0
    updated = dict(regs)
    for slot in event.dependents:
        key: RegisterKey = (Register.COVERAGE, slot)
        current = updated[key]
        current_logit = _logit(current.value)
        next_logit = current_logit + (prior_logit - current_logit) * DEPENDENCY_INVALIDATION_LAMBDA
        updated[key] = RegisterState(
            value=_sigmoid(next_logit),
            confidence=current.confidence * (1 - DEPENDENCY_INVALIDATION_LAMBDA),
            last_evidence_seq=seq,
        )
    return updated


def fold(log: Sequence[LogEntry]) -> Registers:
    """`reduce` over a mixed `Evidence | PremiseRevised` log, `seq = index` in `log`.

    Pure and total: `fold(log) == fold(log)` always (A2.7's replay-determinism half); a log with
    two same-register entries transposed generally folds to a DIFFERENT result (A2.7's negative
    control) whenever an intermediate step saturates `MAX_ABS_LOGIT` differently depending on
    order — addition alone commutes, but the clamp does not.
    """
    regs = initial_registers()
    for seq, entry in enumerate(log):
        if isinstance(entry, PremiseRevised):
            regs = invalidate(regs, entry, seq)
        elif isinstance(entry, Evidence):
            regs = apply_evidence(regs, entry, seq)
        else:
            raise TypeError(f"unknown evidence-log entry type: {type(entry)!r}")
    return regs
