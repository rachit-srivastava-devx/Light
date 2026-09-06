"""SQLite-backed persistence for the context pack's `recent_tasks[]` (docs/BUILD-DIGEST.md §2).

Zero new runtime dependency (C3): this uses only Python's stdlib `sqlite3`, so no ADR is required.
One table, keyed by `(tenant_id, user_id)`, capped at `MAX_RECENT_TASKS_PER_USER` rows per user with
prune-oldest-on-insert — the digest's "recent_tasks[] last 50".

Determinism: following the injected-timestamp pattern used by `apps/mobile/src/session/
StateMachine.ts` and `cognitive/Policy.ts`, this module never reads the wall clock itself.
`insert_task` takes `created_at` as a parameter; the caller (the FastAPI route handler) supplies
`time.time()`. That keeps the store itself a pure function of its inputs and testable without
monkeypatching a clock.

This does not fabricate ML embeddings. `RecentTaskRecord` has no `embedding` field — the caller
(`app.py`'s `session_warmup`) is responsible for deciding what, if anything, to put in the context
pack's embedding slot, and per the build task that is deliberately left empty (honest lexical-only
retrieval on the client) rather than faked.
"""

from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from pathlib import Path

from ..proxy.schemas import AtomizerOutput

# BUILD-DIGEST §2: "recent_tasks[] (last 50, ~25KB)".
MAX_RECENT_TASKS_PER_USER = 50

_CREATE_TABLE_SQL = """
CREATE TABLE IF NOT EXISTS recent_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    task_text TEXT NOT NULL,
    steps_json TEXT NOT NULL,
    created_at REAL NOT NULL
)
"""

_CREATE_INDEX_SQL = """
CREATE INDEX IF NOT EXISTS idx_recent_tasks_tenant_user_created
ON recent_tasks (tenant_id, user_id, created_at DESC, id DESC)
"""

_PRUNE_SQL = """
DELETE FROM recent_tasks
WHERE tenant_id = ? AND user_id = ? AND id NOT IN (
    SELECT id FROM recent_tasks
    WHERE tenant_id = ? AND user_id = ?
    ORDER BY created_at DESC, id DESC
    LIMIT ?
)
"""


@dataclass(frozen=True)
class RecentTaskRecord:
    task_text: str
    steps: AtomizerOutput
    created_at: float


class ContextStore:
    """One table, per-`(tenant_id, user_id)` recent-tasks history.

    Each public method opens and closes its own connection: the relay is stateless and low-QPS on
    this path (an atomize call and an app-open warmup, not a hot loop), so pooling would be
    premature complexity for no measured benefit.
    """

    def __init__(self, db_path: str | Path) -> None:
        self._db_path = Path(db_path)
        self._db_path.parent.mkdir(parents=True, exist_ok=True)
        with self._connect() as conn:
            conn.execute(_CREATE_TABLE_SQL)
            conn.execute(_CREATE_INDEX_SQL)

    def _connect(self) -> sqlite3.Connection:
        conn = sqlite3.connect(self._db_path)
        conn.execute("PRAGMA foreign_keys = ON")
        return conn

    def insert_task(
        self,
        *,
        tenant_id: str,
        user_id: str,
        task_text: str,
        steps: AtomizerOutput,
        created_at: float,
    ) -> None:
        """Insert one task+steps row, then prune this user's history down to the last
        `MAX_RECENT_TASKS_PER_USER` rows by `created_at` (oldest dropped first).
        """
        steps_json = steps.model_dump_json()
        with self._connect() as conn:
            conn.execute(
                "INSERT INTO recent_tasks (tenant_id, user_id, task_text, steps_json, created_at)"
                " VALUES (?, ?, ?, ?, ?)",
                (tenant_id, user_id, task_text, steps_json, created_at),
            )
            conn.execute(
                _PRUNE_SQL,
                (tenant_id, user_id, tenant_id, user_id, MAX_RECENT_TASKS_PER_USER),
            )
            conn.commit()

    def recent_tasks(
        self,
        *,
        tenant_id: str,
        user_id: str,
        limit: int = MAX_RECENT_TASKS_PER_USER,
    ) -> list[RecentTaskRecord]:
        """Most-recent-first, scoped strictly to `(tenant_id, user_id)` — a different tenant or a
        different user under the same tenant never sees another user's rows.
        """
        with self._connect() as conn:
            conn.row_factory = sqlite3.Row
            rows = conn.execute(
                "SELECT task_text, steps_json, created_at FROM recent_tasks"
                " WHERE tenant_id = ? AND user_id = ?"
                " ORDER BY created_at DESC, id DESC"
                " LIMIT ?",
                (tenant_id, user_id, limit),
            ).fetchall()

        return [
            RecentTaskRecord(
                task_text=row["task_text"],
                steps=AtomizerOutput.model_validate_json(row["steps_json"]),
                created_at=row["created_at"],
            )
            for row in rows
        ]
