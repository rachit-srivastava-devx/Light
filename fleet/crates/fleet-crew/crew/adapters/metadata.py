"""Optional resolved-model/usage metadata scraping from a transcript."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .records import UsageRecord


def read_metadata(path: Path) -> tuple[str | None, UsageRecord | None]:
    """Read optional model/usage metadata without making a second model call.

    Never raises: a file that doesn't exist, isn't UTF-8, or contains no
    parseable JSON object yields ``(None, None)`` -- absence of metadata is an
    expected outcome for adapters that don't emit any, not an error. (A
    non-UTF-8 transcript byte previously escaped this contract by raising
    ``UnicodeDecodeError`` out of ``path.read_text``, uncaught by the
    ``except OSError`` below -- it is now caught explicitly.)
    """

    try:
        raw = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return None, None

    documents: list[dict[str, Any]] = []
    for candidate in (raw, *raw.splitlines()):
        try:
            value = json.loads(candidate)
        except (TypeError, ValueError):
            continue
        if isinstance(value, dict):
            documents.append(value)

    model: str | None = None
    usage: UsageRecord | None = None
    for document in documents:
        for key in ("resolved_model", "resolvedModel", "model"):
            value = document.get(key)
            if isinstance(value, str) and value.strip():
                model = value.strip()
                break
        raw_usage = document.get("usage")
        if isinstance(raw_usage, dict):
            values = {
                "input_tokens": raw_usage.get("input_tokens", raw_usage.get("inputTokens")),
                "output_tokens": raw_usage.get("output_tokens", raw_usage.get("outputTokens")),
                "total_tokens": raw_usage.get("total_tokens", raw_usage.get("totalTokens")),
            }
            if any(isinstance(v, int) and not isinstance(v, bool) for v in values.values()):
                usage = UsageRecord(**{
                    key: value if isinstance(value, int) and not isinstance(value, bool) else None
                    for key, value in values.items()
                })
    return model, usage
