"""Split observations into the two named prompter score arrays."""

from __future__ import annotations

from collections.abc import Sequence

import numpy as np

from .types import Observation, ParityRefusal


def group_by_prompter(
    observations: Sequence[Observation],
) -> tuple[str, str, dict[str, np.ndarray], dict[str, int]]:
    """Return ``(first, second, scores_by_name, counts_by_name)``."""

    prompters = tuple(dict.fromkeys(row.prompter for row in observations))
    if len(prompters) != 2:
        raise ParityRefusal(f"expected exactly two prompters; found {len(prompters)}")
    scores = {
        name: np.asarray([row.score for row in observations if row.prompter == name], dtype=float)
        for name in prompters
    }
    counts = {name: int(values.size) for name, values in scores.items()}
    if any(count < 2 for count in counts.values()):
        raise ParityRefusal("each prompter needs at least two observations for TOST")
    first, second = prompters
    return first, second, scores, counts
