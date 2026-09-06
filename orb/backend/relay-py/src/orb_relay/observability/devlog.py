"""Dev-only structured event log — the Python side of the unified dev-logs/ pipeline.

Every backend/mobile process in this product writes NDJSON lines to `dev-logs/<service>.ndjson`
at the product root; `tooling/dev-logs/watch.mjs` tails all of them and prints one merged,
formatted stream. This is a debugging aid for the dev rung only (ORB_DEV_LOGGING=0 disables it
outright) — it is not a telemetry pipeline and makes no production storage claim, same posture as
`observability/metrics.py`.

Best-effort by construction: a logging failure (disk full, permissions) must never break a request,
so every write is wrapped and swallowed.
"""

from __future__ import annotations

import json
import os
import threading
import time
from pathlib import Path
from typing import Any

DEV_LOGGING = os.environ.get("ORB_DEV_LOGGING", "1") != "0"

# backend/relay-py/src/orb_relay/observability/devlog.py -> product root is 5 parents up.
_PRODUCT_ROOT = Path(__file__).resolve().parents[5]
_DEFAULT_LOG_DIR = _PRODUCT_ROOT / "dev-logs"
LOG_DIR = Path(os.environ.get("ORB_DEV_LOG_DIR", str(_DEFAULT_LOG_DIR)))

_lock = threading.Lock()


def truncate(value: str, limit: int = 300) -> str:
    if len(value) <= limit:
        return value
    return f"{value[:limit]}…(+{len(value) - limit} chars)"


def dev_log(event: str, *, level: str = "info", service: str = "relay-py", **fields: Any) -> None:
    """Append one NDJSON event. Never raises — a broken dev log must not break the request path."""
    if not DEV_LOGGING:
        return
    record = {"ts": time.time(), "service": service, "level": level, "event": event, **fields}
    try:
        line = json.dumps(record, default=str)
    except (TypeError, ValueError):
        return
    try:
        with _lock:
            LOG_DIR.mkdir(parents=True, exist_ok=True)
            path = LOG_DIR / f"{_safe_service_name(service)}.ndjson"
            with path.open("a", encoding="utf-8") as fh:
                fh.write(line + "\n")
    except OSError:
        pass


def _safe_service_name(service: str) -> str:
    cleaned = "".join(c for c in service if c.isalnum() or c in ("-", "_")).strip("-_")
    return cleaned or "unknown"
