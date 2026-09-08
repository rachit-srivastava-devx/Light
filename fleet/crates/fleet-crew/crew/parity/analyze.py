"""The Welch's-TOST orchestration itself -- this package's reason to exist.

Prompter-count check -> per-group variance check -> ``run_tost`` -> joint
verdict logic. Grouping lives in ``_grouping.py`` and the statsmodels calls
live in ``_tost.py`` so this file stays under the package-wide 80-line cap
without splitting the derivation's threshold logic across files.
"""

from __future__ import annotations

import math
from collections.abc import Sequence

import numpy as np

from ._grouping import group_by_prompter
from ._tost import run_tost
from .result import ParityResult
from .types import ALPHA, MIN_PER_PROMPTER, MIN_TOTAL_N, Observation, ParityRefusal


def _require_preregistration(
    observations: Sequence[Observation], margin: float, margin_justification: str
) -> None:
    if not math.isfinite(margin) or margin <= 0:
        raise ParityRefusal("equivalence margin must be a finite positive number")
    if not margin_justification.strip():
        raise ParityRefusal("equivalence margin justification must be non-empty")
    if not observations:
        raise ParityRefusal("input contains zero observations")


def analyze_parity(
    observations: Sequence[Observation], *, margin: float, margin_justification: str,
) -> ParityResult:
    """Run Welch TOST and return an explicit equivalence/power verdict."""

    _require_preregistration(observations, margin, margin_justification)
    first, second, scores, counts = group_by_prompter(observations)
    difference = float(np.mean(scores[first]) - np.mean(scores[second]))
    n = len(observations)
    powered = n >= MIN_TOTAL_N and all(count >= MIN_PER_PROMPTER for count in counts.values())
    zero_variance = np.var(scores[first], ddof=1) == 0 and np.var(scores[second], ddof=1) == 0

    def _result(verdict: str, stats: tuple[float | None, ...]) -> ParityResult:
        tost_p, lower_p, upper_p, ci_low, ci_high = stats
        return ParityResult(
            verdict=verdict, prompters=(first, second), difference=difference,
            ci_90=(ci_low, ci_high), margin=margin, margin_justification=margin_justification.strip(),
            n=n, checked=n, total=n, n_by_prompter=counts, required_n=MIN_TOTAL_N,
            tost_p_value=tost_p, lower_test_p_value=lower_p, upper_test_p_value=upper_p,
        )

    def _refuse_underpowered(result: ParityResult) -> None:
        raise ParityRefusal(
            f"underpowered: N={n}; need N={MIN_TOTAL_N} total and {MIN_PER_PROMPTER} per prompter",
            result=result,
        )

    if zero_variance:
        if powered:
            raise ParityRefusal("TOST is undefined; scores must contain non-zero variance")
        _refuse_underpowered(_result("UNDERPOWERED", (None, None, None, None, None)))

    stats = run_tost(scores[first], scores[second], margin)
    if not all(math.isfinite(value) for value in stats):
        raise ParityRefusal("TOST is undefined; scores must contain non-zero variance")
    _, lower_p, upper_p, ci_low, ci_high = stats
    equivalent = lower_p < ALPHA and upper_p < ALPHA and ci_low > -margin and ci_high < margin
    verdict = "UNDERPOWERED" if not powered else ("EQUIVALENT" if equivalent else "NOT-EQUIVALENT")
    result = _result(verdict, stats)
    if not powered:
        _refuse_underpowered(result)
    return result
