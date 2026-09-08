"""Portability contract every model adapter in this package satisfies."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Protocol, runtime_checkable

from .records import InvocationResult, UsageRecord


@runtime_checkable
class Adapter(Protocol):
    """The six-capability contract ``probe_adapter`` checks: non-interactive
    invocation, file-backed output/exit-code, closed stdin, resolved-model
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
        """The model the operator's CLI actually served, read from ITS OWN
        transcript -- never the requested alias echoed back."""
        ...

    def usage_record(self) -> UsageRecord | None:
        """Return local usage data, never data fetched from fleet state."""
        ...

    def operator_credentials(self) -> bool:
        """Whether credentials are sourced from the operator's own environment."""
        ...

    def command(self, prompt: str) -> Sequence[str]:
        """Build the non-interactive argv without executing it."""
        ...
