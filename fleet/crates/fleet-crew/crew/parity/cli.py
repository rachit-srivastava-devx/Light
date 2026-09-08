"""``python -m crew.parity.cli`` entry point."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import IO

from .analyze import analyze_parity
from .read import read_observations
from .types import ParityRefusal


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
    parser.add_argument("--margin", type=float, required=True, help="preregistered raw-score epsilon")
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
            observations, margin=args.margin, margin_justification=args.margin_justification
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


if __name__ == "__main__":
    raise SystemExit(main())
