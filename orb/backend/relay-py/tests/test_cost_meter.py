"""CostMeter: the reservation must fail closed BEFORE spend (INV5), and the currency/usage unit
split from docs/adr/LESSONS.md L4 must hold.
"""

from typing import cast

import pytest

from orb_relay.cost.meter import (
    Rates,
    ReservationExceededError,
    SessionMeter,
    UsageDelta,
    UsageValidationError,
    price_paise,
)

RATES = Rates(
    paise_per_1k_llm_tokens_in=10,
    paise_per_1k_llm_tokens_out=50,
    paise_per_1k_tts_chars=140,
    paise_per_stt_minute=48,
)


def test_stt_seconds_may_be_fractional() -> None:
    """LESSONS L4: this is the field that broke last time — providers bill fractional seconds."""
    delta = UsageDelta(stt_seconds_billed=12.5)
    assert delta.stt_seconds_billed == 12.5


def test_discrete_counts_reject_floats() -> None:
    with pytest.raises(UsageValidationError) as exc_info:
        UsageDelta(llm_tokens_in=cast(int, 1.5))
    assert str(exc_info.value) == "llm_tokens_in must be an int (discrete unit), got float"


def test_rejects_negative_usage() -> None:
    with pytest.raises(UsageValidationError, match=">= 0"):
        UsageDelta(tts_chars_novel=-1)


def test_rejects_negative_continuous_seconds_with_exact_error() -> None:
    with pytest.raises(UsageValidationError) as exc_info:
        UsageDelta(stt_seconds_billed=-0.25)
    assert str(exc_info.value) == "stt_seconds_billed must be >= 0, got -0.25"


def test_rejects_non_finite_seconds() -> None:
    with pytest.raises(UsageValidationError, match="finite"):
        UsageDelta(stt_seconds_billed=float("inf"))


def test_rejects_bool_as_a_count() -> None:
    """bool is an int subclass in Python — without an explicit check, True would meter as 1."""
    with pytest.raises(UsageValidationError):
        UsageDelta(llm_tokens_in=True)


def test_rejects_bool_as_continuous_seconds_with_exact_error() -> None:
    """bool is numeric in Python, but it is never a valid provider-billed duration."""
    with pytest.raises(UsageValidationError) as exc_info:
        UsageDelta(stt_seconds_billed=True)
    assert str(exc_info.value) == "stt_seconds_billed must be numeric (continuous unit), got bool"


def test_price_rounds_up_so_a_cap_never_under_counts() -> None:
    # 1 token in at 10 paise/1k = 0.01 paise, which must not floor to 0.
    assert price_paise(UsageDelta(llm_tokens_in=1), RATES) == 1


@pytest.mark.parametrize(
    ("delta", "expected_paise"),
    [
        (UsageDelta(llm_tokens_in=1_000), 10),
        (UsageDelta(llm_tokens_out=1_000), 50),
        (UsageDelta(tts_chars_novel=1_000), 140),
        (UsageDelta(stt_seconds_billed=60), 48),
    ],
)
def test_each_price_line_has_an_exact_positive_direction(
    delta: UsageDelta, expected_paise: int
) -> None:
    """Pins every divisor and sign: no line may disappear, invert, or use /1001."""
    assert price_paise(delta, RATES) == expected_paise


def test_realistic_mixed_charge_cannot_round_down_at_the_integer_boundary() -> None:
    delta = UsageDelta(llm_tokens_in=400, tts_chars_novel=80, stt_seconds_billed=0.006)
    rates = Rates(
        paise_per_1k_llm_tokens_in=1,
        paise_per_1k_tts_chars=57,
        paise_per_stt_minute=400,
    )
    assert price_paise(delta, rates) == 6


def test_price_is_zero_for_zero_usage() -> None:
    assert price_paise(UsageDelta(), RATES) == 0


def test_charge_refuses_before_spending_when_over_reservation() -> None:
    meter = SessionMeter(tenant_id="t1", session_id="s1", user_id="u1", reservation_paise=100)
    huge = UsageDelta(llm_tokens_out=1_000_000)
    with pytest.raises(ReservationExceededError) as exc_info:
        meter.charge("llm", huge, RATES)
    error = exc_info.value
    assert str(error) == (
        "tenant t1, session s1: spend of 50000 paise exceeds remaining reservation of 100 paise"
    )
    assert error.tenant_id == "t1"
    assert error.session_id == "s1"
    assert error.requested_paise == 50_000
    assert error.remaining_paise == 100
    # the refusal must leave the ledger untouched — nothing was spent
    assert meter.spent_paise == 0
    assert meter.ledger == []


def test_reservation_error_without_tenant_has_an_exact_diagnostic() -> None:
    error = ReservationExceededError(
        session_id="s-unknown", requested_paise=2, remaining_paise=1
    )
    assert str(error) == (
        "tenant <unknown>, session s-unknown: spend of 2 paise exceeds remaining reservation of 1 paise"
    )


def test_charge_accumulates_and_reports_remaining() -> None:
    meter = SessionMeter(tenant_id="t1", session_id="s1", user_id="u1", reservation_paise=400)
    cost = meter.charge("llm", UsageDelta(llm_tokens_in=1000, llm_tokens_out=1000), RATES)
    assert cost == 60
    assert meter.spent_paise == 60
    assert meter.remaining_paise == 340
    assert meter.ledger == [("llm", 60)]


def test_charge_at_exactly_the_remaining_balance_is_allowed() -> None:
    """The boundary: 'exceeds' means strictly greater. Spending the last paise is legal."""
    meter = SessionMeter(tenant_id="t1", session_id="s1", user_id="u1", reservation_paise=1)
    meter.charge("llm", UsageDelta(llm_tokens_in=1), RATES)
    assert meter.remaining_paise == 0


def test_would_exceed_predicts_the_refusal_without_spending() -> None:
    meter = SessionMeter(tenant_id="t1", session_id="s1", user_id="u1", reservation_paise=10)
    delta = UsageDelta(llm_tokens_out=1_000)
    assert meter.would_exceed(delta, RATES) is True
    assert meter.spent_paise == 0


def test_reservation_is_held_before_spend_and_released_on_settlement() -> None:
    meter = SessionMeter(tenant_id="t1", session_id="s1", user_id="u1", reservation_paise=100)
    held = meter.reserve_remaining()
    assert held == 100
    assert meter.reserved_paise == 100
    assert meter.spent_paise == 0

    meter.settle("llm", held, UsageDelta(llm_tokens_in=1), Rates(paise_per_1k_llm_tokens_in=1000))
    assert meter.spent_paise == 1
    assert meter.remaining_paise == 99
    assert meter.reserved_paise == 0


def test_oversized_settlement_rejects_and_releases_the_hold_atomically() -> None:
    """INV5: a rejected settlement must not strand the pre-spend reservation."""
    meter = SessionMeter(tenant_id="t1", session_id="s1", user_id="u1", reservation_paise=10)
    held = meter.reserve_remaining()

    with pytest.raises(ReservationExceededError):
        meter.settle(
            "llm",
            held,
            UsageDelta(llm_tokens_in=11),
            Rates(paise_per_1k_llm_tokens_in=1000),
        )

    assert meter.spent_paise == 0
    assert meter.reserved_paise == 0
    assert meter.ledger == []
