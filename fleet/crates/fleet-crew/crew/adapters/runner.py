"""Subprocess execution plus the log-floor/diff success invariants."""

from __future__ import annotations

import subprocess
from collections.abc import Mapping, Sequence
from pathlib import Path

from ._diff import refresh_diff
from .errors import AdapterError
from .records import InvocationResult, UsageRecord

DEFAULT_LOG_SIZE_FLOOR = 40


def _effective_floor(log_size_floor: int) -> int:
    """A non-positive floor would let a near-empty log trivially pass the
    "successful invocation has evidence" check -- fall back to the package
    default instead of silently accepting zero or negative bytes as proof."""

    return log_size_floor if log_size_floor > 0 else DEFAULT_LOG_SIZE_FLOOR


def run_model(
    command: Sequence[str],
    *,
    stdout_path: Path,
    stderr_path: Path,
    cwd: Path | None,
    env: Mapping[str, str] | None,
    resolved_model: str | None,
    usage: UsageRecord | None,
    diff_path: Path | None,
    log_size_floor: int,
) -> InvocationResult:
    """Execute ``command`` with stdin closed and validate successful evidence."""

    stdout_path.parent.mkdir(parents=True, exist_ok=True)
    stderr_path.parent.mkdir(parents=True, exist_ok=True)
    child_env = dict(env) if env is not None else None
    with stdout_path.open("wb") as stdout_file, stderr_path.open("wb") as stderr_file:
        try:
            completed = subprocess.run(
                list(command), stdin=subprocess.DEVNULL, stdout=stdout_file,
                stderr=stderr_file, cwd=cwd, env=child_env, check=False,
            )
        except FileNotFoundError as exc:
            stderr_file.write(str(exc).encode("utf-8", errors="replace"))
            return InvocationResult(
                returncode=3, stdout_path=stdout_path, stderr_path=stderr_path,
                resolved_model=resolved_model, usage=usage,
            )

    for attribute, path in (("stdout", stdout_path), ("stderr", stderr_path)):
        payload = getattr(completed, attribute, None)
        if path.stat().st_size == 0 and isinstance(payload, (bytes, bytearray)):
            path.write_bytes(bytes(payload))

    if completed.returncode == 0 and diff_path is not None and cwd is not None:
        refresh_diff(diff_path, cwd)

    result = InvocationResult(
        returncode=completed.returncode, stdout_path=stdout_path, stderr_path=stderr_path,
        resolved_model=resolved_model, usage=usage,
    )
    if result.ok:
        size = stdout_path.stat().st_size + stderr_path.stat().st_size
        floor = _effective_floor(log_size_floor)
        if size < floor:
            raise AdapterError(f"successful invocation log below floor: {size} < {floor} bytes")
        if diff_path is None or not diff_path.exists() or diff_path.stat().st_size == 0:
            raise AdapterError("successful invocation produced an empty diff")
    return result
