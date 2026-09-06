"""FreezeStore — the append-only ledger for lld.v1 FreezeRecords (Speed-of-Thought P0 contract
§1.5, docs/SPEED-OF-THOUGHT-P0-CONTRACT.md).

Deliberately modelled on `conversation_store.py`'s ConversationStore: same sqlite backing, same
(tenant_id, user_id, session_id) scoping, same injected `now` (no clock read inside, so replay is
bit-identical). It deliberately does NOT copy ConversationStore's `_PRUNE_SQL` — a freeze ledger
that forgets its 25th freeze destroys the record of why a spec changed — and it exposes no
update/delete/edit method at all. Supersede is expressed only as a new row whose `supersedes`
names the prior `freeze_id`; there is no code path that mutates or removes a stamped freeze.
"""

from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from pathlib import Path

_CREATE_TABLE_SQL = """
CREATE TABLE IF NOT EXISTS freeze_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    freeze_id TEXT NOT NULL UNIQUE,
    node_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    supersedes TEXT,
    payload TEXT NOT NULL,
    created_at REAL NOT NULL
)
"""

_CREATE_INDEX_SQL = """
CREATE INDEX IF NOT EXISTS idx_freeze_records_scope_order
ON freeze_records (tenant_id, user_id, session_id, created_at ASC, id ASC)
"""

_CREATE_HASH_INDEX_SQL = """
CREATE INDEX IF NOT EXISTS idx_freeze_records_content_hash
ON freeze_records (tenant_id, user_id, content_hash)
"""


@dataclass(frozen=True)
class FreezeRow:
    freeze_id: str
    node_id: str
    version: int
    content_hash: str
    supersedes: str | None
    payload: str
    created_at: float


def _row_to_freeze(row: sqlite3.Row) -> FreezeRow:
    return FreezeRow(
        freeze_id=row["freeze_id"],
        node_id=row["node_id"],
        version=row["version"],
        content_hash=row["content_hash"],
        supersedes=row["supersedes"],
        payload=row["payload"],
        created_at=row["created_at"],
    )


class FreezeStore:
    """Durable, append-only ledger of stamped FreezeRecords, isolated by tenant/user/session.

    No `update`, `delete`, `edit`, `set_superseded`, or `prune` method exists on this class — that
    absence is the structural half of "you cannot edit a freeze in place" (§1.5); the other half is
    that `ModuleBrief`/`FreezeRecord` themselves carry no proposer-writable authority field
    (contract §2.1). A supersede is recorded as a brand-new row whose `supersedes` names the prior
    `freeze_id`; the prior row is never touched.
    """

    def __init__(self, db_path: str | Path) -> None:
        self._db_path = Path(db_path)
        self._db_path.parent.mkdir(parents=True, exist_ok=True)
        with self._connect() as conn:
            conn.execute(_CREATE_TABLE_SQL)
            conn.execute(_CREATE_INDEX_SQL)
            conn.execute(_CREATE_HASH_INDEX_SQL)

    def _connect(self) -> sqlite3.Connection:
        conn = sqlite3.connect(self._db_path)
        conn.execute("PRAGMA foreign_keys = ON")
        return conn

    def append_freeze(
        self,
        *,
        tenant_id: str,
        user_id: str,
        session_id: str,
        freeze_id: str,
        node_id: str,
        version: int,
        content_hash: str,
        supersedes: str | None,
        payload: str,
        created_at: float,
    ) -> None:
        with self._connect() as conn:
            conn.execute(
                "INSERT INTO freeze_records"
                " (tenant_id, user_id, session_id, freeze_id, node_id, version, content_hash,"
                "  supersedes, payload, created_at)"
                " VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    tenant_id,
                    user_id,
                    session_id,
                    freeze_id,
                    node_id,
                    version,
                    content_hash,
                    supersedes,
                    payload,
                    created_at,
                ),
            )
            conn.commit()

    def list_freezes(
        self,
        *,
        tenant_id: str,
        user_id: str,
        session_id: str,
    ) -> list[FreezeRow]:
        """Every freeze ever stamped for this scope, oldest first. No `limit` — the whole point of
        an append-only ledger is that nothing here is ever too old to see (§1.5's no-prune rule).
        """
        with self._connect() as conn:
            conn.row_factory = sqlite3.Row
            rows = conn.execute(
                "SELECT freeze_id, node_id, version, content_hash, supersedes, payload, created_at"
                " FROM freeze_records"
                " WHERE tenant_id = ? AND user_id = ? AND session_id = ?"
                " ORDER BY created_at ASC, id ASC",
                (tenant_id, user_id, session_id),
            ).fetchall()
        return [_row_to_freeze(row) for row in rows]

    def get_by_content_hash(
        self,
        *,
        tenant_id: str,
        user_id: str,
        content_hash: str,
    ) -> FreezeRow | None:
        """The freeze (if any) whose `content_hash` already matches — lets a caller recognise "this
        exact decision was already frozen" without re-deriving it, scoped to the tenant/user (not
        session, since the same brief can recur across sessions for the same user).
        """
        with self._connect() as conn:
            conn.row_factory = sqlite3.Row
            row = conn.execute(
                "SELECT freeze_id, node_id, version, content_hash, supersedes, payload, created_at"
                " FROM freeze_records"
                " WHERE tenant_id = ? AND user_id = ? AND content_hash = ?"
                " ORDER BY created_at ASC, id ASC LIMIT 1",
                (tenant_id, user_id, content_hash),
            ).fetchone()
        return _row_to_freeze(row) if row is not None else None
