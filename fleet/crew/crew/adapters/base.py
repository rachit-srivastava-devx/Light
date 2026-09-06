"""Common adapter contract and subprocess result types."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Protocol, Sequence, runtime_checkable


class AdapterError(RuntimeError):
    """A model invocation failed an adapter invariant."""

    exit_code = 6


@dataclass(frozen=True)
class UsageRecord:
    """Local, integer-only usage accounting emitted by an adapter."""

    input_tokens: int | None = None
    output_tokens: int | None = None
    total_tokens: int | None = None

    def __post_init__(self) -> None:
        for name in ("input_tokens", "output_tokens", "total_tokens"):
            value = getattr(self, name)
            if value is not None and (isinstance(value, bool) or not isinstance(value, int) or value < 0):
                raise ValueError(f"{name} must be a non-negative integer or absent")


@dataclass(frozen=True)
class InvocationResult:
    """Evidence from one non-interactive model invocation."""

    returncode: int
    stdout_path: Path
    stderr_path: Path
    resolved_model: str | None
    usage: UsageRecord | None

    @property
    def ok(self) -> bool:
        return self.returncode == 0

    @property
    def exit_code(self) -> int:
        """Contract vocabulary alias for the process return code."""

        return self.returncode


@runtime_checkable
class Adapter(Protocol):
    """Portability contract for an operator-owned model adapter.

    The six capabilities are represented explicitly by this protocol:
    invocation, file-backed output/status, closed stdin, resolved-model
    readback, local usage, and operator-owned credentials.
    """

    name: str

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
        """Run one model request with stdin connected to ``/dev/null``."""
        ...

    def resolved_model(self) -> str | None:
        """Return the model selected by the operator's CLI, if supported."""
        ...

    def usage_record(self) -> UsageRecord | None:
        """Return local usage data, never data fetched from fleet state."""
        ...

    def operator_credentials(self) -> bool:
        """Whether credentials are sourced from the operator environment."""
        ...

    def command(self, prompt: str) -> Sequence[str]:
        """Build the non-interactive command without executing it."""
        ...
