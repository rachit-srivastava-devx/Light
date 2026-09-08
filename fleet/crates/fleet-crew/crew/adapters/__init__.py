"""Model adapter implementations.

This package is the only place in the crew package that may invoke a model
process.  Callers receive typed results and capability reports rather than
handling subprocess details themselves.
"""

from .capability_probe import probe_adapter
from .capability_report import CapabilityReport
from .claude import ClaudeAdapter
from .codex import CodexAdapter
from .errors import AdapterError
from .protocol import Adapter
from .records import InvocationResult, UsageRecord

__all__ = [
    "Adapter",
    "AdapterError",
    "CapabilityReport",
    "ClaudeAdapter",
    "CodexAdapter",
    "InvocationResult",
    "UsageRecord",
    "probe_adapter",
]
