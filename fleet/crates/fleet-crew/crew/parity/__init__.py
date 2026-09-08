"""The prompter-parity Welch's-TOST equivalence experiment."""

from .analyze import analyze_parity
from .read import read_observations
from .result import ParityResult
from .types import ALPHA, MIN_PER_PROMPTER, MIN_TOTAL_N, Observation, ParityRefusal

__all__ = [
    "ALPHA",
    "MIN_PER_PROMPTER",
    "MIN_TOTAL_N",
    "Observation",
    "ParityRefusal",
    "ParityResult",
    "analyze_parity",
    "read_observations",
]
