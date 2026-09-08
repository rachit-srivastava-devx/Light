"""Probe the six adapter capabilities without making a model call."""

from __future__ import annotations

from .capability_report import CapabilityReport
from .protocol import Adapter


def _has_resolved_model_readback(adapter: Adapter) -> bool:
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


def _has_operator_credentials(adapter: Adapter) -> bool:
    method = getattr(adapter, "operator_credentials", None)
    if not callable(method):
        return False
    try:
        return method() is True
    except (OSError, RuntimeError, TypeError, ValueError):
        return False


def probe_adapter(adapter: Adapter) -> CapabilityReport:
    """Inspect the six contract capabilities without making a model call."""

    return CapabilityReport(
        adapter=getattr(adapter, "name", type(adapter).__name__),
        non_interactive_invoke=callable(getattr(adapter, "invoke", None))
        and callable(getattr(adapter, "command", None)),
        output_files_and_exit_code=callable(getattr(adapter, "invoke", None)),
        closable_stdin=callable(getattr(adapter, "invoke", None)),
        resolved_model_readback=_has_resolved_model_readback(adapter),
        local_usage_record=callable(getattr(adapter, "usage_record", None)),
        operator_credentials=_has_operator_credentials(adapter),
    )


probe = probe_adapter
