"""Shared, private subprocess mechanics for model adapters."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any, Mapping, Sequence

from .base import AdapterError, InvocationResult, UsageRecord


DEFAULT_LOG_SIZE_FLOOR = 40


def read_metadata(path: Path) -> tuple[str | None, UsageRecord | None]:
    """Read optional model/usage metadata without making a second model call."""

    try:
        raw = path.read_text(encoding="utf-8")
    except OSError:
        return None, None

    documents: list[dict[str, Any]] = []
    for candidate in (raw, *raw.splitlines()):
        try:
            value = json.loads(candidate)
        except (TypeError, ValueError):
            continue
        if isinstance(value, dict):
            documents.append(value)

    model: str | None = None
    usage: UsageRecord | None = None
    for document in documents:
        for key in ("resolved_model", "resolvedModel", "model"):
            value = document.get(key)
            if isinstance(value, str) and value.strip():
                model = value.strip()
                break
        raw_usage = document.get("usage")
        if isinstance(raw_usage, dict):
            values = {
                "input_tokens": raw_usage.get("input_tokens", raw_usage.get("inputTokens")),
                "output_tokens": raw_usage.get("output_tokens", raw_usage.get("outputTokens")),
                "total_tokens": raw_usage.get("total_tokens", raw_usage.get("totalTokens")),
            }
            if any(isinstance(value, int) and not isinstance(value, bool) for value in values.values()):
                usage = UsageRecord(**{
                    key: value if isinstance(value, int) and not isinstance(value, bool) else None
                    for key, value in values.items()
                })
    return model, usage


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
    """Execute a CLI with closed stdin and validate successful evidence."""

    stdout_path.parent.mkdir(parents=True, exist_ok=True)
    stderr_path.parent.mkdir(parents=True, exist_ok=True)
    child_env = dict(env) if env is not None else None
    with stdout_path.open("wb") as stdout_file, stderr_path.open("wb") as stderr_file:
        try:
            completed = subprocess.run(
                list(command),
                stdin=subprocess.DEVNULL,
                stdout=stdout_file,
                stderr=stderr_file,
                cwd=cwd,
                env=child_env,
                check=False,
            )
        except FileNotFoundError as exc:
            stderr_file.write(str(exc).encode("utf-8", errors="replace"))
            return InvocationResult(
                returncode=3,
                stdout_path=stdout_path,
                stderr_path=stderr_path,
                resolved_model=resolved_model,
                usage=usage,
            )

    # Mocks commonly return captured streams instead of writing file handles;
    # accepting those streams keeps the file-backed contract testable without
    # weakening the real subprocess path above.
    for attribute, path in (("stdout", stdout_path), ("stderr", stderr_path)):
        payload = getattr(completed, attribute, None)
        if path.stat().st_size == 0 and isinstance(payload, (bytes, bytearray)):
            path.write_bytes(bytes(payload))

    # Refresh the evidence file after the CLI exits, before validating
    # success, so a pre-seeded placeholder cannot satisfy the diff invariant.
    if completed.returncode == 0 and diff_path is not None and cwd is not None:
        try:
            diff = subprocess.run(
                ["git", "-C", str(cwd), "diff"],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
        except FileNotFoundError as exc:
            raise AdapterError(f"unable to measure git diff: {exc}") from exc
        if diff.returncode == 0 and isinstance(diff.stdout, (bytes, bytearray)):
            diff_path.parent.mkdir(parents=True, exist_ok=True)
            diff_path.write_bytes(bytes(diff.stdout))

    result = InvocationResult(
        returncode=completed.returncode,
        stdout_path=stdout_path,
        stderr_path=stderr_path,
        resolved_model=resolved_model,
        usage=usage,
    )
    if result.ok:
        size = stdout_path.stat().st_size + stderr_path.stat().st_size
        if size < log_size_floor:
            raise AdapterError(
                f"successful invocation log below floor: {size} < {log_size_floor} bytes"
            )
        if diff_path is None or not diff_path.exists() or diff_path.stat().st_size == 0:
            raise AdapterError("successful invocation produced an empty diff")
    return result
