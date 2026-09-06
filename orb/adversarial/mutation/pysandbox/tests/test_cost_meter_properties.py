"""H2-1 property-based tests (hypothesis, MPL-2.0) for backend/relay-py/src/orb_relay/cost/meter.py.

Track H2 (adversarial). Black-box: this file only imports orb_relay.cost.meter, it does not modify
it. Run with:

    ORB_ROOT=/Users/rachitsrivastava/youtube/Principal Engineering/company/products/adhd-focus-orb
    PYTHONPATH="$ORB_ROOT/backend/relay-py/src" \
      adversarial/.venv/bin/pytest -q adversarial/py-fuzz/test_cost_meter_properties.py

The specific hunt (per the H2 brief): "money/cost arithmetic must never lose precision; floats for
money is a defect -- if you find one, report it loudly." meter.py's own docstring says rounding
UP is deliberate because under-counting is the unsafe direction ("lets a session drift past its
reservation"). price_paise() computes with float division (`/1000`, `/60`) before `math.ceil`, so
the property under test is: does the float path ever under-count relative to the exact rational
value? We compute the exact answer with `fractions.Fraction` (no precision loss) and compare.
"""

from __future__ import annotations

import math
from decimal import Decimal
from fractions import Fraction

import pytest
from hypothesis import HealthCheck, given, settings, strategies as st

from orb_relay.cost.meter import (
    Rates,
    ReservationExceededError,
    SessionMeter,
    UsageDelta,
    UsageValidationError,
    price_paise,
)

# ---------------------------------------------------------------------------------------------
# Strategies. Bounds are generous (real STT/LLM bills are nowhere near these) specifically to
# stress the float path: hypothesis is told to hunt near float64's 2**53 integer-precision cliff
# and near IEEE754 rounding boundaries, not just "reasonable" values.
# ---------------------------------------------------------------------------------------------
_DISCRETE = st.integers(min_value=0, max_value=2**53 + 10_000)
_CONTINUOUS = st.floats(min_value=0, max_value=1e12, allow_nan=False, allow_infinity=False)
_RATE = st.integers(min_value=0, max_value=100_000)

usage_strategy = st.builds(
    UsageDelta,
    llm_tokens_in=_DISCRETE,
    llm_tokens_out=_DISCRETE,
    tts_chars_novel=_DISCRETE,
    stt_seconds_billed=_CONTINUOUS,
)
rates_strategy = st.builds(
    Rates,
    paise_per_1k_llm_tokens_in=_RATE,
    paise_per_1k_llm_tokens_out=_RATE,
    paise_per_1k_tts_chars=_RATE,
    paise_per_stt_minute=_RATE,
)


def _exact_paise(delta: UsageDelta, rates: Rates) -> Fraction:
    """Ground truth: the same formula as price_paise, but in exact rational arithmetic."""
    return (
        Fraction(delta.llm_tokens_in) * Fraction(rates.paise_per_1k_llm_tokens_in) / 1000
        + Fraction(delta.llm_tokens_out) * Fraction(rates.paise_per_1k_llm_tokens_out) / 1000
        + Fraction(delta.tts_chars_novel) * Fraction(rates.paise_per_1k_tts_chars) / 1000
        # Fraction(float) (no limit_denominator) gives the float64's EXACT rational value, so this
        # ground truth carries zero precision loss of its own.
        + Fraction(delta.stt_seconds_billed) * Fraction(rates.paise_per_stt_minute) / 60
    )


@given(delta=usage_strategy, rates=rates_strategy)
@settings(max_examples=2000, deadline=None, suppress_health_check=[HealthCheck.too_slow])
def test_price_paise_never_undercounts_the_exact_value(delta: UsageDelta, rates: Rates) -> None:
    """FLAGSHIP PROPERTY: price_paise(...) must be >= the exact rational price, always.

    meter.py's own comment says under-counting is the unsafe direction for a spend guard. If the
    float-based computation ever returns LESS than ceil(exact value), that is exactly the
    "floats for money" defect class H2 was told to hunt for.
    """
    observed = price_paise(delta, rates)
    exact = _exact_paise(delta, rates)
    exact_ceiling = math.ceil(exact)
    assert observed >= exact_ceiling, (
        f"price_paise UNDER-COUNTED: float path returned {observed} paise but the exact "
        f"rational computation ceils to {exact_ceiling} paise (exact={exact}) for delta={delta}, "
        f"rates={rates}. This is the unsafe direction: a session could spend past its reservation."
    )


@given(delta=usage_strategy, rates=rates_strategy)
@settings(max_examples=2000, deadline=None, suppress_health_check=[HealthCheck.too_slow])
def test_price_paise_never_overcounts_by_more_than_a_few_paise(delta: UsageDelta, rates: Rates) -> None:
    """Rounding up is allowed (documented, deliberate), but the OVER-count must stay small and
    bounded -- large float-summation error compounding into many-paise overcharges would itself be
    a (different, milder) real-money defect worth reporting.
    """
    observed = price_paise(delta, rates)
    exact = _exact_paise(delta, rates)
    # Generous bound: legitimate ceil() rounding is < 1 paise per priced field (4 fields), plus a
    # little slack for float summation noise. Anything past this is a real precision bug, not
    # "rounding up is deliberate".
    assert observed - float(exact) <= 5, (
        f"price_paise OVER-COUNTED by more than rounding-up can explain: observed={observed}, "
        f"exact={float(exact):.6f}, delta={delta}, rates={rates}"
    )


@given(delta=usage_strategy, rates=rates_strategy)
@settings(max_examples=500, deadline=None)
def test_price_paise_is_monotonic_in_each_field(delta: UsageDelta, rates: Rates) -> None:
    """More usage must never cost less. A float-precision inversion here would silently undercharge
    a heavier turn relative to a lighter one.
    """
    bumped = UsageDelta(
        llm_tokens_in=delta.llm_tokens_in + 1,
        llm_tokens_out=delta.llm_tokens_out,
        tts_chars_novel=delta.tts_chars_novel,
        stt_seconds_billed=delta.stt_seconds_billed,
    )
    assert price_paise(bumped, rates) >= price_paise(delta, rates)


@given(
    llm_in=_DISCRETE, llm_out=_DISCRETE, tts=_DISCRETE, rates=rates_strategy,
    reservation=st.integers(min_value=0, max_value=10_000),
)
@settings(max_examples=500, deadline=None, suppress_health_check=[HealthCheck.too_slow])
def test_session_meter_charge_never_exceeds_reservation(
    llm_in: int, llm_out: int, tts: int, rates: Rates, reservation: int
) -> None:
    """INV5 as a property, not an example: no sequence of admitted charges can push spent_paise
    above reservation_paise, for ANY usage/rate combination hypothesis can find -- including the
    float-precision edge cases the example-based unit tests don't think to try.
    """
    meter = SessionMeter(tenant_id="t", session_id="s", user_id="u", reservation_paise=reservation)
    delta = UsageDelta(llm_tokens_in=llm_in, llm_tokens_out=llm_out, tts_chars_novel=tts)
    try:
        meter.charge("llm", delta, rates)
    except ReservationExceededError:
        pass
    assert meter.spent_paise <= meter.reservation_paise, (
        f"INV5 VIOLATED: spent_paise={meter.spent_paise} > reservation_paise={meter.reservation_paise} "
        f"after charge(delta={delta}, rates={rates})"
    )


@given(
    reservation=st.integers(min_value=1, max_value=10_000),
    deltas=st.lists(
        st.builds(UsageDelta, llm_tokens_in=_DISCRETE, llm_tokens_out=_DISCRETE, tts_chars_novel=_DISCRETE),
        min_size=1, max_size=20,
    ),
    rates=rates_strategy,
)
@settings(max_examples=300, deadline=None, suppress_health_check=[HealthCheck.too_slow])
def test_reserve_release_settle_round_trip_never_overspends(
    reservation: int, deltas: list[UsageDelta], rates: Rates
) -> None:
    """The reserve-whole-balance / settle-actual pattern app.py uses for /v1/respond and
    /v1/atomize. Property: however many (reserve, settle-or-release) cycles run, spent_paise can
    never exceed reservation_paise, and reserved_paise can never go negative.
    """
    meter = SessionMeter(tenant_id="t", session_id="s", user_id="u", reservation_paise=reservation)
    for delta in deltas:
        try:
            held = meter.reserve_remaining()
        except ReservationExceededError:
            break
        try:
            meter.settle("llm", held, delta, rates)
        except ReservationExceededError:
            meter.release(held)
        assert meter.reserved_paise >= 0
        assert meter.spent_paise <= meter.reservation_paise


# ---------------------------------------------------------------------------------------------
# Validation-boundary fuzzing: adversarial inputs to UsageDelta / SessionMeter construction.
# ---------------------------------------------------------------------------------------------


@given(value=st.one_of(st.floats(allow_nan=True, allow_infinity=True), st.integers(), st.booleans()))
@settings(max_examples=300, deadline=None)
def test_discrete_field_rejects_non_int_nan_inf_bool_and_negative(value: object) -> None:
    """llm_tokens_in etc. must be a real, non-negative int. bool is technically an int subclass in
    Python (`isinstance(True, int) is True`) -- a classic validation bypass if a caller forgets to
    exclude it explicitly (meter.py's `_check_discrete` does exclude it; this asserts that holds
    for the whole adversarial input space, not just the one unit-test example).
    """
    is_valid_int = isinstance(value, int) and not isinstance(value, bool) and value >= 0
    if is_valid_int:
        UsageDelta(llm_tokens_in=value)  # must not raise
        return
    with pytest.raises(UsageValidationError):
        UsageDelta(llm_tokens_in=value)  # type: ignore[arg-type]


@given(value=st.one_of(st.floats(allow_nan=True, allow_infinity=True), st.integers(min_value=-(10**9), max_value=10**9)))
@settings(max_examples=300, deadline=None)
def test_continuous_field_rejects_nan_inf_and_negative(value: object) -> None:
    is_valid = (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
        and value >= 0
    )
    if is_valid:
        UsageDelta(stt_seconds_billed=value)  # must not raise
        return
    with pytest.raises(UsageValidationError):
        UsageDelta(stt_seconds_billed=value)  # type: ignore[arg-type]


@given(seconds=st.floats(min_value=1e6, max_value=1e15, allow_nan=False, allow_infinity=False), rate=_RATE)
@settings(max_examples=200, deadline=None)
def test_extreme_stt_seconds_do_not_silently_produce_a_wrong_small_charge(seconds: float, rate: int) -> None:
    """A silently-wrong small charge for a huge (buggy-upstream-provider) seconds value would be a
    cost-control defect worse than a crash: it would UNDER-bill and let real spend leak past the
    reservation while /v1/eval/metrics and dashboards show everything nominal.
    """
    delta = UsageDelta(stt_seconds_billed=seconds)
    rates = Rates(paise_per_stt_minute=rate)
    observed = price_paise(delta, rates)
    exact = math.ceil(Decimal(seconds) * Decimal(rate) / Decimal(60))
    # Allow float64 relative error only (~1e-9 relative), not an absolute-small-number bug.
    assert observed >= exact - 1, (
        f"extreme stt_seconds_billed={seconds} at rate={rate}: got {observed} paise, "
        f"expected >= {exact - 1} (exact Decimal value {exact})"
    )
