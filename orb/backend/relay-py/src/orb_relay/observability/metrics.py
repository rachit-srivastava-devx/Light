"""Observability primitives for the thin relay.

These are in-memory T0 adapters: per-hop latency histograms, audio-gap watchdog, loudness-floor
probe, and sentinel canary state. They do not claim production telemetry storage; they make the
runtime invariants measurable and easy to swap behind a real exporter later (C7).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from math import ceil, isfinite
from typing import cast

from hdrh.histogram import HdrHistogram

# One-unit resolution keeps the acceptance thresholds exact around 2 seconds. HDR widens buckets
# only at larger values, where returning the bucket's upper edge is deliberately conservative.
_HDR_LOWEST_TRACKABLE_VALUE = 1
_HDR_HIGHEST_TRACKABLE_VALUE = 24 * 60 * 60 * 1000
_HDR_SIGNIFICANT_FIGURES = 3


class MetricValidationError(ValueError):
    """Metric input was malformed; callers should drop the sample and log the programmer error."""


def _finite_non_negative(name: str, value: float) -> None:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise MetricValidationError(f"{name} must be numeric")
    if value < 0 or not isfinite(value):
        raise MetricValidationError(f"{name} must be finite and >= 0")


@dataclass
class BoundedHdrHistogram:
    """Fixed-memory nearest-rank histogram for non-negative measurement values.

    Percentiles use the nearest-rank convention: the smallest bucket whose cumulative count is at
    least ``ceil(pct / 100 * N)`` (with pct=0 returning the minimum). There is no linear
    interpolation. ``hdrhistogram`` returns the highest equivalent value in that bucket, so the
    reported result is conservative rather than understating an SLO tail.

    Each supplied observation is recorded; this class never samples. It cannot repair coordinated
    omission created before ``observe`` is called. A closed-loop load generator must instead supply
    an expected interval to an open-loop harness and use HDR's correction API. Guessing that interval
    here would manufacture samples. The fixed 24-hour/3-significant-figure counts array replaces the
    old unbounded list.
    """

    _histogram: HdrHistogram = field(init=False, repr=False)

    def __post_init__(self) -> None:
        self._histogram = HdrHistogram(
            _HDR_LOWEST_TRACKABLE_VALUE,
            _HDR_HIGHEST_TRACKABLE_VALUE,
            _HDR_SIGNIFICANT_FIGURES,
        )

    def observe(self, value: float) -> None:
        _finite_non_negative("value", value)
        # Round upward to the histogram's one-unit resolution; latency can never look faster and
        # spend can never look cheaper because of conversion to HDR's integer input.
        recorded_value = ceil(value)
        if recorded_value > _HDR_HIGHEST_TRACKABLE_VALUE:
            raise MetricValidationError(f"value must be <= {_HDR_HIGHEST_TRACKABLE_VALUE}")
        if not self._histogram.record_value(recorded_value):
            raise MetricValidationError("value was outside the HDR histogram range")

    @property
    def total_count(self) -> int:
        return cast(int, self._histogram.get_total_count())

    @property
    def has_observations(self) -> bool:
        return self.total_count > 0

    @property
    def storage_bucket_count(self) -> int:
        """Fixed allocation size, exposed so the bounded-memory invariant is testable."""
        return len(self._histogram.counts)

    def percentile(self, pct: float) -> float:
        if not isinstance(pct, (int, float)) or isinstance(pct, bool) or not isfinite(pct):
            raise MetricValidationError("pct must be finite and numeric")
        if pct < 0 or pct > 100:
            raise MetricValidationError("pct must be in [0, 100]")
        if not self.has_observations:
            return 0.0
        return float(self._histogram.get_value_at_percentile(pct))


class HopHistogram(BoundedHdrHistogram):
    """Latency-named compatibility surface backed by the shared bounded HDR primitive."""


@dataclass
class AudioGapWatchdog:
    max_gap_ms: float
    session_started_at_ms: float = 0.0
    last_audio_at_ms: float | None = None
    gap_events: int = 0
    observed_audio_points: int = 0
    observed_audio_intervals: int = 0
    gaps_ms: list[float] = field(default_factory=list)

    def __post_init__(self) -> None:
        _finite_non_negative("max_gap_ms", self.max_gap_ms)
        _finite_non_negative("session_started_at_ms", self.session_started_at_ms)

    def _observe_gap(self, next_audio_at_ms: float) -> None:
        previous_audio_at_ms = self.last_audio_at_ms
        if previous_audio_at_ms is None:
            previous_audio_at_ms = self.session_started_at_ms
        gap_ms = next_audio_at_ms - previous_audio_at_ms
        if gap_ms < 0:
            raise MetricValidationError("audio timestamps must be monotonic")
        self.gaps_ms.append(float(gap_ms))
        if gap_ms > self.max_gap_ms:
            self.gap_events += 1

    def observe_audio(self, at_ms: float) -> None:
        _finite_non_negative("at_ms", at_ms)
        self._observe_gap(at_ms)
        self.last_audio_at_ms = float(at_ms)
        self.observed_audio_points += 1

    def observe_interval(self, start_ms: float, end_ms: float) -> None:
        """Observe one actually-audible interval from rendered-output analysis.

        Point sampling cannot distinguish a long audible interval from a long silence between two
        samples. The audio gate therefore reports intervals from ffmpeg's `silencedetect`; only
        the distance from the previous audible interval's end to this interval's start is a gap.
        """
        _finite_non_negative("start_ms", start_ms)
        _finite_non_negative("end_ms", end_ms)
        if end_ms < start_ms:
            raise MetricValidationError("audio interval end must be >= start")
        self._observe_gap(start_ms)
        self.last_audio_at_ms = float(end_ms)
        self.observed_audio_intervals += 1

    def finish(self, session_ended_at_ms: float) -> None:
        """Account for trailing silence; a gate must not ignore the end of its capture."""
        _finite_non_negative("session_ended_at_ms", session_ended_at_ms)
        previous_audio_at_ms = self.last_audio_at_ms
        if previous_audio_at_ms is None:
            previous_audio_at_ms = self.session_started_at_ms
        gap_ms = session_ended_at_ms - previous_audio_at_ms
        if gap_ms < 0:
            raise MetricValidationError("session end must be monotonic")
        self.gaps_ms.append(float(gap_ms))
        if gap_ms > self.max_gap_ms:
            self.gap_events += 1

    @property
    def measured(self) -> bool:
        return self.observed_audio_points > 0 or self.observed_audio_intervals > 0

    @property
    def longest_gap_ms(self) -> float:
        return max(self.gaps_ms, default=0.0)


def loudness_floor_ok(samples_dbfs: list[float], floor_dbfs: float) -> bool:
    if not isinstance(floor_dbfs, (int, float)) or isinstance(floor_dbfs, bool):
        raise MetricValidationError("floor_dbfs must be numeric")
    if not isfinite(floor_dbfs):
        raise MetricValidationError("floor_dbfs must be finite")
    if not samples_dbfs:
        return False
    for sample in samples_dbfs:
        if not isinstance(sample, (int, float)) or isinstance(sample, bool) or not isfinite(sample):
            raise MetricValidationError("samples_dbfs must contain finite numbers")
    return max(samples_dbfs) >= floor_dbfs


@dataclass
class SentinelCanary:
    expected_interval_ms: float
    last_success_at_ms: float | None = None

    def mark_success(self, at_ms: float) -> None:
        _finite_non_negative("at_ms", at_ms)
        self.last_success_at_ms = float(at_ms)

    def stale(self, now_ms: float) -> bool:
        _finite_non_negative("now_ms", now_ms)
        if self.last_success_at_ms is None:
            return True
        return now_ms - self.last_success_at_ms > self.expected_interval_ms
