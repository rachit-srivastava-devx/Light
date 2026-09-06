"""Contract tests for orb_relay.proxy.schemas — the atomizer schema is a hard prerequisite for
StepGate.ts, ContextPack.ts, and ClarifyProtocol.ts (docs/BUILD-DIGEST.md §8); these tests are the
first gate against silent drift.
"""

from orb_relay.proxy.schemas import (
    STEP_TEXT_MAX_CHARS,
    STEPS_TOTAL_MAX,
    AtomizerErrorCode,
    AtomizerOutput,
    AtomizerValidationError,
    validate_atomizer_output,
)


def _valid_step(i: int) -> dict:
    return {"step_text": f"Open the tax portal step {i}", "est_min": 2, "done_signal": "portal open"}


def test_accepts_well_formed_output() -> None:
    raw = {"steps": [_valid_step(1), _valid_step(2)], "steps_total": 2}
    result = validate_atomizer_output(raw)
    assert isinstance(result, AtomizerOutput)
    assert result.steps_total == 2
    assert len(result.steps) == 2


def test_rejects_steps_total_mismatch() -> None:
    raw = {"steps": [_valid_step(1)], "steps_total": 2}
    result = validate_atomizer_output(raw)
    assert isinstance(result, AtomizerValidationError)
    assert result.code == AtomizerErrorCode.SCHEMA_INVALID


def test_rejects_step_count_explosion() -> None:
    raw = {"steps": [_valid_step(i) for i in range(STEPS_TOTAL_MAX + 1)], "steps_total": STEPS_TOTAL_MAX + 1}
    result = validate_atomizer_output(raw)
    assert isinstance(result, AtomizerValidationError)
    assert result.code == AtomizerErrorCode.SCHEMA_INVALID


def test_rejects_step_text_over_120_chars() -> None:
    raw = {"steps": [{"step_text": "x" * (STEP_TEXT_MAX_CHARS + 1), "est_min": 2, "done_signal": "d"}], "steps_total": 1}
    result = validate_atomizer_output(raw)
    assert isinstance(result, AtomizerValidationError)


def test_rejects_est_min_out_of_bounds() -> None:
    raw = {"steps": [{"step_text": "ok", "est_min": 16, "done_signal": "d"}], "steps_total": 1}
    result = validate_atomizer_output(raw)
    assert isinstance(result, AtomizerValidationError)


def test_rejects_duplicate_step_text() -> None:
    raw = {"steps": [_valid_step(1), _valid_step(1)], "steps_total": 2}
    result = validate_atomizer_output(raw)
    assert isinstance(result, AtomizerValidationError)
    assert result.code == AtomizerErrorCode.EMPTY_OR_DUPLICATE


def test_rejects_empty_steps_total_zero() -> None:
    # steps_total's own bound is >=1 (BUILD-DIGEST §2) — 0 must fail at the schema layer.
    raw = {"steps": [], "steps_total": 0}
    result = validate_atomizer_output(raw)
    assert isinstance(result, AtomizerValidationError)
