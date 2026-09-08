"""The eligibility report ``probe_adapter`` produces."""

from __future__ import annotations

from dataclasses import dataclass


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
