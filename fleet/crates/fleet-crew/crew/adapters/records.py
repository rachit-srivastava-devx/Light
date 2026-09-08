"""Evidence and usage value types shared by every adapter."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class UsageRecord:
    """Local, integer-only usage accounting emitted by one adapter invocation.

    Every field is ``None`` (unmeasured) or a non-negative ``int`` -- never a
    float, never negative, and never a ``bool`` (a subclass of ``int`` in
    Python, explicitly excluded so a caller's stray boolean can't masquerade
    as a token count).
    """

    input_tokens: int | None = None
    output_tokens: int | None = None
    total_tokens: int | None = None

    def __post_init__(self) -> None:
        for name in ("input_tokens", "output_tokens", "total_tokens"):
            value = getattr(self, name)
            if value is not None and (
                isinstance(value, bool) or not isinstance(value, int) or value < 0
            ):
                raise ValueError(f"{name} must be a non-negative integer or absent")


@dataclass(frozen=True)
class InvocationResult:
    """Evidence from one non-interactive model invocation -- always file-backed."""

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
