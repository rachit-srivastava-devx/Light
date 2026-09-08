"""``python -m crew.sow.cli`` entry point."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence

from .build import build_sow
from .errors import SOWRefusal
from .parse_sections import _clean_task
from .payload import _sow_payload

# D42: naming the deficiency is not showing how to answer it. A refusal a
# user cannot act on is a dead end however accurate it is -- so a refusal
# also prints a worked, fillable example of the section convention.
_EXAMPLE = (
    '  fleet sow --task "add a --version flag',
    "  leaves:",
    "  - print the version and exit 0 | acceptance: --version prints a semver, exits 0",
    "  challenges:",
    "  - may collide with an existing flag | citation: keel/fleet/src/main.rs:40",
    "  alternatives:",
    "  - json-output: emit JSON instead | tradeoff: harder for humans to read",
    "  - build-info: add commit and date | tradeoff: leaks build-host details",
    "  estimates:",
    "  - 30 minutes",
    "  edge cases:",
    '  - --version combined with another flag"',
)


def main(argv: Sequence[str] | None = None) -> int:
    """Build a validated SOW from one task string and print JSON."""

    parser = argparse.ArgumentParser(description="Build and validate a Fleet SOW")
    parser.add_argument("--task", required=True, help="task text, including optional SOW sections")
    args = parser.parse_args(argv)
    try:
        sow = build_sow(args.task)
    except SOWRefusal as refusal:
        print(f"SOW refused: {refusal}", file=sys.stderr)
        try:
            lines = _clean_task(args.task)
        except SOWRefusal:
            lines = ()
        if lines:
            print("CLARIFYING QUESTIONS", file=sys.stderr)
            print(f"1. {refusal}? [SOW line 1]", file=sys.stderr)
            print(file=sys.stderr)
            print("Answer by re-running with the sections filled in:", file=sys.stderr)
            print(file=sys.stderr)
            for example_line in _EXAMPLE:
                print(example_line, file=sys.stderr)
            print(file=sys.stderr)
            print(
                "Each challenge needs a real citation (file:line or corpus id), and at least two",
                file=sys.stderr,
            )
            print("named alternatives with tradeoffs -- one option is not a choice.", file=sys.stderr)
        return refusal.exit_code
    print(json.dumps(_sow_payload(sow), sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
