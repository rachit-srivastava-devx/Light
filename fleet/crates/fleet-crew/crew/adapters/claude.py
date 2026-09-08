"""Adapter for the operator's ``claude`` CLI."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path

from ._cli_adapter import invoke_cli
from .records import InvocationResult, UsageRecord
from .runner import DEFAULT_LOG_SIZE_FLOOR


class ClaudeAdapter:
    name = "claude"
    supports_resolved_model_readback = True

    def __init__(
        self, *, model: str | None = None, log_size_floor: int = DEFAULT_LOG_SIZE_FLOOR
    ) -> None:
        self.model = model
        self.log_size_floor = log_size_floor
        self._last_usage: UsageRecord | None = None
        # The requested model is not evidence of what the CLI resolved.
        # Readback is populated only from the invocation transcript.
        self._resolved_model: str | None = None

    def command(self, prompt: str) -> Sequence[str]:
        command = ["claude", "-p"]
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
        return invoke_cli(
            self, prompt, stdout_path=stdout_path, stderr_path=stderr_path,
            cwd=cwd, env=env, diff_path=diff_path,
        )

    def resolved_model(self) -> str | None:
        return self._resolved_model

    def usage_record(self) -> UsageRecord | None:
        return self._last_usage

    def operator_credentials(self) -> bool:
        return True
