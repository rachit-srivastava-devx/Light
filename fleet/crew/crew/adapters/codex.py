"""Adapter for the operator's ``codex exec`` CLI."""

from __future__ import annotations

from pathlib import Path
from typing import Mapping, Sequence

from ._subprocess import DEFAULT_LOG_SIZE_FLOOR, read_metadata, run_model
from .base import InvocationResult, UsageRecord


class CodexAdapter:
    name = "codex"
    supports_resolved_model_readback = True

    def __init__(self, *, model: str | None = None, log_size_floor: int = DEFAULT_LOG_SIZE_FLOOR) -> None:
        self.model = model
        self.log_size_floor = log_size_floor
        self._last_usage: UsageRecord | None = None
        # The requested model is not evidence of what the CLI resolved.
        # Readback is populated only from the invocation transcript.
        self._resolved_model: str | None = None

    def command(self, prompt: str) -> Sequence[str]:
        command = ["codex", "exec"]
        if self.model:
            command.extend(["--model", self.model])
        command.append(prompt)
        return command

    def invoke(
        self,
        prompt: str,
        *,
        stdout_path: Path,
        stderr_path: Path,
        cwd: Path | None = None,
        env: Mapping[str, str] | None = None,
        diff_path: Path | None = None,
    ) -> InvocationResult:
        result = run_model(
            self.command(prompt), stdout_path=stdout_path, stderr_path=stderr_path,
            cwd=cwd, env=env, resolved_model=self.resolved_model(), usage=self._last_usage,
            diff_path=diff_path, log_size_floor=self.log_size_floor,
        )
        model, usage = read_metadata(stdout_path)
        if model is not None:
            self._resolved_model = model
        if usage is not None:
            self._last_usage = usage
        return InvocationResult(
            returncode=result.returncode,
            stdout_path=result.stdout_path,
            stderr_path=result.stderr_path,
            resolved_model=self._resolved_model,
            usage=self._last_usage,
        )

    def resolved_model(self) -> str | None:
        return self._resolved_model

    def usage_record(self) -> UsageRecord | None:
        return self._last_usage

    def operator_credentials(self) -> bool:
        return True
