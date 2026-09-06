"""Bounded conversational memory for the relay's non-task response path.

The mobile client may reconnect between turns, so conversation context cannot live only in a
request-local variable. This store keeps a short, tenant/user/session-scoped transcript. The
caller owns truncation and prompt formatting; this module only provides durable, ordered turns.
"""

from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

ConversationRole = Literal["user", "assistant"]
MAX_CONVERSATION_TURNS = 24

_CREATE_TABLE_SQL = """
CREATE TABLE IF NOT EXISTS conversation_turns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    text TEXT NOT NULL,
    created_at REAL NOT NULL
)
"""

_CREATE_INDEX_SQL = """
CREATE INDEX IF NOT EXISTS idx_conversation_turns_scope_order
ON conversation_turns (tenant_id, user_id, session_id, created_at DESC, id DESC)
"""

_PRUNE_SQL = """
DELETE FROM conversation_turns
WHERE tenant_id = ? AND user_id = ? AND session_id = ? AND id NOT IN (
    SELECT id FROM conversation_turns
    WHERE tenant_id = ? AND user_id = ? AND session_id = ?
    ORDER BY created_at DESC, id DESC
    LIMIT ?
)
"""


@dataclass(frozen=True)
class ConversationTurn:
    role: ConversationRole
    text: str
    created_at: float


#: How long after the last word a NEW session still continues the previous one. Thirty minutes is
#: chosen against the failure it prevents: an app reload or a crash-and-reopen is seconds to minutes
#: old, while the stale-context bug this replaces came from resurrecting a task from an entirely
#: different sitting. It is a parameter on `recent()` so a caller can tighten or disable it.
CARRY_OVER_WINDOW_S = 30 * 60


def _turns_from(rows: object) -> list[ConversationTurn]:
    return [
        ConversationTurn(role=row["role"], text=row["text"], created_at=row["created_at"])
        for row in reversed(list(rows))  # type: ignore[call-overload]
    ]


class ConversationStore:
    """Durable, bounded memory isolated by tenant, user, and live session."""

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

    def append(
        self,
        *,
        tenant_id: str,
        user_id: str,
        session_id: str,
        role: ConversationRole,
        text: str,
        created_at: float,
    ) -> None:
        cleaned = text.strip()
        if not cleaned:
            return
        with self._connect() as conn:
            conn.execute(
                "INSERT INTO conversation_turns"
                " (tenant_id, user_id, session_id, role, text, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                (tenant_id, user_id, session_id, role, cleaned, created_at),
            )
            conn.execute(
                _PRUNE_SQL,
                (
                    tenant_id,
                    user_id,
                    session_id,
                    tenant_id,
                    user_id,
                    session_id,
                    MAX_CONVERSATION_TURNS,
                ),
            )
            conn.commit()

    def has_own_turns(self, *, tenant_id: str, user_id: str, session_id: str) -> bool:
        """True when THIS session has stored at least one turn of its own.

        Exists to keep `recent()`'s carry-over from being mistaken for continuity of *this* session.
        A caller that asks "is the user repeating themselves?" must not be answered from turns that
        were carried over from a previous session: from the user's side, the first thing they say in
        a new session is not a repeat, and telling them it is ("you may not have heard my last
        reply") is both wrong and unpleasant.

        Found by a control pair, not by reading: three fresh sessions for the same user produced the
        duplicate notice from turn 2 onward, while the same requests with distinct user ids produced
        it never — which located the cause in carry-over rather than in the duplicate check.
        """
        with self._connect() as conn:
            row = conn.execute(
                "SELECT 1 FROM conversation_turns"
                " WHERE tenant_id = ? AND user_id = ? AND session_id = ? LIMIT 1",
                (tenant_id, user_id, session_id),
            ).fetchone()
        return row is not None

    def recent(
        self,
        *,
        tenant_id: str,
        user_id: str,
        session_id: str,
        limit: int = MAX_CONVERSATION_TURNS,
        now: float | None = None,
        carry_over_window_s: float = CARRY_OVER_WINDOW_S,
    ) -> list[ConversationTurn]:
        """This session's turns, or — for a session with none yet — the user's very recent ones.

        Why the carry-over exists. History was keyed strictly on `session_id`, and the mobile app
        mints a fresh one on every mount (`local-${Date.now()}`), so **every app reload started the
        conversation from nothing.** Reported from a live session: *"it does not have context of
        what I spoke previously the context or continuing the conversation end to end."*

        Why it is not simply "key on user_id". A fixed session id was tried before and reverted
        because it resurrected a stale task from an earlier run and presented it as current (see the
        comment in `apps/mobile/src/App.tsx`). Both extremes are wrong: per-mount forgets everything,
        per-user remembers forever. So continuity is bounded by RECENCY and by emptiness:

          * only when this session has no turns of its own — the moment right after a reload. Once
            the session has said anything, it is authoritative and prior sessions are never mixed in,
            which is what keeps the old resurrection bug from returning;
          * only if the user's newest turn anywhere is within `carry_over_window_s`. A reload is
            seconds old; yesterday's abandoned task is not, and can never come back.

        `now` is injected rather than read from the clock here, so this stays pure and replayable —
        the caller already passes a timestamp to `append`. Omitting `now` disables carry-over
        entirely (returns exactly the old behaviour), which makes every existing caller and test
        byte-for-byte unchanged until it opts in.
        """
        with self._connect() as conn:
            conn.row_factory = sqlite3.Row
            rows = conn.execute(
                "SELECT role, text, created_at FROM conversation_turns"
                " WHERE tenant_id = ? AND user_id = ? AND session_id = ?"
                " ORDER BY created_at DESC, id DESC LIMIT ?",
                (tenant_id, user_id, session_id, limit),
            ).fetchall()
            if rows or now is None or carry_over_window_s <= 0:
                return _turns_from(rows)
            # Fresh session: carry over the single most recent prior session, if it is recent
            # enough. One session, not a merge of several — a merge would interleave unrelated
            # conversations and read as the orb confusing two topics.
            previous = conn.execute(
                "SELECT session_id, MAX(created_at) AS last_at FROM conversation_turns"
                " WHERE tenant_id = ? AND user_id = ? AND session_id != ?"
                " GROUP BY session_id ORDER BY last_at DESC LIMIT 1",
                (tenant_id, user_id, session_id),
            ).fetchone()
            if previous is None:
                return []
            if now - float(previous["last_at"]) > carry_over_window_s:
                return []
            carried = conn.execute(
                "SELECT role, text, created_at FROM conversation_turns"
                " WHERE tenant_id = ? AND user_id = ? AND session_id = ?"
                " ORDER BY created_at DESC, id DESC LIMIT ?",
                (tenant_id, user_id, previous["session_id"], limit),
            ).fetchall()
        return _turns_from(carried)
