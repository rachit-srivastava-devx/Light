"""Model adapter implementations.

This package is the only place in the crew package that may invoke a model
process.  Callers receive typed results and capability reports rather than
handling subprocess details themselves.
"""

from .base import Adapter, AdapterError, InvocationResult, UsageRecord
from .capability import CapabilityReport, probe_adapter
from .claude import ClaudeAdapter
from .codex import CodexAdapter

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
