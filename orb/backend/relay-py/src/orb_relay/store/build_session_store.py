"""BuildSessionStore -- append-only evidence log for build-mode belief registers (F04 §3.3).

Deliberately modelled on `freeze_store.py`'s discipline, in the same SQLite file
(`ORB_CONTEXT_DB_PATH`): no `update`, `delete`, `edit`, or `prune` method exists on this class. The
registers are `fold(log)` (`cognitive/belief.py`'s `fold`), so mutating a stored row would make the
current state unreproducible from its own history -- exactly the property an append-only,
replay-derived store exists to prevent losing.
"""

from __future__ import annotations

import json
import sqlite3
from pathlib import Path

from ..cognitive.belief import (
    CoverageSlot,
    Evidence,
    EvidenceTier,
    PremiseRevised,
    Register,
    Registers,
    fold,
)

_CREATE_TABLE_SQL = """
CREATE TABLE IF NOT EXISTS build_evidence (
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    PRIMARY KEY (tenant_id, user_id, session_id, seq)
)
"""

_CREATE_INDEX_SQL = """
CREATE INDEX IF NOT EXISTS idx_build_evidence_scope_order
ON build_evidence (tenant_id, user_id, session_id, seq ASC)
"""

_KIND_EVIDENCE = "evidence"
_KIND_PREMISE_REVISED = "premise_revised"


def _encode(entry: Evidence | PremiseRevised) -> tuple[str, str]:
    if isinstance(entry, Evidence):
        evidence_payload: dict[str, object] = {
            "register": entry.register.value,
            "slot": entry.slot.value if entry.slot is not None else None,
            "weight": entry.weight,
            "reliability": entry.reliability,
            "tier": entry.tier.value,
        }
        return _KIND_EVIDENCE, json.dumps(evidence_payload)
    if isinstance(entry, PremiseRevised):
        premise_payload: dict[str, object] = {
            "premise": entry.premise.value,
            "dependents": [slot.value for slot in entry.dependents],
        }
        return _KIND_PREMISE_REVISED, json.dumps(premise_payload)
    raise TypeError(f"unknown evidence-log entry type: {type(entry)!r}")


def _decode(kind: str, payload_json: str) -> Evidence | PremiseRevised:
    payload = json.loads(payload_json)
    if kind == _KIND_EVIDENCE:
        return Evidence(
            register=Register(payload["register"]),
            slot=CoverageSlot(payload["slot"]) if payload["slot"] is not None else None,
            weight=payload["weight"],
            reliability=payload["reliability"],
            tier=EvidenceTier(payload["tier"]),
        )
    if kind == _KIND_PREMISE_REVISED:
        return PremiseRevised(
            premise=CoverageSlot(payload["premise"]),
            dependents=tuple(CoverageSlot(s) for s in payload["dependents"]),
        )
    raise ValueError(f"unknown stored evidence-log kind: {kind!r}")


class BuildSessionStore:
    """Append-only evidence log for build-mode belief registers.

    No `update`, `delete`, `edit`, or `prune` method exists on this class -- the registers are a
    fold over the log (see `cognitive/belief.py:fold`), so mutating a row would make the current
    state unreproducible from its own history. Same discipline as `FreezeStore`, same db file.
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

    def append_evidence(
        self,
        *,
        tenant_id: str,
        user_id: str,
        session_id: str,
        entry: Evidence | PremiseRevised,
    ) -> int:
        """Append one log entry, returning its assigned `seq` (0-based, monotonic per session).

        `seq` is assigned HERE, by the store, never accepted from the caller (§3.3) -- two
        concurrent turns racing on a caller-supplied index is exactly the corruption an
        append-only, fold-derived log cannot tolerate.
        """
        kind, payload_json = _encode(entry)
        with self._connect() as conn:
            row = conn.execute(
                "SELECT COALESCE(MAX(seq), -1) FROM build_evidence"
                " WHERE tenant_id = ? AND user_id = ? AND session_id = ?",
                (tenant_id, user_id, session_id),
            ).fetchone()
            next_seq: int = row[0] + 1
            conn.execute(
                "INSERT INTO build_evidence"
                " (tenant_id, user_id, session_id, seq, kind, payload_json)"
                " VALUES (?, ?, ?, ?, ?, ?)",
                (tenant_id, user_id, session_id, next_seq, kind, payload_json),
            )
            conn.commit()
        return next_seq

    def log(self, *, tenant_id: str, user_id: str, session_id: str) -> list[Evidence | PremiseRevised]:
        """This session's full evidence log, oldest first. No `limit` -- `fold` needs every entry,
        and this ledger never prunes (same discipline as `FreezeStore`).
        """
        with self._connect() as conn:
            conn.row_factory = sqlite3.Row
            rows = conn.execute(
                "SELECT kind, payload_json FROM build_evidence"
                " WHERE tenant_id = ? AND user_id = ? AND session_id = ?"
                " ORDER BY seq ASC",
                (tenant_id, user_id, session_id),
            ).fetchall()
        return [_decode(row["kind"], row["payload_json"]) for row in rows]

    def registers(self, *, tenant_id: str, user_id: str, session_id: str) -> Registers:
        """The derived current state -- exactly `fold(log(...))`, never a second implementation."""
        return fold(self.log(tenant_id=tenant_id, user_id=user_id, session_id=session_id))
