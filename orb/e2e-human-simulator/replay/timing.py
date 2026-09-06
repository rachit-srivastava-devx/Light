"""Injectable replay timing for real-time and accelerated playback."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable
import time


@dataclass
class ReplayClock:
    """Sleep between frames at ``speed`` times source time.

    ``speed=1`` is real-time. Values greater than one accelerate playback. Tests can
    inject ``sleep_fn`` to record delays without waiting.
    """

    speed: float = 1.0
    sleep_fn: Callable[[float], None] = time.sleep

    def __post_init__(self) -> None:
        if self.speed <= 0:
            raise ValueError("speed must be greater than zero")

    def wait_for_frame(self, frame_duration_ms: float) -> float:
        delay_seconds = frame_duration_ms / 1000 / self.speed
        self.sleep_fn(delay_seconds)
        return delay_seconds
