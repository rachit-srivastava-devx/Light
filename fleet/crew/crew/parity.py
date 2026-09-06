"""Prompter-parity equivalence experiment.

The experiment implements blueprint S4b: Welch's two-sample TOST at alpha
0.05 and its corresponding 90% confidence interval.  The raw-score margin and
its justification are required command-line inputs so they are declared before
the observations are read rather than selected after looking at the data.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import IO, Iterable, Sequence

import numpy as np
from statsmodels.stats.weightstats import CompareMeans, DescrStatsW, ttost_ind


ALPHA = 0.05
POWER = 0.80
DESIGN_EFFECT_SIZE = 0.25
MIN_TOTAL_N = 128
MIN_PER_PROMPTER = MIN_TOTAL_N // 2


class ParityRefusal(ValueError):
    """An invalid or underpowered experiment, with the repository exit type."""

    exit_code = 7

    def __init__(self, reason: str, *, result: "ParityResult | None" = None):
        self.reason = reason
        self.result = result
        super().__init__(reason)


@dataclass(frozen=True)
class Observation:
    prompter: str
    task: str
    run: int
    score: float


@dataclass(frozen=True)
class ParityResult:
    verdict: str
    prompters: tuple[str, str]
    difference: float
    ci_90: tuple[float | None, float | None]
    margin: float
    margin_justification: str
    n: int
    checked: int
    total: int
    n_by_prompter: dict[str, int]
    required_n: int
    tost_p_value: float | None
    lower_test_p_value: float | None
    upper_test_p_value: float | None
    alpha: float = ALPHA
    target_power: float = POWER
    design_effect_size: float = DESIGN_EFFECT_SIZE

    def as_dict(self) -> dict[str, object]:
        return {
            "verdict": self.verdict,
            "prompters": list(self.prompters),
            "difference": self.difference,
            "ci_90": {"low": self.ci_90[0], "high": self.ci_90[1]},
            "margin": self.margin,
            "margin_justification": self.margin_justification,
            "n": self.n,
            "checked": self.checked,
            "total": self.total,
            "n_by_prompter": self.n_by_prompter,
            "required_n": self.required_n,
            "tost": {
                "p_value": self.tost_p_value,
                "lower_one_sided_p_value": self.lower_test_p_value,
                "upper_one_sided_p_value": self.upper_test_p_value,
                "alpha": self.alpha,
            },
            "power_design": {
                "target_power": self.target_power,
                "effect_size_f": self.design_effect_size,
                "required_n": self.required_n,
                "basis": "blueprint S4b: one-df medium-effect design",
            },
        }


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
            raise ParityRefusal(
                f"line {line_number}: expected exactly prompter, task, run, score"
            )
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


def analyze_parity(
    observations: Sequence[Observation],
    *,
    margin: float,
    margin_justification: str,
) -> ParityResult:
    """Run Welch TOST and return an explicit equivalence/power verdict."""

    if not math.isfinite(margin) or margin <= 0:
        raise ParityRefusal("equivalence margin must be a finite positive number")
    if not margin_justification.strip():
        raise ParityRefusal("equivalence margin justification must be non-empty")
    if not observations:
        raise ParityRefusal("input contains zero observations")

    prompters = tuple(dict.fromkeys(row.prompter for row in observations))
    if len(prompters) != 2:
        raise ParityRefusal(f"expected exactly two prompters; found {len(prompters)}")
    scores = {
        name: np.asarray(
            [row.score for row in observations if row.prompter == name], dtype=float
        )
        for name in prompters
    }
    counts = {name: int(values.size) for name, values in scores.items()}
    if any(count < 2 for count in counts.values()):
        raise ParityRefusal("each prompter needs at least two observations for TOST")

    first, second = prompters
    difference = float(np.mean(scores[first]) - np.mean(scores[second]))
    n = len(observations)
    powered = n >= MIN_TOTAL_N and all(
        count >= MIN_PER_PROMPTER for count in counts.values()
    )
    if np.var(scores[first], ddof=1) == 0 and np.var(scores[second], ddof=1) == 0:
        if powered:
            raise ParityRefusal("TOST is undefined; scores must contain non-zero variance")
        result = ParityResult(
            verdict="UNDERPOWERED",
            prompters=(first, second),
            difference=difference,
            ci_90=(None, None),
            margin=margin,
            margin_justification=margin_justification.strip(),
            n=n,
            checked=n,
            total=n,
            n_by_prompter=counts,
            required_n=MIN_TOTAL_N,
            tost_p_value=None,
            lower_test_p_value=None,
            upper_test_p_value=None,
        )
        raise ParityRefusal(
            f"underpowered: N={n}; need N={MIN_TOTAL_N} total and {MIN_PER_PROMPTER} per prompter",
            result=result,
        )
    tost_p, lower_test, upper_test = ttost_ind(
        scores[first], scores[second], -margin, margin, usevar="unequal"
    )
    ci_low, ci_high = CompareMeans(
        DescrStatsW(scores[first]), DescrStatsW(scores[second])
    ).tconfint_diff(alpha=2 * ALPHA, usevar="unequal")
    statistics = (
        float(tost_p),
        float(lower_test[1]),
        float(upper_test[1]),
        float(ci_low),
        float(ci_high),
    )
    if not all(math.isfinite(value) for value in statistics):
        raise ParityRefusal("TOST is undefined; scores must contain non-zero variance")
    equivalent = (
        float(lower_test[1]) < ALPHA
        and float(upper_test[1]) < ALPHA
        and float(ci_low) > -margin
        and float(ci_high) < margin
    )
    verdict = (
        "UNDERPOWERED"
        if not powered
        else ("EQUIVALENT" if equivalent else "NOT-EQUIVALENT")
    )
    result = ParityResult(
        verdict=verdict,
        prompters=(first, second),
        difference=difference,
        ci_90=(statistics[3], statistics[4]),
        margin=margin,
        margin_justification=margin_justification.strip(),
        n=n,
        checked=n,
        total=n,
        n_by_prompter=counts,
        required_n=MIN_TOTAL_N,
        tost_p_value=statistics[0],
        lower_test_p_value=statistics[1],
        upper_test_p_value=statistics[2],
    )
    if not powered:
        raise ParityRefusal(
            f"underpowered: N={n}; need N={MIN_TOTAL_N} total and {MIN_PER_PROMPTER} per prompter",
            result=result,
        )
    return result


def _open_input(path: str) -> IO[str]:
    if path == "-":
        return sys.stdin
    try:
        return Path(path).open(encoding="utf-8")
    except OSError as exc:
        raise ParityRefusal(f"cannot read input {path!r}: {exc.strerror}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run the Fleet S4b prompter-parity TOST")
    parser.add_argument("input", help="JSONL observations, or - for standard input")
    parser.add_argument(
        "--margin", type=float, required=True, help="preregistered raw-score epsilon"
    )
    parser.add_argument(
        "--margin-justification",
        required=True,
        help="why this margin is the largest practically equivalent score difference",
    )
    args = parser.parse_args(argv)
    try:
        stream = _open_input(args.input)
        try:
            observations = read_observations(stream)
        finally:
            if stream is not sys.stdin:
                stream.close()
        result = analyze_parity(
            observations,
            margin=args.margin,
            margin_justification=args.margin_justification,
        )
    except ParityRefusal as refusal:
        if refusal.result is not None:
            print(json.dumps(refusal.result.as_dict(), sort_keys=True))
        receipt = {
            "event": "refusal",
            "exit_code": refusal.exit_code,
            "body": {"reason": refusal.reason},
        }
        print(json.dumps(receipt, sort_keys=True), file=sys.stderr)
        return refusal.exit_code
    print(json.dumps(result.as_dict(), sort_keys=True))
    return 0


__all__ = [
    "ALPHA",
    "MIN_PER_PROMPTER",
    "MIN_TOTAL_N",
    "Observation",
    "ParityRefusal",
    "ParityResult",
    "analyze_parity",
    "main",
    "read_observations",
]


if __name__ == "__main__":
    raise SystemExit(main())
