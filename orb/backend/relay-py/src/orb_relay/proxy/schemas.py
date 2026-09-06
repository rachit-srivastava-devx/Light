"""Atomizer + STT/TTS proxy wire schemas.

Canonical source: docs/BUILD-DIGEST.md §2 ("Atomizer schema", "Context pack"). This is the
schema's primary home per docs/adr/0004-backend-language-split.md — the mobile client
(apps/mobile/src/session/StepGate.ts) keeps its own local TypeScript mirror per the existing
duplication rule (docs/adr/LESSONS.md L5); the extraction trigger for a single shared,
language-agnostic schema fires at Tier 5 (ContextPack.ts/ClarifyProtocol.ts), unchanged.
"""

from __future__ import annotations

from enum import Enum
from typing import Annotated

from pydantic import BaseModel, Field, field_validator

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
