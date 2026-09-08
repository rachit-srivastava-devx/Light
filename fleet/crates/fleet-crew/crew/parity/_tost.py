"""The statsmodels TOST/CI calls, isolated so ``analyze.py`` stays orchestration.

Reimplementing Welch's TOST by hand would be exactly the kind of "adopting a
tool is not the tool working" risk this repo's own PRINCIPLES.md warns
against -- use the maintained library, call it from one narrow seam.
"""

from __future__ import annotations

import numpy as np
from statsmodels.stats.weightstats import CompareMeans, DescrStatsW, ttost_ind

from .types import ALPHA


def run_tost(
    first: np.ndarray, second: np.ndarray, margin: float
) -> tuple[float, float, float, float, float]:
    """Return ``(tost_p, lower_p, upper_p, ci_low, ci_high)`` for two samples."""

    tost_p, lower_test, upper_test = ttost_ind(first, second, -margin, margin, usevar="unequal")
    ci_low, ci_high = CompareMeans(
        DescrStatsW(first), DescrStatsW(second)
    ).tconfint_diff(alpha=2 * ALPHA, usevar="unequal")
    return float(tost_p), float(lower_test[1]), float(upper_test[1]), float(ci_low), float(ci_high)
