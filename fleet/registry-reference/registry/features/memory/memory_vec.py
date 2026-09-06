#!/usr/bin/env python3
"""Small sqlite-vec bridge for Python builds without SQLite extension loading."""

from __future__ import annotations

import argparse
import json
import sqlite3
from typing import Any


def open_db(database: str, extension: str) -> sqlite3.Connection:
    conn = sqlite3.connect(database, timeout=30)
    conn.enable_load_extension(True)
    conn.load_extension(extension)
    return conn


def ensure_table(conn: sqlite3.Connection, dimensions: int) -> None:
    if not dimensions > 0:
        raise ValueError("dimensions must be positive")
    conn.execute(
        f"CREATE VIRTUAL TABLE IF NOT EXISTS memory_vectors "
        f"USING vec0(memory_rowid INTEGER PRIMARY KEY, embedding FLOAT[{dimensions}])"
    )


def sync(args: argparse.Namespace) -> dict[str, Any]:
    payload = json.load(__import__("sys").stdin)
    rows = payload.get("rows", [])
    deletes = payload.get("deletes", [])
    conn = open_db(args.database, args.extension)
    try:
        ensure_table(conn, args.dimensions)
        conn.execute("BEGIN")
        for memory_rowid in deletes:
            conn.execute("DELETE FROM memory_vectors WHERE memory_rowid = ?", (int(memory_rowid),))
        for memory_rowid, encoded in rows:
            conn.execute("DELETE FROM memory_vectors WHERE memory_rowid = ?", (int(memory_rowid),))
            vector = bytes.fromhex(encoded)
            if len(vector) != args.dimensions * 4:
                raise ValueError(f"memory_rowid {memory_rowid} has the wrong vector size")
            conn.execute(
                "INSERT INTO memory_vectors(memory_rowid, embedding) VALUES (?, ?)",
                (int(memory_rowid), vector),
            )
        conn.commit()
        count = conn.execute("SELECT count(*) FROM memory_vectors").fetchone()[0]
        return {"upserted": len(rows), "deleted": len(deletes), "vector_rows": count}
    finally:
        conn.close()


def count(args: argparse.Namespace) -> dict[str, int]:
    conn = open_db(args.database, args.extension)
    try:
        ensure_table(conn, args.dimensions)
        return {"vector_rows": conn.execute("SELECT count(*) FROM memory_vectors").fetchone()[0]}
    finally:
        conn.close()


def knn(args: argparse.Namespace) -> dict[str, Any]:
    conn = open_db(args.database, args.extension)
    try:
        ensure_table(conn, args.dimensions)
        query = bytes.fromhex(args.query_hex)
        if len(query) != args.dimensions * 4:
            raise ValueError("query vector has the wrong size")
        rows = conn.execute(
            """
            SELECT memory_rowid, distance
            FROM memory_vectors
            WHERE embedding MATCH ?
            ORDER BY distance
            LIMIT ?
            """,
            (query, max(1, min(args.limit, 1000))),
        ).fetchall()
        return {"results": [{"memory_rowid": row[0], "distance": row[1]} for row in rows]}
    finally:
        conn.close()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--database", required=True)
    parser.add_argument("--extension", required=True)
    parser.add_argument("--dimensions", type=int, default=384)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("count")
    sub.add_parser("sync")
    knn_parser = sub.add_parser("knn")
    knn_parser.add_argument("--query-hex", required=True)
    knn_parser.add_argument("--limit", type=int, default=1000)
    args = parser.parse_args()
    result = sync(args) if args.command == "sync" else count(args) if args.command == "count" else knn(args)
    print(json.dumps(result, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
