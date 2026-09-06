"""Independent, evidence-only evaluator for human-simulation E2E traces."""

from .checks import Evaluation, Finding, evaluate
from .trace_schema import Trace, TraceEvent, load_jsonl

__all__ = ["Evaluation", "Finding", "Trace", "TraceEvent", "evaluate", "load_jsonl"]
