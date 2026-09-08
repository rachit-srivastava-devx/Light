"""Shared ``invoke()`` body used by every CLI-backed adapter.

Extracted so ``claude.py``/``codex.py`` differ only in ``command()``
construction, never in invocation mechanics (the two files carried a
byte-for-byte duplicate ``invoke()`` before this split).
"""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

from .metadata import read_metadata
from .records import InvocationResult
from .runner import run_model


def invoke_cli(
    adapter,
    prompt: str,
    *,
    stdout_path: Path,
    stderr_path: Path,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
    diff_path: Path | None = None,
) -> InvocationResult:
    """Run ``adapter.command(prompt)`` and read back model/usage evidence."""

    result = run_model(
        adapter.command(prompt), stdout_path=stdout_path, stderr_path=stderr_path,
        cwd=cwd, env=env, resolved_model=adapter.resolved_model(),
        usage=adapter._last_usage, diff_path=diff_path,
        log_size_floor=adapter.log_size_floor,
    )
    model, usage = read_metadata(stdout_path)
    if model is not None:
        adapter._resolved_model = model
    if usage is not None:
        adapter._last_usage = usage
    return InvocationResult(
        returncode=result.returncode,
        stdout_path=result.stdout_path,
        stderr_path=result.stderr_path,
        resolved_model=adapter._resolved_model,
        usage=adapter._last_usage,
    )
