#!/usr/bin/env python3
"""Offline hybrid SQLite FTS5 and sqlite-vec memory store.

SQLite is deliberate here: fleet needs small transactional point writes plus ranked
full-text lookup, not DuckDB's analytical scan engine. FTS5's porter tokenizer and
BM25 ranking are fused with cached 384-dimensional embeddings through reciprocal-rank
fusion; DuckDB remains the analytics dependency for event ledgers.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sqlite3
import sys
import time
from hashlib import sha256
from pathlib import Path
from typing import Any, Optional

WORD_RE = re.compile(r"[A-Za-z0-9_]{2,}")
MODEL_NAME = "BAAI/bge-small-en-v1.5"
VECTOR_DIMENSIONS = 384
RRF_K = 60
VECTOR_BRIDGE = Path(__file__).with_name("memory_vec.py")
_EMBEDDER: Any = None
_VECTOR_PYTHON: Optional[str] = None


def root_dir() -> Path:
    return Path(os.environ.get("AGENT_RECALL_ROOT", ".")).expanduser().resolve()


def db_path() -> Path:
    configured = os.environ.get("FLEET_MEMORY_DB")
    return Path(configured).expanduser().resolve() if configured else root_dir() / "fleet-memory.sqlite3"


def connect() -> sqlite3.Connection:
    path = db_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(path, timeout=10)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    conn.execute("PRAGMA journal_mode = WAL")
    conn.execute("PRAGMA synchronous = NORMAL")
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS memories (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT '',
            keywords TEXT NOT NULL DEFAULT '',
            evidence TEXT NOT NULL DEFAULT '',
            severity TEXT NOT NULL DEFAULT 'important',
            confirmed_count INTEGER NOT NULL DEFAULT 1,
            last_confirmed TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
            title, body, keywords, evidence,
            content='memories', content_rowid='rowid',
            tokenize='porter unicode61'
        );

        CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
            INSERT INTO memories_fts(rowid, title, body, keywords, evidence)
            VALUES (new.rowid, new.title, new.body, new.keywords, new.evidence);
        END;
        CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
            INSERT INTO memories_fts(memories_fts, rowid, title, body, keywords, evidence)
            VALUES ('delete', old.rowid, old.title, old.body, old.keywords, old.evidence);
        END;
        CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
            INSERT INTO memories_fts(memories_fts, rowid, title, body, keywords, evidence)
            VALUES ('delete', old.rowid, old.title, old.body, old.keywords, old.evidence);
            INSERT INTO memories_fts(rowid, title, body, keywords, evidence)
            VALUES (new.rowid, new.title, new.body, new.keywords, new.evidence);
        END;
        """
    )
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS memory_vector_meta (
            memory_rowid INTEGER PRIMARY KEY,
            content_hash TEXT NOT NULL,
            model_name TEXT NOT NULL,
            dimensions INTEGER NOT NULL,
            embedded_at TEXT NOT NULL
        )
        """
    )
    indexed = conn.execute("SELECT count(*) FROM memories_fts").fetchone()[0]
    stored = conn.execute("SELECT count(*) FROM memories").fetchone()[0]
    if indexed != stored:
        conn.execute("INSERT INTO memories_fts(memories_fts) VALUES ('rebuild')")
    conn.commit()
    return conn


def legacy_rows() -> list[dict[str, Any]]:
    path = root_dir() / "insights-index.json"
    if not path.exists():
        return []
    payload = json.loads(path.read_text(encoding="utf-8"))
    return payload.get("insights", [])


def migrate(conn: sqlite3.Connection) -> dict[str, int | str]:
    rows = legacy_rows()
    migrated = 0
    for row in rows:
        ident = str(row.get("id", "")).strip()
        title = str(row.get("title", "")).strip()
        if not ident or not title:
            continue
        keywords = row.get("applies_when", [])
        if not isinstance(keywords, list):
            keywords = []
        conn.execute(
            """
            INSERT INTO memories
              (id, title, body, source, keywords, evidence, severity, confirmed_count, last_confirmed)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              title=excluded.title, body=excluded.body, source=excluded.source,
              keywords=excluded.keywords, evidence=excluded.evidence,
              severity=excluded.severity, confirmed_count=excluded.confirmed_count,
              last_confirmed=excluded.last_confirmed, updated_at=datetime('now')
            """,
            (
                ident,
                title,
                title,
                str(row.get("source", "")),
                ",".join(str(item) for item in keywords),
                str(row.get("evidence", title)),
                str(row.get("severity", "important")),
                int(row.get("confirmed_count", 1) or 1),
                str(row.get("last_confirmed", "")),
            ),
        )
        migrated += 1
    conn.commit()
    count = conn.execute("SELECT count(*) FROM memories").fetchone()[0]
    return {"legacy_rows": len(rows), "migrated_rows": migrated, "database_rows": count, "database": str(db_path())}


def ensure_migrated(conn: sqlite3.Connection) -> None:
    if legacy_rows():
        migrate(conn)


def vector_extension() -> Path:
    try:
        import sqlite_vec
    except ImportError as exc:
        raise RuntimeError("sqlite_vec is required for hybrid memory retrieval") from exc
    package = Path(sqlite_vec.__file__).parent
    for name in ("vec0.dylib", "vec0.so", "vec0.dll"):
        candidate = package / name
        if candidate.exists():
            return candidate.resolve()
    raise RuntimeError(f"sqlite-vec loadable extension is missing from {package}")


def vector_python() -> str:
    global _VECTOR_PYTHON
    if _VECTOR_PYTHON:
        return _VECTOR_PYTHON
    candidates = [os.environ.get("FLEET_SQLITE_PYTHON", ""), sys.executable]
    candidates.extend(("/opt/homebrew/opt/python@3.14/bin/python3.14", "python3"))
    for candidate in candidates:
        if not candidate:
            continue
        try:
            check = subprocess.run(
                [candidate, "-c", "import sqlite3; c=sqlite3.connect(':memory:'); raise SystemExit(0 if hasattr(c, 'load_extension') else 1)"],
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except OSError:
            continue
        if check.returncode == 0:
            _VECTOR_PYTHON = candidate
            return candidate
    raise RuntimeError("no Python interpreter with SQLite extension loading is available")


def vector_bridge(command: str, payload: dict[str, Any] | None = None, extra: list[str] | None = None) -> dict[str, Any]:
    argv = [
        vector_python(),
        str(VECTOR_BRIDGE),
        "--database",
        str(db_path()),
        "--extension",
        str(vector_extension()),
        "--dimensions",
        str(VECTOR_DIMENSIONS),
        command,
    ]
    if extra:
        argv.extend(extra)
    result = subprocess.run(
        argv,
        input=json.dumps(payload).encode("utf-8") if payload is not None else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"sqlite-vec bridge failed ({result.returncode}): {detail}")
    return json.loads(result.stdout.decode("utf-8"))


def memory_text(row: sqlite3.Row) -> str:
    return "\n".join(
        (
            str(row["title"]),
            str(row["body"]),
            f"Keywords: {row['keywords']}",
            f"Evidence: {row['evidence']}",
        )
    )


def content_hash(text: str) -> str:
    return sha256(f"{MODEL_NAME}\0{VECTOR_DIMENSIONS}\0{text}".encode("utf-8")).hexdigest()


def embedder() -> Any:
    global _EMBEDDER
    if _EMBEDDER is None:
        os.environ.setdefault("HF_HUB_OFFLINE", "1")
        os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
        from fastembed import TextEmbedding

        _EMBEDDER = TextEmbedding(model_name=MODEL_NAME, local_files_only=True)
    return _EMBEDDER


def embed_texts(texts: list[str]) -> list[bytes]:
    if not texts:
        return []
    vectors = list(embedder().embed(texts, batch_size=256))
    encoded: list[bytes] = []
    for vector in vectors:
        if len(vector) != VECTOR_DIMENSIONS:
            raise RuntimeError(f"embedding dimension is {len(vector)}, expected {VECTOR_DIMENSIONS}")
        encoded.append(vector.astype("float32", copy=False).tobytes())
    return encoded


def ensure_embeddings(conn: sqlite3.Connection) -> dict[str, int | str]:
    rows = conn.execute(
        "SELECT rowid AS memory_rowid, title, body, keywords, evidence FROM memories ORDER BY rowid"
    ).fetchall()
    metadata = {
        row["memory_rowid"]: row["content_hash"]
        for row in conn.execute("SELECT memory_rowid, content_hash FROM memory_vector_meta")
    }
    current_ids = {row["memory_rowid"] for row in rows}
    stale_ids = sorted(set(metadata) - current_ids)
    vector_rows = vector_bridge("count")["vector_rows"]
    needs_all = vector_rows != len(rows)
    pending = [
        row
        for row in rows
        if needs_all or metadata.get(row["memory_rowid"]) != content_hash(memory_text(row))
    ]
    vectors = embed_texts([memory_text(row) for row in pending])
    payload = {
        "rows": [[row["memory_rowid"], vector.hex()] for row, vector in zip(pending, vectors)],
        "deletes": stale_ids,
    }
    if pending or stale_ids or needs_all:
        vector_bridge("sync", payload)
        now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        conn.executemany(
            """
            INSERT INTO memory_vector_meta(memory_rowid, content_hash, model_name, dimensions, embedded_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(memory_rowid) DO UPDATE SET
              content_hash=excluded.content_hash, model_name=excluded.model_name,
              dimensions=excluded.dimensions, embedded_at=excluded.embedded_at
            """,
            [(row["memory_rowid"], content_hash(memory_text(row)), MODEL_NAME, VECTOR_DIMENSIONS, now) for row in pending],
        )
        if stale_ids:
            conn.executemany("DELETE FROM memory_vector_meta WHERE memory_rowid = ?", [(ident,) for ident in stale_ids])
        conn.commit()
    final_vectors = vector_bridge("count")["vector_rows"]
    memory_count = len(rows)
    meta_count = conn.execute("SELECT count(*) FROM memory_vector_meta").fetchone()[0]
    if final_vectors != memory_count or meta_count != memory_count:
        raise RuntimeError(
            f"embedding invariant failed: memories={memory_count} vectors={final_vectors} metadata={meta_count}"
        )
    return {
        "model": MODEL_NAME,
        "dimensions": VECTOR_DIMENSIONS,
        "memory_count": memory_count,
        "embedded_count": meta_count,
        "new_embeddings": len(pending),
    }


def terms(query: str) -> list[str]:
    seen: set[str] = set()
    result: list[str] = []
    for token in WORD_RE.findall(query.lower()):
        if token not in seen:
            seen.add(token)
            result.append(token)
    return result


def bm25_search(conn: sqlite3.Connection, query: str, limit: int) -> list[dict[str, Any]]:
    tokens = terms(query)
    if not tokens:
        return []
    match = " OR ".join('"' + token.replace('"', '""') + '"' for token in tokens)
    rows = conn.execute(
        """
        SELECT m.id, m.source, m.title, m.severity, m.confirmed_count,
               round(-bm25(memories_fts, 5.0, 2.0, 1.0, 1.0), 6) AS relevance
        FROM memories_fts
        JOIN memories AS m ON m.rowid = memories_fts.rowid
        WHERE memories_fts MATCH ?
        ORDER BY bm25(memories_fts, 5.0, 2.0, 1.0, 1.0), m.updated_at DESC, m.id
        LIMIT ?
        """,
        (match, max(1, min(limit, 1000))),
    ).fetchall()
    return [
        {**dict(row), "bm25_rank": rank}
        for rank, row in enumerate(rows, start=1)
    ]


def search(conn: sqlite3.Connection, query: str, limit: int, ranking: str = "hybrid") -> dict[str, Any]:
    candidate_limit = max(100, min(limit, 1000))
    bm25 = bm25_search(conn, query, candidate_limit)
    if ranking == "bm25":
        return {
            "database": str(db_path()),
            "query": query,
            "ranking": "sqlite-fts5-bm25",
            "retrieval": "bm25-only",
            "results": [{key: value for key, value in row.items() if key != "bm25_rank"} for row in bm25[:limit]],
        }
    ensure_embeddings(conn)
    query_vector = embed_texts([query])[0]
    vector_rows = vector_bridge("knn", extra=["--query-hex", query_vector.hex(), "--limit", str(candidate_limit)])[
        "results"
    ]
    memory_rows = {
        row["memory_rowid"]: dict(row)
        for row in conn.execute(
            "SELECT rowid AS memory_rowid, id, source, title, severity, confirmed_count FROM memories"
        )
    }
    by_id: dict[str, dict[str, Any]] = {}
    bm25_by_id = {row["id"]: row for row in bm25}
    vector_by_id: dict[str, dict[str, Any]] = {}
    for rank, row in enumerate(vector_rows, start=1):
        memory = memory_rows.get(row["memory_rowid"])
        if memory is not None:
            vector_by_id[memory["id"]] = {"rank": rank, "distance": row["distance"]}
    for ident in set(bm25_by_id) | set(vector_by_id):
        lexical = bm25_by_id.get(ident)
        vector = vector_by_id.get(ident)
        result = dict((lexical or {}).copy())
        if not result:
            result = next(memory for memory in memory_rows.values() if memory["id"] == ident)
        bm25_rank = lexical["bm25_rank"] if lexical else None
        vector_rank = vector["rank"] if vector else None
        rrf_score = (1 / (RRF_K + bm25_rank) if bm25_rank else 0) + (1 / (RRF_K + vector_rank) if vector_rank else 0)
        result.update(
            {
                "bm25_rank": bm25_rank,
                "vector_rank": vector_rank,
                "vector_similarity": round(1 / (1 + vector["distance"]), 6) if vector else None,
                "rrf_score": round(rrf_score, 8),
            }
        )
        by_id[ident] = result
    results = sorted(by_id.values(), key=lambda row: (-row["rrf_score"], row["id"]))[:limit]
    return {
        "database": str(db_path()),
        "query": query,
        # Keep the old field stable for existing consumers; retrieval/fusion state is explicit.
        "ranking": "sqlite-fts5-bm25",
        "retrieval": "hybrid-rrf",
        "fusion": f"reciprocal-rank-fusion(k={RRF_K},equal-weight)",
        "results": results,
    }


def remember(conn: sqlite3.Connection, ident: str, text: str, severity: str) -> dict[str, Any]:
    keywords = ",".join(terms(text))
    title = f"[{ident}] {text}"
    conn.execute(
        """
        INSERT INTO memories (id, title, body, source, keywords, evidence, severity)
        VALUES (?, ?, ?, 'fleet', ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
          title=excluded.title, body=excluded.body, keywords=excluded.keywords,
          evidence=excluded.evidence, severity=excluded.severity,
          confirmed_count=memories.confirmed_count + 1, updated_at=datetime('now')
        """,
        (ident, title, text, keywords, text, severity),
    )
    conn.commit()
    return {"id": ident, "database": str(db_path()), "keywords": keywords}


def load_pairs(path: Path) -> dict[str, Any]:
    if str(path) == "-":
        return json.load(sys.stdin)
    return json.loads(path.read_text(encoding="utf-8"))


def benchmark(conn: sqlite3.Connection, pairs_path: Path, limit: int) -> dict[str, Any]:
    document = load_pairs(pairs_path)
    positives = document.get("positive", [])
    negatives = document.get("negative", [])
    if len(positives) < 12 or len(negatives) != len(positives):
        raise RuntimeError("benchmark requires at least 12 positive pairs and one negative per target")

    memories = conn.execute(
        "SELECT id, title, body, keywords FROM memories ORDER BY id"
    ).fetchall()
    ids = [row["id"] for row in memories]
    if len(ids) != len(set(ids)):
        raise RuntimeError("corpus invariant failed: duplicate memory ids")
    content_keys = [f"{row['title']}\0{row['body']}\0{row['keywords']}" for row in memories]
    if len(content_keys) != len(set(content_keys)):
        raise RuntimeError("corpus invariant failed: duplicate memory contents")
    memory_by_id = {row["id"]: row for row in memories}

    stopwords = {
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "does", "for", "from",
        "how", "in", "is", "it", "its", "of", "on", "or", "that", "the", "their", "this",
        "to", "when", "with", "without", "you", "your",
    }

    def content_terms(text: str) -> set[str]:
        return {token for token in terms(text) if token not in stopwords}

    for pair in positives:
        ident = pair.get("id")
        if ident not in memory_by_id:
            raise RuntimeError(f"benchmark target is not in the real corpus: {ident}")
        overlap = content_terms(pair["probe"]) & content_terms(
            " ".join(str(memory_by_id[ident][column]) for column in ("title", "body", "keywords"))
        )
        if overlap:
            raise RuntimeError(f"probe reuses target trigger words for {ident}: {sorted(overlap)}")

    embedding_report = ensure_embeddings(conn)
    benchmark_rows: list[dict[str, Any]] = []
    bm25_hits = hybrid_hits = bm25_false = hybrid_false = 0
    arms_differ = False
    for positive, negative in zip(positives, negatives):
        if positive["id"] != negative["id"]:
            raise RuntimeError("positive and negative arms are not paired by memory id")
        bm25_result = search(conn, positive["probe"], limit, "bm25")["results"]
        hybrid_result = search(conn, positive["probe"], limit, "hybrid")["results"]
        bm25_negative = search(conn, negative["probe"], limit, "bm25")["results"]
        hybrid_negative = search(conn, negative["probe"], limit, "hybrid")["results"]
        target = positive["id"]
        bm25_hit = target in {row["id"] for row in bm25_result}
        hybrid_hit = target in {row["id"] for row in hybrid_result}
        bm25_fp = target in {row["id"] for row in bm25_negative}
        hybrid_fp = target in {row["id"] for row in hybrid_negative}
        bm25_hits += int(bm25_hit)
        hybrid_hits += int(hybrid_hit)
        bm25_false += int(bm25_fp)
        hybrid_false += int(hybrid_fp)
        arms_differ = arms_differ or [row["id"] for row in bm25_result] != [row["id"] for row in hybrid_result]
        benchmark_rows.append(
            {
                "id": target,
                "probe": positive["probe"],
                "unrelated": negative["probe"],
                "bm25_hit": bm25_hit,
                "hybrid_hit": hybrid_hit,
                "bm25_false_positive": bm25_fp,
                "hybrid_false_positive": hybrid_fp,
                "bm25_rank": next((index for index, row in enumerate(bm25_result, 1) if row["id"] == target), None),
                "hybrid_rank": next((index for index, row in enumerate(hybrid_result, 1) if row["id"] == target), None),
            }
        )
    if not arms_differ:
        raise RuntimeError("benchmark invariant failed: BM25 and hybrid arms are identical")

    global _EMBEDDER
    _EMBEDDER = None
    cold_start = time.perf_counter()
    search(conn, positives[0]["probe"], limit, "hybrid")
    cold_ms = (time.perf_counter() - cold_start) * 1000
    warm_samples = []
    for _ in range(5):
        warm_start = time.perf_counter()
        search(conn, positives[0]["probe"], limit, "hybrid")
        warm_samples.append((time.perf_counter() - warm_start) * 1000)
    warm_samples.sort()
    warm_ms = warm_samples[len(warm_samples) // 2]
    total_positive = len(positives)
    bm25_precision_denominator = total_positive + bm25_false
    hybrid_precision_denominator = total_positive + hybrid_false
    return {
        "database": str(db_path()),
        "model": MODEL_NAME,
        "dimensions": VECTOR_DIMENSIONS,
        "fusion": f"reciprocal-rank-fusion(k={RRF_K},equal-weight)",
        "corpus": {
            "memories": len(memories),
            "embedded": embedding_report["embedded_count"],
            "missing_embeddings": len(memories) - int(embedding_report["embedded_count"]),
            "unique_ids": len(set(ids)),
            "unique_contents": len(set(content_keys)),
        },
        "distinct_arms": arms_differ,
        "pairs": benchmark_rows,
        "metrics": {
            "bm25": {
                "recall_hits": bm25_hits,
                "recall_total": total_positive,
                "recall": bm25_hits / total_positive,
                "false_positives": bm25_false,
                "precision": total_positive / bm25_precision_denominator,
            },
            "hybrid": {
                "recall_hits": hybrid_hits,
                "recall_total": total_positive,
                "recall": hybrid_hits / total_positive,
                "false_positives": hybrid_false,
                "precision": total_positive / hybrid_precision_denominator,
            },
        },
        "timings_ms": {"cold_query": round(cold_ms, 3), "warm_query_median": round(warm_ms, 3)},
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("migrate")
    sub.add_parser("count")
    remember_parser = sub.add_parser("remember")
    remember_parser.add_argument("--id", required=True)
    remember_parser.add_argument("--text", required=True)
    remember_parser.add_argument("--severity", choices=("critical", "important", "minor"), default="important")
    search_parser = sub.add_parser("search")
    search_parser.add_argument("--query", required=True)
    search_parser.add_argument("--limit", type=int, default=20)
    search_parser.add_argument("--ranking", choices=("bm25", "hybrid"), default="hybrid")
    benchmark_parser = sub.add_parser("benchmark")
    benchmark_parser.add_argument("--pairs", required=True)
    benchmark_parser.add_argument("--limit", type=int, default=10)
    args = parser.parse_args()
    conn = connect()
    ensure_migrated(conn)
    if args.command == "migrate":
        result = migrate(conn)
        result.update(ensure_embeddings(conn))
    elif args.command == "count":
        result = {"database": str(db_path()), "database_rows": conn.execute("SELECT count(*) FROM memories").fetchone()[0]}
        result.update(ensure_embeddings(conn))
    elif args.command == "remember":
        result = remember(conn, args.id, args.text, args.severity)
        result.update(ensure_embeddings(conn))
    elif args.command == "search":
        result = search(conn, args.query, args.limit, args.ranking)
    else:
        result = benchmark(conn, Path(args.pairs), args.limit)
    print(json.dumps(result, ensure_ascii=False, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
