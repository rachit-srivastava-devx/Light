"""Versioned JSONL trace contract and defensive loader.

This module deliberately has no dependency on the app. A trace is an exported
evidence file produced by another system, and malformed evidence is reported
instead of making the evaluator crash.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
import json
from pathlib import Path
import re
from typing import Any, Iterable

SCHEMA_VERSION = "1.0"
REQUIRED_FIELDS = ("ts", "seq", "source", "event", "data")
SENSITIVE_KEY = re.compile(r"(token|secret|password|authorization|api[_-]?key|credential)", re.I)


@dataclass(frozen=True)
class TraceEvent:
    line: int
    ts: Any
    seq: int
    source: str
    event: str
    data: dict[str, Any]
    run_id: str | None = None
    session_id: str | None = None

    @classmethod
    def from_mapping(cls, raw: dict[str, Any], line: int) -> tuple["TraceEvent | None", list[str]]:
        errors: list[str] = []
        missing = [name for name in REQUIRED_FIELDS if name not in raw]
        if missing:
            errors.append(f"line {line}: missing required field(s): {', '.join(missing)}")
            return None, errors
        if not isinstance(raw["seq"], int) or isinstance(raw["seq"], bool):
            errors.append(f"line {line}: seq must be an integer")
        if not isinstance(raw["source"], str) or not raw["source"].strip():
            errors.append(f"line {line}: source must be a non-empty string")
        if not isinstance(raw["event"], str) or not raw["event"].strip():
            errors.append(f"line {line}: event must be a non-empty string")
        if not isinstance(raw["data"], dict):
            errors.append(f"line {line}: data must be an object")
        if errors:
            return None, errors
        return cls(
            line=line,
            ts=raw["ts"],
            seq=raw["seq"],
            source=raw["source"],
            event=raw["event"],
            data=raw["data"],
            run_id=raw.get("run_id"),
            session_id=raw.get("session_id"),
        ), errors

    def timestamp_ms(self) -> float | None:
        """Return an event timestamp in milliseconds when it is usable."""
        if isinstance(self.ts, (int, float)) and not isinstance(self.ts, bool):
            return float(self.ts)
        if not isinstance(self.ts, str):
            return None
        value = self.ts.strip()
        try:
            parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
        except ValueError:
            return None
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=timezone.utc)
        return parsed.timestamp() * 1000


@dataclass
class Trace:
    events: list[TraceEvent] = field(default_factory=list)
    schema_errors: list[str] = field(default_factory=list)
    source_path: str | None = None

    @property
    def ordered_events(self) -> list[TraceEvent]:
        return sorted(self.events, key=lambda item: (item.timestamp_ms() is None, item.timestamp_ms() or 0, item.seq))

    def evidence(self) -> dict[str, Any]:
        """Create bounded, redacted evidence safe to send to optional judges."""
        return {
            "schema_version": SCHEMA_VERSION,
            "source_path": self.source_path,
            "schema_errors": list(self.schema_errors),
            "events": [
                {
                    "line": event.line,
                    "ts": event.ts,
                    "seq": event.seq,
                    "source": event.source,
                    "event": event.event,
                    "run_id": event.run_id,
                    "session_id": event.session_id,
                    "data": _redact(event.data),
                }
                for event in self.ordered_events
            ],
        }


def _redact(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: "[REDACTED]" if SENSITIVE_KEY.search(key) else _redact(item) for key, item in value.items()}
    if isinstance(value, list):
        return [_redact(item) for item in value]
    return value


def load_jsonl(path: str | Path, *, max_events: int = 100_000) -> Trace:
    trace = Trace(source_path=str(path))
    try:
        handle = open(path, "r", encoding="utf-8")
    except OSError as exc:
        trace.schema_errors.append(f"cannot read trace: {exc}")
        return trace
    with handle:
        for line_number, raw_line in enumerate(handle, start=1):
            if len(trace.events) >= max_events:
                trace.schema_errors.append(f"trace exceeds max_events={max_events}; remaining lines ignored")
                break
            if not raw_line.strip():
                continue
            try:
                payload = json.loads(raw_line)
            except json.JSONDecodeError as exc:
                trace.schema_errors.append(f"line {line_number}: invalid JSON: {exc.msg}")
                continue
            if not isinstance(payload, dict):
                trace.schema_errors.append(f"line {line_number}: JSONL record must be an object")
                continue
            event, errors = TraceEvent.from_mapping(payload, line_number)
            trace.schema_errors.extend(errors)
            if event is not None:
                trace.events.append(event)
    return trace


def events_of(trace: Trace, *names: str) -> list[TraceEvent]:
    wanted = set(names)
    return [event for event in trace.ordered_events if event.event in wanted]


def first_value(data: dict[str, Any], *keys: str) -> Any:
    for key in keys:
        if key in data:
            return data[key]
    return None
