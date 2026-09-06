"""Stable JSONL evidence output for replay runs."""

from __future__ import annotations

from dataclasses import dataclass, field
import json
from pathlib import Path
from typing import Any, TextIO


class EventWriter:
    """Small interface used by the replay engine without a logging dependency."""

    def emit(self, event: str, **fields: Any) -> None:
        raise NotImplementedError


@dataclass
class JsonlEventWriter(EventWriter):
    """Write one deterministic, flush-on-write JSON object per line."""

    stream: TextIO
    schema: str = "focus-orb.replay.event.v1"
    _event_index: int = field(default=0, init=False)

    @classmethod
    def from_path(cls, path: str | Path) -> "JsonlEventWriter":
        return cls(Path(path).open("w", encoding="utf-8", newline="\n"))

    def emit(self, event: str, **fields: Any) -> None:
        if not event or not isinstance(event, str):
            raise ValueError("event must be a non-empty string")
        reserved = {"schema", "event_index", "event"} & fields.keys()
        if reserved:
            raise ValueError(f"reserved event fields cannot be overridden: {sorted(reserved)}")
        record = {
            "schema": self.schema,
            "event_index": self._event_index,
            "event": event,
            **fields,
        }
        self._event_index += 1
        self.stream.write(json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
        self.stream.write("\n")
        self.stream.flush()


@dataclass
class MemoryEventWriter(EventWriter):
    """In-memory writer useful for unit tests and callers building a report."""

    records: list[dict[str, Any]] = field(default_factory=list)

    def emit(self, event: str, **fields: Any) -> None:
        self.records.append({"event_index": len(self.records), "event": event, **fields})
