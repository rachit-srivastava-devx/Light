"""Follow-up probe: does the price_paise float-precision defect (see
test_cost_meter_properties.py::test_price_paise_never_undercounts_the_exact_value, which found a
counterexample at ~9e15 tts_chars_novel) reproduce at REALISTIC production scale, or only at
cosmically large inputs no real caller would send?

Realistic bounds used here: tokens/chars up to 2,000,000 (an order of magnitude past any single
teach-mode turn this product could plausibly send in one gateway call) and rates up to 1,000 paise
per unit (the real configured rates, see app.py rates_from_env, are 10/48/143/50 -- this is >6x
headroom). This narrows the finding to an honest severity: "reachable only past ~2**53" vs
"reachable in production-plausible ranges" are very different bug reports.
"""

from __future__ import annotations

import math
from fractions import Fraction

from hypothesis import given, settings, strategies as st

from orb_relay.cost.meter import Rates, UsageDelta, price_paise

REALISTIC_COUNT = st.integers(min_value=0, max_value=2_000_000)
REALISTIC_RATE = st.integers(min_value=0, max_value=1_000)

usage = st.builds(
    UsageDelta,
    llm_tokens_in=REALISTIC_COUNT,
    llm_tokens_out=REALISTIC_COUNT,
    tts_chars_novel=REALISTIC_COUNT,
    # Real STT providers bill at millisecond-ish granularity, not arbitrary float64 noise. Testing
    # with unrestricted st.floats() down to subnormal magnitudes (~1e-38s) found a "counterexample"
    # that turned out to be a test-methodology artifact, not a product bug -- see the note below
    # this file's docstring in the H2 report. This strategy is deliberately restricted to
    # realistic billing granularity (whole milliseconds) so a failure here means something.
    stt_seconds_billed=st.integers(min_value=0, max_value=3_600_000).map(lambda ms: ms / 1000.0),
)
rates = st.builds(
    Rates,
    paise_per_1k_llm_tokens_in=REALISTIC_RATE,
    paise_per_1k_llm_tokens_out=REALISTIC_RATE,
    paise_per_1k_tts_chars=REALISTIC_RATE,
    paise_per_stt_minute=REALISTIC_RATE,
)


@given(delta=usage, rates=rates)
@settings(max_examples=20_000, deadline=None)
def test_price_paise_exact_at_realistic_production_scale(delta: UsageDelta, rates: Rates) -> None:
    exact = (
        Fraction(delta.llm_tokens_in) * Fraction(rates.paise_per_1k_llm_tokens_in) / 1000
        + Fraction(delta.llm_tokens_out) * Fraction(rates.paise_per_1k_llm_tokens_out) / 1000
        + Fraction(delta.tts_chars_novel) * Fraction(rates.paise_per_1k_tts_chars) / 1000
        # Fraction(float) (no limit_denominator) gives the float64's EXACT rational value, so this
        # ground truth has zero precision loss of its own -- a rounding artifact here would be a
        # false alarm about meter.py, not a real finding.
        + Fraction(delta.stt_seconds_billed) * Fraction(rates.paise_per_stt_minute) / 60
    )
    observed = price_paise(delta, rates)
    assert observed == math.ceil(exact), (
        f"price_paise WRONG at realistic scale: observed={observed}, exact_ceil={math.ceil(exact)}, "
        f"delta={delta}, rates={rates}"
    )
