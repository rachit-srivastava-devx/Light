"""Build-mode session behavior, server-side (F04 §3.2) — ported from F01's `T0FocusSession.ts`.

Pure: no store, no HTTP. Bridges `cognitive/belief.py`'s registers to the wire shapes in
`proxy/schemas.py`, and supplies:

- `BUILD_OPENING` — the session-open greeting (§3.2).
- `build_slot_dependencies` — the P0 stand-in slot-dependency graph (§3.2), used to construct
  `PremiseRevised` events.
- `derive_turn_evidence` — the deterministic Tier-0 evidence deriver `/v1/respond`'s build-mode
  branch calls once per turn (§3.6c). **Disclosed judgment call**: the contract specifies that a
  build turn "appends this turn's Tier-0 evidence" without naming an exact algorithm (real slot
  extraction is the atomizer/decomposer's job — out of scope here, §8.2). This is a deliberately
  boring, deterministic, keyword-substring match against the user's own turn text: genuinely
  Tier-0 ("rules / arithmetic, ~0ms, no model call", contracts.ts), not a re-derivation of the
  decomposer. It exists so the manual E2E (lane contract §6 step 3f) has *something real* to show
  moving, not to claim real NLU slot-filling — a later lane with real slot extraction replaces this
  wholesale, and nothing here should be read as a substitute for it.
- `next_slot_to_ask` / `is_contradiction_blocking` / `build_turn_state` — advisory-only renderers
  (blueprint §4.1's firewall: nothing here ever gates a freeze).
- `derive_brief_evidence` — F03 §3.3: the brief -> evidence deriver. A field read on an already
  schema-validated `ModuleBrief` (no model call, no NLU), fired once per successfully decomposed
  brief, alongside (never instead of) `derive_turn_evidence`'s per-turn text-keyword evidence. This
  module (unlike `cognitive/belief.py`) is not firewalled against importing `lld_schemas` -- the
  firewall (`tests/test_f04_belief_registers.py`'s A5) is an AST check on `belief.py` specifically.
"""

from __future__ import annotations

from collections.abc import Mapping

from ..cognitive.belief import CoverageSlot, Evidence, EvidenceTier, Register, Registers
from ..proxy.lld_schemas import ModuleBrief
from ..proxy.schemas import BuildTurnState, RegisterReading

# Ported from T0FocusSession.ts:106's BUILD_GREETING -- same string, deliberately, so the two
# clients say the same thing during the transition period and a diff between them is a one-line
# grep. Kept OUT of app.py's `phrase_manifest` and given no `phrase_id` on purpose (F01's
# greeting-array trap: `phrase_manifest`/`GREETINGS` are indexed collections, and appending build
# copy to either would let it surface on an ordinary, non-build turn with no test failure anywhere).
BUILD_OPENING = "Build mode. What are we making?"

_SLOT_DEPENDENTS: dict[CoverageSlot, tuple[CoverageSlot, ...]] = {
    # P0 shape only (§3.2): acceptance and data_owned depend on interface; acceptance depends on
    # data_owned. The blueprint's real dependency structure is the design graph in `03` §4, owned
    # by F05/F02 -- this is a named, minimal stand-in, not that graph.
    CoverageSlot.INTERFACE: (CoverageSlot.DATA_OWNED, CoverageSlot.ACCEPTANCE),
    CoverageSlot.DATA_OWNED: (CoverageSlot.ACCEPTANCE,),
    CoverageSlot.ACCEPTANCE: (),
    CoverageSlot.DEPS: (),
    CoverageSlot.NON_GOALS: (),
    CoverageSlot.REGISTRY_VERDICT: (),
}


def build_slot_dependencies() -> Mapping[CoverageSlot, tuple[CoverageSlot, ...]]:
    """The static P0 dependency edges (§3.2) needed to construct `PremiseRevised` events: for a
    given premise slot, the tuple of slots that depend on it (and must be invalidated when the
    premise is revised). NAMED STAND-IN -- see the module and dict docstrings above; `03` §4 owns
    the real design graph.
    """
    return dict(_SLOT_DEPENDENTS)


# --- Tier-0 turn-evidence deriver ---------------------------------------------------------------

# A lowercase substring match against the user's own turn text, checked in CoverageSlot's declared
# order so a turn mentioning more than one slot's keywords still produces exactly one deterministic
# piece of evidence (first match wins). Genuinely Tier-0: no model call, no cosine similarity, no
# new dependency -- see the module docstring's disclosed-judgment-call note.
_SLOT_KEYWORDS: dict[CoverageSlot, tuple[str, ...]] = {
    CoverageSlot.INTERFACE: ("interface", "signature", "endpoint", " api"),
    CoverageSlot.DATA_OWNED: ("owns", "own ", "database", "table", "storage", "persist"),
    CoverageSlot.ACCEPTANCE: ("acceptance", "given ", "when ", "then ", "accepted"),
    CoverageSlot.DEPS: ("depends on", "dependency", "dependencies", "requires"),
    CoverageSlot.NON_GOALS: ("non-goal", "non goal", "out of scope", "not responsible for"),
    CoverageSlot.REGISTRY_VERDICT: ("reuse", "already exists", "install", "extract", "build-new", "build new"),
}

_TURN_EVIDENCE_WEIGHT = 1.2  # matches A2.1's e1 -- one confident, on-topic turn
_TURN_EVIDENCE_RELIABILITY = 1.0
_DEPTH_FALLBACK_WEIGHT = 0.5  # a smaller, generic "the dialogue progressed" signal

_NEXT_SLOT_CONFIDENCE_FLOOR = 1.0  # a slot below this confidence still needs more evidence
_CONTRADICTION_BLOCKING_THRESHOLD = 0.8


def derive_turn_evidence(user_text: str) -> Evidence:
    """One deterministic Tier-0 `Evidence` for a single build-mode turn's user text.

    Checks `CoverageSlot` in its declared order and returns the first keyword match. With none,
    returns generic `Depth` evidence, so every real turn moves *something* rather than silently
    producing an evidence-free turn (see the module docstring's disclosed judgment call).
    """
    lowered = user_text.lower()
    for slot in CoverageSlot:
        if any(keyword in lowered for keyword in _SLOT_KEYWORDS[slot]):
            return Evidence(
                register=Register.COVERAGE,
                slot=slot,
                weight=_TURN_EVIDENCE_WEIGHT,
                reliability=_TURN_EVIDENCE_RELIABILITY,
                tier=EvidenceTier.TIER0,
            )
    return Evidence(
        register=Register.DEPTH,
        slot=None,
        weight=_DEPTH_FALLBACK_WEIGHT,
        reliability=_TURN_EVIDENCE_RELIABILITY,
        tier=EvidenceTier.TIER0,
    )


def derive_brief_evidence(brief: ModuleBrief) -> list[Evidence]:
    """F03 §3.3: a VALIDATED `ModuleBrief` -> Tier-0 `Evidence` entries.

    A field read on an object `validate_module_brief` has already accepted -- no model call, no
    similarity, no clamp needed, so every entry is `EvidenceTier.TIER0` at the same
    `_TURN_EVIDENCE_WEIGHT`/`_TURN_EVIDENCE_RELIABILITY` scale `derive_turn_evidence` uses (one
    scale, not two, per the lane contract).

    Three hard constraints (contract §3.3), preserved here structurally:
      1. This does not replace or modify `derive_turn_evidence` -- it is a sibling the caller
         appends alongside it, only when a decompose call actually produced a brief.
      2. Never emits `Contradiction` evidence -- that is F05's, and a single decode cannot
         contradict itself (the schema's own cross-field validators already refuse that shape).
      3. The caller (not this function) is what must never call this on a `clarify_request` --
         see the lane contract §6.7b's laundering-guard case. This function has no branch that
         could emit nothing-from-a-brief, because it is never handed anything but one.
    """
    evidence: list[Evidence] = []

    def coverage(slot: CoverageSlot) -> None:
        evidence.append(
            Evidence(
                register=Register.COVERAGE,
                slot=slot,
                weight=_TURN_EVIDENCE_WEIGHT,
                reliability=_TURN_EVIDENCE_RELIABILITY,
                tier=EvidenceTier.TIER0,
            )
        )

    def scalar(register: Register) -> None:
        evidence.append(
            Evidence(
                register=register,
                slot=None,
                weight=_TURN_EVIDENCE_WEIGHT,
                reliability=_TURN_EVIDENCE_RELIABILITY,
                tier=EvidenceTier.TIER0,
            )
        )

    if brief.interface:
        coverage(CoverageSlot.INTERFACE)
    if brief.data_owned:
        coverage(CoverageSlot.DATA_OWNED)
    coverage(CoverageSlot.ACCEPTANCE)  # always -- schema-required, non-optional field
    if brief.deps:
        coverage(CoverageSlot.DEPS)
    if brief.non_goals:
        coverage(CoverageSlot.NON_GOALS)
    coverage(CoverageSlot.REGISTRY_VERDICT)  # always -- schema-required, non-optional field

    scalar(Register.ALTERNATIVES)  # always -- schema floor is >=2 entries
    scalar(Register.DEPTH)  # always -- schema floor is >=1 guarantee

    return evidence


def next_slot_to_ask(registers: Registers) -> CoverageSlot | None:
    """The first `CoverageSlot` (declared order) not yet confidently covered, or `None` once every
    slot is. Advisory only (blueprint §4.1's firewall) -- a hint for what to ask about next, never
    an input to any gate.
    """
    for slot in CoverageSlot:
        if registers[(Register.COVERAGE, slot)].confidence < _NEXT_SLOT_CONFIDENCE_FLOOR:
            return slot
    return None


def is_contradiction_blocking(registers: Registers) -> bool:
    """Advisory read of the `Contradiction` register (blueprint §4.1's firewall: beliefs steer,
    they never freeze -- F05 is the only consumer that may ever act on this).

    Always `False` today: nothing in F04 emits `Contradiction` evidence yet (that needs real
    dependency/claim comparison across turns, out of scope here), so the register never leaves its
    zero-confidence prior. Reading the real register rather than hardcoding `False` keeps this
    correctly wired for whenever a later lane adds a `Contradiction` evidence producer.
    """
    state = registers[(Register.CONTRADICTION, None)]
    return state.confidence > 0.0 and state.value >= _CONTRADICTION_BLOCKING_THRESHOLD


def render_registers(registers: Registers) -> list[RegisterReading]:
    """`Registers` -> the wire shape (`proxy.schemas.RegisterReading`): Coverage slots first
    (declared `CoverageSlot` order, rendered as `"Coverage[<slot>]"`), then the five scalar
    registers (declared `Register` order).
    """
    readings = [
        RegisterReading(
            register=f"{Register.COVERAGE.value}[{slot.value}]",
            value=registers[(Register.COVERAGE, slot)].value,
            confidence=registers[(Register.COVERAGE, slot)].confidence,
        )
        for slot in CoverageSlot
    ]
    readings += [
        RegisterReading(
            register=register.value,
            value=registers[(register, None)].value,
            confidence=registers[(register, None)].confidence,
        )
        for register in (
            Register.AMBIGUITY,
            Register.DEPTH,
            Register.ALTERNATIVES,
            Register.CONTRADICTION,
            Register.REUSE_RESOLVED,
        )
    ]
    return readings


def build_turn_state(registers: Registers) -> BuildTurnState:
    """The full advisory `BuildTurnState` for one build-mode turn's response envelope."""
    next_slot = next_slot_to_ask(registers)
    return BuildTurnState(
        registers=render_registers(registers),
        next_slot=next_slot.value if next_slot is not None else None,
        contradiction_blocking=is_contradiction_blocking(registers),
    )
