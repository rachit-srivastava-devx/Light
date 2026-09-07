"""Atomizer + STT/TTS proxy wire schemas.

Canonical source: docs/BUILD-DIGEST.md §2 ("Atomizer schema", "Context pack"). This is the
schema's primary home per docs/adr/0004-backend-language-split.md — the mobile client
(apps/mobile/src/session/StepGate.ts) keeps its own local TypeScript mirror per the existing
duplication rule (docs/adr/LESSONS.md L5); the extraction trigger for a single shared,
language-agnostic schema fires at Tier 5 (ContextPack.ts/ClarifyProtocol.ts), unchanged.
"""

from __future__ import annotations

from enum import Enum
from typing import Annotated, Literal

from pydantic import BaseModel, Field, field_validator, model_validator

from .lld_schemas import KilledAlt, ModuleBrief

# BUILD-DIGEST §2: "steps: [{step_text: string(<=120 chars), est_min: int(1..15), done_signal}],
# steps_total: int(1..12)".
STEP_TEXT_MAX_CHARS = 120
EST_MIN_MIN = 1
EST_MIN_MAX = 15
STEPS_TOTAL_MIN = 1
STEPS_TOTAL_MAX = 12


class AtomizerStep(BaseModel):
    step_text: Annotated[str, Field(min_length=1, max_length=STEP_TEXT_MAX_CHARS)]
    est_min: Annotated[int, Field(ge=EST_MIN_MIN, le=EST_MIN_MAX)]
    done_signal: Annotated[str, Field(min_length=1)]


class AtomizerOutput(BaseModel):
    steps: Annotated[list[AtomizerStep], Field(min_length=STEPS_TOTAL_MIN, max_length=STEPS_TOTAL_MAX)]
    steps_total: Annotated[int, Field(ge=STEPS_TOTAL_MIN, le=STEPS_TOTAL_MAX)]

    @field_validator("steps_total")
    @classmethod
    def steps_total_matches_len(cls, v: int, info: object) -> int:
        # Pydantic v2 doesn't give cross-field access in a plain field_validator without
        # model_validator; the real cross-check (steps_total == len(steps)) is enforced in
        # `validate_atomizer_output` below, which is the single call site every caller must use —
        # this validator only re-asserts the per-field bound so a direct AtomizerOutput(...)
        # construction can't silently skip it either.
        if not (STEPS_TOTAL_MIN <= v <= STEPS_TOTAL_MAX):
            raise ValueError(f"steps_total must be in [{STEPS_TOTAL_MIN}, {STEPS_TOTAL_MAX}]")
        return v


class AtomizerErrorCode(str, Enum):
    """BUILD-DIGEST §2/§4 error taxonomy — exactly these five, no more."""

    SCHEMA_INVALID = "schema_invalid"
    STEP_NOT_ATOMIC = "step_not_atomic"
    STEP_COUNT_EXPLOSION = "step_count_explosion"
    EMPTY_OR_DUPLICATE = "empty_or_duplicate"
    OFF_TASK = "off_task"


class AtomizerValidationError(BaseModel):
    code: AtomizerErrorCode
    detail: str


def validate_atomizer_output(raw: dict) -> AtomizerOutput | AtomizerValidationError:
    """The one call site: structured decoding -> validate -> repair-once -> fail-closed
    (BUILD-DIGEST §2/§4). This function is the "validate" step only; repair-once and fail-closed
    are the caller's (the Atomizer pipeline's) responsibility — see proxy/atomizer.py.
    """
    try:
        output = AtomizerOutput.model_validate(raw)
    except Exception as exc:  # noqa: BLE001 - re-raised as the typed taxonomy below
        return AtomizerValidationError(code=AtomizerErrorCode.SCHEMA_INVALID, detail=str(exc))

    if len(output.steps) != output.steps_total:
        return AtomizerValidationError(
            code=AtomizerErrorCode.SCHEMA_INVALID,
            detail=f"steps_total ({output.steps_total}) != len(steps) ({len(output.steps)})",
        )

    texts = [s.step_text.strip().lower() for s in output.steps]
    if len(texts) != len(set(texts)) or any(not t for t in texts):
        return AtomizerValidationError(
            code=AtomizerErrorCode.EMPTY_OR_DUPLICATE,
            detail="duplicate or empty step_text",
        )

    return output


class ResponseMode(str, Enum):
    """`/v1/respond`'s explicit, typed mode (Track C acceptance contract C1/B4: "the chosen mode is
    explicit and observable in the response envelope, never inferred by the caller ... illegal or
    unknown mode is unrepresentable"). An unrecognized string is a FastAPI/pydantic 422 validation
    error, never a silent coercion to a default — only *omitting* the field defaults (to FOCUS, for
    backward compatibility with the existing mobile caller; see app.py).
    """

    CONVERSE = "converse"
    FOCUS = "focus"
    TEACH = "teach"
    # Speed-of-Thought P0 (docs/SPEED-OF-THOUGHT-P0-CONTRACT.md §1.3): the build/module-brief loop.
    # Matches apps/mobile/src/router/contracts.ts's OrbMode literal exactly.
    BUILD = "build"


class RegisterReading(BaseModel):
    """One depth-completeness belief register's current wire reading (F04).

    `register` is slot-qualified for the Coverage vector (e.g. `"Coverage[interface]"`) and bare
    for the five scalar registers (e.g. `"Ambiguity"`) -- see
    `orb_relay.build.build_session.render_registers`, the one call site that builds this list.
    """

    register: str
    value: float
    confidence: float


class DecomposeOutcome(BaseModel):
    """F03 §3.6: the wire mirror of `lld_decomposer.DecomposeResult`, minus `usage` (internal cost
    accounting -- not a client concern, same reasoning `AtomizeResponse` already applies by only
    surfacing `spent_paise`/`remaining_paise`, never a raw token count).

    Non-null on `ConversationResponse.decompose` iff `mode` is BUILD -- the same "non-null iff
    BUILD" idiom `BuildTurnState` below already established (`app.py:363`'s own words).
    """

    kind: Literal["brief", "clarify_request"]
    brief: ModuleBrief | None
    missing: list[str]
    rejections: list[str]


class AskKindWire(str, Enum):
    """F05 (blueprint `02-DIALOGUE-PLANE-BUILD-MODE.md` §5.1) -- the wire mirror of
    `build.freeze_protocol.AskKind`. A separate enum, not a re-export: this one is part of the
    frozen wire contract (`proxy.schemas`), the other is this lane's pure decision logic
    (`build.freeze_protocol`) -- the two are kept from drifting apart by
    `tests/test_f05_freeze_protocol.py`'s and `test_f05_convergence.py`'s own assertions, not by a
    shared identity.
    """

    CONFIRM = "confirm"
    CHOOSE = "choose"
    ANSWER = "answer"


class Ask(BaseModel):
    """blueprint §5.1 -- one clarifying question, wire-shaped."""

    kind: AskKindWire
    question: Annotated[str, Field(min_length=1)]
    options: list[str] = []  # non-empty iff kind is CHOOSE
    slot: str | None = None  # the CoverageSlot this targets, or None for a depth/contradiction ask

    @model_validator(mode="after")
    def _options_non_empty_iff_choose(self) -> Ask:
        if self.kind is AskKindWire.CHOOSE and not self.options:
            raise ValueError('Ask.options must be non-empty when kind is "choose"')
        if self.kind is not AskKindWire.CHOOSE and self.options:
            raise ValueError('Ask.options must be empty unless kind is "choose"')
        return self


class ProposalTurn(BaseModel):
    """blueprint §5.1 -- the no-naked-proposal type: a PROPOSE turn MUST show at least one
    tradeoff (`cons`) and at least two killed alternatives, structurally, not by convention
    (`tests/test_f05_freeze_protocol.py`'s T6.3 asserts both floors raise `ValidationError`).
    """

    kind: Literal["PROPOSE"]
    target: Annotated[str, Field(min_length=1)]  # node_id
    proposal: Annotated[str, Field(min_length=1)]
    why: Annotated[str, Field(min_length=1)]
    achieves: list[str]
    pros: list[str]
    cons: Annotated[list[str], Field(min_length=1)]
    killed: Annotated[list[KilledAlt], Field(min_length=2)]
    ask: Ask


class ReadinessVerdictWire(BaseModel):
    """The wire mirror of `build.readiness.ReadinessVerdict` -- `detail` is `stdout` and `stderr`
    joined for a human to read, never parsed by any caller (the same "carried as evidence, never
    data" rule `readiness.py` itself holds server-side)."""

    outcome: Literal["ready", "not_ready", "unavailable"]
    exit_code: int | None
    detail: str


class FreezeProposal(BaseModel):
    """The (freeze) event (F05, lane contract §4.3/§2.1). Deliberately NOT an `lld.v1` `Freeze`:
    `proposed_by` is a DIFFERENT const from `Freeze.stamped_by` so no coercion between the two
    types can typecheck, and no field here is named `stamped_by` -- `tests/test_f05_convergence
    .py`'s T7.2 asserts that string is not a key anywhere in the response body containing this.
    """

    proposed_by: Literal["orb:dialogue"]
    node_id: Annotated[str, Field(min_length=1)]
    content_hash: Annotated[str, Field(pattern=r"^sha256:[0-9a-f]{64}$")]  # of the ModuleBrief only
    brief: ModuleBrief
    readiness: ReadinessVerdictWire
    clarify_turns_used: int
    residual_ambiguity: float
    coverage: list[RegisterReading]


class HandoffOutcomeWire(str, Enum):
    """F09 -- the wire mirror of `build.handoff.HandoffOutcome`. A separate enum, not a re-export,
    the same idiom `AskKindWire` above already established: this one is part of the frozen wire
    contract (`proxy.schemas`), the other is the pure subprocess-driving logic (`build.handoff`).
    """

    SOW_READY = "sow_ready"
    NOT_STAMPED = "not_stamped"
    NOT_ACCEPTED = "not_accepted"
    UNAVAILABLE = "unavailable"
    TIMEOUT = "timeout"


class SowHandoff(BaseModel):
    """F09 (lane contract §5.3) -- the freeze->SOW handoff outcome for this turn. Non-null exactly
    on the turn `ConversationResponse.freeze` is non-null (same additive-field discipline
    `FreezeProposal` above already holds). Carries only ids/paths to echo -- no `freeze` object and
    no `lld.v1` sub-document, because the orb never holds a stamp (lane contract §1/§4.1).
    """

    outcome: HandoffOutcomeWire
    sow_id: str | None
    freeze_id: str | None
    node_id: str
    freeze_version: int | None
    detail: str


class FreezeProtocolState(BaseModel):
    """Additive on `BuildTurnState` (F05). Advisory-shaped like the rest of `BuildTurnState`, but
    `move` is the one field a client actually branches on: "ask" (show `best_question_*` as the
    next clarifying question), "freeze" (a `FreezeProposal` is present on this same turn), or
    "split" (the module should be broken up -- lane contract §3.4/§9's SPLIT_TRIGGER_TURNS ceiling).
    """

    move: Literal["ask", "freeze", "split"]
    residual_ambiguity: float
    best_question_slot: str | None
    best_question_kind: AskKindWire | None
    best_question_eig: float
    escalated: bool
    clarify_turns: int
    blocking: list[str]


class BuildTurnState(BaseModel):
    """Advisory only. Blueprint `Speed-of-Thought-L8-Deep-Dive/02-DIALOGUE-PLANE-BUILD-MODE.md`
    §4.1's firewall: beliefs steer the conversation, they never freeze anything. Nothing in this
    object is ever an input to any gate -- the freeze verdict is a pure function of `ModuleBrief`
    fields (F05's surface), never of a register value.
    """

    registers: list[RegisterReading]
    next_slot: str | None  # which slot the orb intends to ask about next; None once all are covered
    contradiction_blocking: bool  # Contradiction register above threshold -- F05 consumes; advisory here
    # F05: the freeze-protocol move for this turn. None only for a pre-F05 caller that never sent
    # a build turn through the new code path -- see ConversationResponse.build's own compatibility
    # note; every real BUILD turn populates this.
    protocol: FreezeProtocolState | None = None


class BeatKind(str, Enum):
    """What a speakable beat is doing. EXPLAIN delivers content; CHECK checks understanding or
    offers the next beat (contract C5 — teach mode must not dump information in one block).
    """

    EXPLAIN = "explain"
    CHECK = "check"


class Beat(BaseModel):
    """One speakable, interruptible chunk of a spoken reply (contract C4). `index` is the beat's
    position in this turn so a client can resume a specific beat after a barge-in; `is_final` marks
    the last beat so a client knows the turn is complete without inspecting content.
    """

    index: Annotated[int, Field(ge=0)]
    text: Annotated[str, Field(min_length=1)]
    kind: BeatKind
    is_final: bool


class SttProxyRequest(BaseModel):
    session_id: str
    audio_codec: str = "opus"
    sample_rate_hz: int = 16_000


class SttProxyResponsePartial(BaseModel):
    session_id: str
    transcript_partial: str
    is_final: bool


class TtsProxyRequest(BaseModel):
    session_id: str
    text: str
    voice_id: str
    emotion: str


class TtsProxyResponseChunk(BaseModel):
    session_id: str
    audio_chunk_b64: str
    is_final: bool
