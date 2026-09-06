#!/usr/bin/env python3
"""Fail-closed rendered-audio continuity and first-content gate.

Input is a two-channel WAV captured on one host clock: channel 0 is the microphone and channel 1
is the Mac/Simulator output. ffmpeg owns decoding and silence detection; this module only converts
its reported silence intervals into the three Track J metrics. No provider SDK or API key is used.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import subprocess
import sys
import tempfile
from dataclasses import asdict, dataclass
from itertools import pairwise
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
RELAY_PY_SRC = REPO_ROOT / "backend" / "relay-py" / "src"
sys.path.insert(0, str(RELAY_PY_SRC))

from orb_relay.observability.metrics import (
    AudioGapWatchdog,
    HopHistogram,
    loudness_floor_ok,
)

SILENCE_START_RE = re.compile(r"silence_start:\s*([0-9]+(?:\.[0-9]+)?)")
SILENCE_END_RE = re.compile(r"silence_end:\s*([0-9]+(?:\.[0-9]+)?)")
MAX_VOLUME_RE = re.compile(r"max_volume:\s*(-inf|[-+]?[0-9]+(?:\.[0-9]+)?)\s*dB")


class GateInputError(ValueError):
    pass


@dataclass(frozen=True)
class Interval:
    start_ms: float
    end_ms: float


@dataclass(frozen=True)
class TurnMetric:
    turn_id: str
    provider: str
    scheduled_start_ms: float
    actual_user_start_ms: float
    user_stop_ms: float
    presence_latency_ms: float
    content_latency_ms: float
    presence_only_duration_ms: float


def _run(command: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, check=False, capture_output=True, text=True)


def _probe(path: Path) -> tuple[float, int]:
    result = _run(
        [
            "ffprobe",
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=channels:format=duration",
            "-of",
            "json",
            str(path),
        ]
    )
    if result.returncode != 0:
        raise GateInputError(f"ffprobe failed: {result.stderr.strip()}")
    body = json.loads(result.stdout)
    streams = body.get("streams", [])
    if not streams:
        raise GateInputError("capture contains no audio stream")
    duration_s = float(body.get("format", {}).get("duration", 0))
    channels = int(streams[0].get("channels", 0))
    if not math.isfinite(duration_s) or duration_s <= 0:
        raise GateInputError("capture duration must be > 0")
    return duration_s * 1000.0, channels


def _filter_for_channel(channel: int, extra_filter: str | None = None) -> str:
    filters = [f"pan=mono|c0=c{channel}"]
    if extra_filter:
        filters.append(extra_filter)
    return ",".join(filters)


def _silences(
    path: Path,
    channel: int,
    duration_ms: float,
    threshold_db: float,
    minimum_silence_ms: float,
    extra_filter: str | None = None,
) -> list[Interval]:
    audio_filter = _filter_for_channel(channel, extra_filter)
    audio_filter += f",silencedetect=noise={threshold_db}dB:d={minimum_silence_ms / 1000.0}"
    result = _run(
        [
            "ffmpeg",
            "-hide_banner",
            "-nostats",
            "-i",
            str(path),
            "-af",
            audio_filter,
            "-f",
            "null",
            "-",
        ]
    )
    if result.returncode != 0:
        raise GateInputError(f"ffmpeg silencedetect failed: {result.stderr.strip()}")

    intervals: list[Interval] = []
    pending_start_ms: float | None = None
    for line in result.stderr.splitlines():
        start_match = SILENCE_START_RE.search(line)
        if start_match:
            pending_start_ms = float(start_match.group(1)) * 1000.0
        end_match = SILENCE_END_RE.search(line)
        if end_match:
            if pending_start_ms is None:
                pending_start_ms = 0.0
            end_ms = min(float(end_match.group(1)) * 1000.0, duration_ms)
            intervals.append(Interval(pending_start_ms, end_ms))
            pending_start_ms = None
    if pending_start_ms is not None:
        intervals.append(Interval(pending_start_ms, duration_ms))
    return intervals


def _audible_intervals(duration_ms: float, silences: list[Interval]) -> list[Interval]:
    cursor_ms = 0.0
    audible: list[Interval] = []
    for silence in sorted(silences, key=lambda interval: interval.start_ms):
        if silence.start_ms > cursor_ms:
            audible.append(Interval(cursor_ms, silence.start_ms))
        cursor_ms = max(cursor_ms, silence.end_ms)
    if cursor_ms < duration_ms:
        audible.append(Interval(cursor_ms, duration_ms))
    return [interval for interval in audible if interval.end_ms > interval.start_ms]


def _max_volume_dbfs(path: Path, channel: int) -> float:
    audio_filter = _filter_for_channel(channel) + ",volumedetect"
    result = _run(
        [
            "ffmpeg",
            "-hide_banner",
            "-nostats",
            "-i",
            str(path),
            "-af",
            audio_filter,
            "-f",
            "null",
            "-",
        ]
    )
    if result.returncode != 0:
        raise GateInputError(f"ffmpeg volumedetect failed: {result.stderr.strip()}")
    matches = MAX_VOLUME_RE.findall(result.stderr)
    if not matches:
        raise GateInputError("ffmpeg volumedetect produced no max_volume sample")
    return float("-inf") if matches[-1] == "-inf" else float(matches[-1])


def _silero_content_intervals(path: Path, channel: int) -> list[Interval]:
    """Detect rendered speech with Silero VAD; fail closed when the local tool is unavailable."""
    try:
        from silero_vad import get_speech_timestamps, load_silero_vad, read_audio
    except ImportError as error:
        raise GateInputError(
            "Silero VAD is required for real content_latency_ms proof; install the MIT-licensed "
            "local test tool or use --content-detector highpass only for synthetic controls"
        ) from error

    with tempfile.NamedTemporaryFile(suffix=".wav") as mono_wav:
        result = _run(
            [
                "ffmpeg",
                "-hide_banner",
                "-nostats",
                "-y",
                "-i",
                str(path),
                "-af",
                _filter_for_channel(channel),
                "-ar",
                "16000",
                "-ac",
                "1",
                "-c:a",
                "pcm_s16le",
                mono_wav.name,
            ]
        )
        if result.returncode != 0:
            raise GateInputError(
                f"ffmpeg speech-channel extraction failed: {result.stderr.strip()}"
            )
        waveform = read_audio(mono_wav.name, sampling_rate=16_000)
        timestamps = get_speech_timestamps(
            waveform,
            load_silero_vad(),
            sampling_rate=16_000,
            return_seconds=True,
        )
    intervals = [
        Interval(float(timestamp["start"]) * 1000.0, float(timestamp["end"]) * 1000.0)
        for timestamp in timestamps
    ]
    if not intervals:
        raise GateInputError("Silero VAD measured zero rendered-speech intervals")
    return intervals


def _first_audible_at_or_after(intervals: list[Interval], at_ms: float) -> float | None:
    for interval in intervals:
        if interval.end_ms <= at_ms:
            continue
        return max(interval.start_ms, at_ms)
    return None


def _percentiles(values: list[float]) -> dict[str, float | None]:
    if not values:
        return {"p50": None, "p95": None}
    histogram = HopHistogram()
    for value in values:
        histogram.observe(value)
    return {"p50": histogram.percentile(50), "p95": histogram.percentile(95)}


def _load_schedule(path: Path) -> tuple[float, list[dict[str, Any]]]:
    body = json.loads(path.read_text(encoding="utf-8"))
    cadence_ms = float(body.get("cadence_ms", 0))
    turns = body.get("turns")
    if cadence_ms <= 0 or not isinstance(turns, list) or len(turns) < 2:
        raise GateInputError("schedule requires cadence_ms > 0 and at least two turns")
    starts = [float(turn["scheduled_start_ms"]) for turn in turns]
    for previous, current in pairwise(starts):
        if abs((current - previous) - cadence_ms) > 0.001:
            raise GateInputError("turn schedule is not fixed cadence")
    return cadence_ms, turns


def evaluate(args: argparse.Namespace) -> tuple[dict[str, Any], bool]:
    audio_path = Path(args.audio).resolve()
    schedule_path = Path(args.schedule).resolve()
    duration_ms, channels = _probe(audio_path)
    if args.mic_channel >= channels or args.output_channel >= channels:
        raise GateInputError(
            f"capture has {channels} channel(s); mic={args.mic_channel} output={args.output_channel} requested"
        )

    cadence_ms, scheduled_turns = _load_schedule(schedule_path)
    mic_audible = _audible_intervals(
        duration_ms,
        _silences(
            audio_path,
            args.mic_channel,
            duration_ms,
            args.mic_silence_db,
            args.minimum_silence_ms,
        ),
    )
    rendered_audible = _audible_intervals(
        duration_ms,
        _silences(
            audio_path,
            args.output_channel,
            duration_ms,
            args.presence_silence_db,
            args.minimum_silence_ms,
        ),
    )
    if args.content_detector == "silero":
        content_audible = _silero_content_intervals(audio_path, args.output_channel)
    else:
        content_audible = _audible_intervals(
            duration_ms,
            _silences(
                audio_path,
                args.output_channel,
                duration_ms,
                args.content_silence_db,
                args.minimum_silence_ms,
                extra_filter=f"highpass=f={args.content_highpass_hz}",
            ),
        )

    watchdog = AudioGapWatchdog(max_gap_ms=args.max_gap_ms, session_started_at_ms=0)
    for interval in rendered_audible:
        watchdog.observe_interval(interval.start_ms, interval.end_ms)
    watchdog.finish(duration_ms)
    max_volume_dbfs = _max_volume_dbfs(audio_path, args.output_channel)
    loudness_ok = (
        False
        if not math.isfinite(max_volume_dbfs)
        else loudness_floor_ok([max_volume_dbfs], args.loudness_floor_dbfs)
    )

    failures: list[str] = []
    if not watchdog.measured:
        failures.append("zero rendered-audio intervals measured")
    if not loudness_ok:
        failures.append(
            f"rendered output never crossed loudness floor {args.loudness_floor_dbfs} dBFS"
        )
    if watchdog.gap_events > 0:
        failures.append(
            f"{watchdog.gap_events} rendered-audio gap(s) exceeded {args.max_gap_ms} ms"
        )
    if len(mic_audible) != len(scheduled_turns):
        failures.append(
            f"fixed-cadence schedule expected {len(scheduled_turns)} turns; captured {len(mic_audible)}"
        )

    turn_metrics: list[TurnMetric] = []
    for index, scheduled in enumerate(scheduled_turns):
        if index >= len(mic_audible):
            continue
        user_interval = mic_audible[index]
        scheduled_start_ms = float(scheduled["scheduled_start_ms"])
        if abs(user_interval.start_ms - scheduled_start_ms) > args.cadence_tolerance_ms:
            failures.append(
                f"turn {scheduled['turn_id']} started {abs(user_interval.start_ms - scheduled_start_ms):.1f} ms "
                f"off fixed cadence (limit {args.cadence_tolerance_ms} ms)"
            )
        presence_start_ms = _first_audible_at_or_after(rendered_audible, user_interval.end_ms)
        content_start_ms = _first_audible_at_or_after(content_audible, user_interval.end_ms)
        if presence_start_ms is None:
            failures.append(
                f"turn {scheduled['turn_id']} measured no rendered presence after user stop"
            )
            continue
        if content_start_ms is None:
            failures.append(
                f"turn {scheduled['turn_id']} measured no real-content band after user stop"
            )
            continue
        presence_latency_ms = presence_start_ms - user_interval.end_ms
        content_latency_ms = content_start_ms - user_interval.end_ms
        if presence_latency_ms > args.max_gap_ms:
            failures.append(
                f"turn {scheduled['turn_id']} presence latency {presence_latency_ms:.1f} ms exceeded "
                f"{args.max_gap_ms} ms"
            )
        turn_metrics.append(
            TurnMetric(
                turn_id=str(scheduled["turn_id"]),
                provider=str(scheduled.get("provider", "unknown")),
                scheduled_start_ms=scheduled_start_ms,
                actual_user_start_ms=user_interval.start_ms,
                user_stop_ms=user_interval.end_ms,
                presence_latency_ms=presence_latency_ms,
                content_latency_ms=content_latency_ms,
                presence_only_duration_ms=max(0.0, content_start_ms - presence_start_ms),
            )
        )

    gap_values = watchdog.gaps_ms
    presence_values = [metric.presence_latency_ms for metric in turn_metrics]
    content_values = [metric.content_latency_ms for metric in turn_metrics]
    presence_only_values = [metric.presence_only_duration_ms for metric in turn_metrics]
    turns_exceeding = sum(value > args.max_gap_ms for value in presence_values)
    report = {
        "passed": not failures,
        "capture": {
            "path": str(audio_path),
            "duration_ms": duration_ms,
            "channels": channels,
            "max_volume_dbfs": max_volume_dbfs if math.isfinite(max_volume_dbfs) else None,
            "blackhole_license_boundary": "GPL-3 local test tooling only; never bundled",
        },
        "cadence": {
            "cadence_ms": cadence_ms,
            "scheduled_turns": len(scheduled_turns),
            "measured_turns": len(mic_audible),
            "tolerance_ms": args.cadence_tolerance_ms,
        },
        "continuity": {
            "audio_intervals_analysed": watchdog.observed_audio_intervals,
            "total_audio_analysed_ms": duration_ms,
            "longest_gap_ms": watchdog.longest_gap_ms,
            "p95_gap_ms": _percentiles(gap_values)["p95"],
            "turns_exceeding_max_gap_ms": turns_exceeding,
            "gap_events": watchdog.gap_events,
        },
        "metrics": {
            "presence_latency_ms": _percentiles(presence_values),
            "content_latency_ms": _percentiles(content_values),
            "presence_only_duration_ms": _percentiles(presence_only_values),
        },
        "turns": [asdict(metric) for metric in turn_metrics],
        "failures": failures,
        "content_detector": {
            "method": args.content_detector,
            "highpass_hz": args.content_highpass_hz,
            "threshold_db": args.content_silence_db,
            "proof_status": (
                "rendered-speech VAD"
                if args.content_detector == "silero"
                else "synthetic-control only; not valid real-content proof"
            ),
        },
    }
    return report, not failures


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--audio", required=True)
    parser.add_argument("--schedule", required=True)
    parser.add_argument("--report")
    parser.add_argument("--mic-channel", type=int, default=0)
    parser.add_argument("--output-channel", type=int, default=1)
    # Blueprint 02-ALWAYS-ON-AUDIO-ENGINE.md §11.2 (the loudness-floor probe this gate implements):
    # "assert no window > 20 ms falls below the bed's noise floor". The gap budget itself is 0 ms
    # (§1), bounded below one buffer period (~5-10 ms); 20 ms is the probe's own stated tripwire,
    # not a looser placeholder. Override tighter per-capture only if a specific rig's measurement
    # noise floor demands it; never loosen this default to make a capture pass.
    parser.add_argument("--max-gap-ms", type=float, default=20.0)
    parser.add_argument("--minimum-silence-ms", type=float, default=20.0)
    parser.add_argument("--mic-silence-db", type=float, default=-35.0)
    parser.add_argument("--presence-silence-db", type=float, default=-50.0)
    parser.add_argument("--content-silence-db", type=float, default=-42.0)
    parser.add_argument("--content-highpass-hz", type=float, default=2_000.0)
    parser.add_argument(
        "--content-detector",
        choices=("silero", "highpass"),
        default="silero",
        help="silero is required for real proof; highpass is only for synthetic controls",
    )
    parser.add_argument("--loudness-floor-dbfs", type=float, default=-50.0)
    parser.add_argument("--cadence-tolerance-ms", type=float, default=100.0)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        report, passed = evaluate(args)
    except (GateInputError, OSError, ValueError, json.JSONDecodeError) as error:
        report = {"passed": False, "failures": [str(error)]}
        passed = False
    rendered = json.dumps(report, indent=2, sort_keys=True)
    print(rendered)
    if args.report:
        Path(args.report).write_text(rendered + "\n", encoding="utf-8")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
