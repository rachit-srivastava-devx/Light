"""Typed capability probing for model adapters."""

from __future__ import annotations

from dataclasses import dataclass

from .base import Adapter


@dataclass(frozen=True)
class CapabilityReport:
    adapter: str
    non_interactive_invoke: bool
    output_files_and_exit_code: bool
    closable_stdin: bool
    resolved_model_readback: bool
    local_usage_record: bool
    operator_credentials: bool

    @property
    def usable_as_builder(self) -> bool:
        return all((
            self.non_interactive_invoke,
            self.output_files_and_exit_code,
            self.closable_stdin,
            self.local_usage_record,
            self.operator_credentials,
        ))

    @property
    def usable_as_verifier(self) -> bool:
        return self.usable_as_builder and self.resolved_model_readback

    @property
    def builder_eligible(self) -> bool:
        return self.usable_as_builder

    @property
    def verifier_eligible(self) -> bool:
        return self.usable_as_verifier

    @property
    def missing(self) -> tuple[str, ...]:
        names = (
            "non_interactive_invoke",
            "output_files_and_exit_code",
            "closable_stdin",
            "resolved_model_readback",
            "local_usage_record",
            "operator_credentials",
        )
        return tuple(name for name in names if not getattr(self, name))


def probe_adapter(adapter: Adapter) -> CapabilityReport:
    """Inspect the six contract capabilities without making a model call."""

    def has_resolved_model_readback() -> bool:
        method = getattr(adapter, "resolved_model", None)
        if not callable(method):
            return False
        # A fresh adapter has not served a request yet, so it has no resolved
        # identity to return. Adapters that read identity from their transcript
        # declare that capability; never echo the requested alias as evidence.
        if getattr(adapter, "supports_resolved_model_readback", False) is True:
            return True
        try:
            value = method()
            return isinstance(value, str) and bool(value.strip())
        except (OSError, RuntimeError, TypeError, ValueError):
            return False

    def has_operator_credentials() -> bool:
        method = getattr(adapter, "operator_credentials", None)
        if not callable(method):
            return False
        try:
            return method() is True
        except (OSError, RuntimeError, TypeError, ValueError):
            return False

    return CapabilityReport(
        adapter=getattr(adapter, "name", type(adapter).__name__),
        non_interactive_invoke=callable(getattr(adapter, "invoke", None))
        and callable(getattr(adapter, "command", None)),
        output_files_and_exit_code=callable(getattr(adapter, "invoke", None)),
        closable_stdin=callable(getattr(adapter, "invoke", None)),
        resolved_model_readback=has_resolved_model_readback(),
        local_usage_record=callable(getattr(adapter, "usage_record", None)),
        operator_credentials=has_operator_credentials(),
    )


probe = probe_adapter
