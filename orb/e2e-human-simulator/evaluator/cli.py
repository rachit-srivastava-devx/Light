"""Command-line entry point for the independent evaluator."""

from __future__ import annotations

import argparse
from pathlib import Path

from .checks import evaluate
from .judges import run_judges
from .report import render_report
from .trace_schema import load_jsonl


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Evaluate a captured ADHD Focus Orb JSONL trace.")
    parser.add_argument("trace", type=Path, help="Captured JSONL evidence trace")
    parser.add_argument("--out", type=Path, help="Write Markdown report to this path")
    parser.add_argument("--judges", action="store_true", help="Run local Codex and Claude over captured evidence only")
    args = parser.parse_args(argv)
    evaluation = evaluate(load_jsonl(args.trace))
    judges = run_judges(evaluation) if args.judges else None
    report = render_report(evaluation, judges)
    if args.out:
        args.out.write_text(report, encoding="utf-8")
    else:
        print(report)
    return 0 if evaluation.passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
