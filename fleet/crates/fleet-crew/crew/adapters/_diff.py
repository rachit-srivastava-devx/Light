"""Refresh a diff evidence file after a CLI exits, before success is judged."""

from __future__ import annotations

import subprocess
from pathlib import Path

from .errors import AdapterError


def refresh_diff(diff_path: Path, cwd: Path) -> None:
    """Write ``git -C cwd diff`` output to ``diff_path``.

    Runs after the CLI exits so a pre-seeded placeholder cannot satisfy the
    empty-diff invariant in ``run_model``.
    """

    try:
        diff = subprocess.run(
            ["git", "-C", str(cwd), "diff"], stdin=subprocess.DEVNULL,
            capture_output=True, check=False,
        )
    except FileNotFoundError as exc:
        raise AdapterError(f"unable to measure git diff: {exc}") from exc
    if diff.returncode == 0 and isinstance(diff.stdout, (bytes, bytearray)):
        diff_path.parent.mkdir(parents=True, exist_ok=True)
        diff_path.write_bytes(bytes(diff.stdout))
