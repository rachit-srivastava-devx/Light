"""JSONL observation parsing and validation."""

from __future__ import annotations

import json
import math
from collections.abc import Iterable

from .types import Observation, ParityRefusal


def _finite_number(value: object, *, line_number: int) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ParityRefusal(f"line {line_number}: score must be a number")
    score = float(value)
    if not math.isfinite(score):
        raise ParityRefusal(f"line {line_number}: score must be finite")
    return score


def read_observations(lines: Iterable[str]) -> list[Observation]:
    """Parse and validate JSONL observations without silently dropping rows."""

    observations: list[Observation] = []
    seen: set[tuple[str, str, int]] = set()
    required = {"prompter", "task", "run", "score"}
    for line_number, raw_line in enumerate(lines, start=1):
        if not raw_line.strip():
            raise ParityRefusal(f"line {line_number}: blank JSONL record")
        try:
            payload = json.loads(raw_line)
        except json.JSONDecodeError as exc:
            raise ParityRefusal(f"line {line_number}: invalid JSON: {exc.msg}") from exc
        if not isinstance(payload, dict) or set(payload) != required:
            raise ParityRefusal(f"line {line_number}: expected exactly prompter, task, run, score")
        prompter = payload["prompter"]
        task = payload["task"]
        run = payload["run"]
        if not isinstance(prompter, str) or not prompter.strip():
            raise ParityRefusal(f"line {line_number}: prompter must be a non-empty string")
        if not isinstance(task, str) or not task.strip():
            raise ParityRefusal(f"line {line_number}: task must be a non-empty string")
        if isinstance(run, bool) or not isinstance(run, int) or run < 1:
            raise ParityRefusal(f"line {line_number}: run must be a positive integer")
        key = (prompter, task, run)
        if key in seen:
            raise ParityRefusal(f"line {line_number}: duplicate prompter/task/run observation")
        seen.add(key)
        score = _finite_number(payload["score"], line_number=line_number)
        observations.append(Observation(prompter, task, run, score))
    if not observations:
        raise ParityRefusal("input contains zero observations")
    return observations
