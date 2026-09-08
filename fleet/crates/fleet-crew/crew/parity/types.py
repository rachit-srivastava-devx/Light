"""Design constants, the typed refusal, and the raw observation shape.

Blueprint S4b: Welch's two-sample TOST at alpha 0.05 and its corresponding
90% confidence interval. The margin and its justification are required
inputs so they are declared before the observations are read, not selected
after looking at the data.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .result import ParityResult

ALPHA = 0.05
POWER = 0.80
DESIGN_EFFECT_SIZE = 0.25
MIN_TOTAL_N = 128
MIN_PER_PROMPTER = MIN_TOTAL_N // 2


class ParityRefusal(ValueError):
    """An invalid or underpowered experiment, with the repository exit type."""

    exit_code = 7

    def __init__(self, reason: str, *, result: ParityResult | None = None):
        self.reason = reason
        self.result = result
        super().__init__(reason)


@dataclass(frozen=True)
class Observation:
    prompter: str
    task: str
    run: int
    score: float
