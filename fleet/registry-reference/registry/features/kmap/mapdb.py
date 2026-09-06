#!/usr/bin/env python3
"""Offline SQLite project memory map for Bash, Python, and TypeScript.

SQLite recursive CTEs are sufficient for this slice. If closure becomes the
bottleneck, the upgrade path is embedded Cypher in LadybugDB (MIT, active);
KuzuDB is archived and is deliberately not used.

Bash call edges are a heuristic: a bare command name is matched against a
known function in the file or its source closure. Calls through $cmd or eval
are undetectable, so Bash call edges are confidence=heuristic, never exact.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sqlite3
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from pydriller import Repository
from tree_sitter import Language, Parser
import tree_sitter_bash
import tree_sitter_python
import tree_sitter_typescript


SCHEMA = """
CREATE TABLE IF NOT EXISTS repos (
 id TEXT PRIMARY KEY, root TEXT UNIQUE NOT NULL, head TEXT, indexed_at REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS folders (
 id TEXT PRIMARY KEY, repo_id TEXT NOT NULL, path TEXT NOT NULL, UNIQUE(repo_id,path)
);
CREATE TABLE IF NOT EXISTS files (
 id TEXT PRIMARY KEY, repo_id TEXT NOT NULL, path TEXT NOT NULL, language TEXT,
 content_hash TEXT NOT NULL, parsed INTEGER NOT NULL, skip_reason TEXT,
 parse_error TEXT, updated_at REAL NOT NULL, UNIQUE(repo_id,path)
);
CREATE TABLE IF NOT EXISTS symbols (
 id TEXT PRIMARY KEY, file_id TEXT NOT NULL, name TEXT NOT NULL, line INTEGER NOT NULL,
 language TEXT NOT NULL, UNIQUE(file_id,name,line)
);
CREATE TABLE IF NOT EXISTS facts (
 file_id TEXT PRIMARY KEY, sources_json TEXT NOT NULL, calls_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS commits (
 id TEXT PRIMARY KEY, repo_id TEXT NOT NULL, hash TEXT NOT NULL, author_date TEXT,
 message TEXT, UNIQUE(repo_id,hash)
);
CREATE TABLE IF NOT EXISTS edges (
 source_id TEXT NOT NULL, target_id TEXT NOT NULL, kind TEXT NOT NULL,
 confidence TEXT NOT NULL CHECK(confidence IN ('exact','heuristic','inferred')),
 PRIMARY KEY(source_id,target_id,kind)
);
CREATE INDEX IF NOT EXISTS idx_files_repo_path ON files(repo_id,path);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target_id,kind);
CREATE INDEX IF NOT EXISTS idx_commits_repo_hash ON commits(repo_id,hash);
"""

EXTENSIONS = {
    ".sh": "bash", ".bash": "bash", ".py": "python", ".pyw": "python",
    ".ts": "typescript", ".tsx": "typescript", ".js": "typescript", ".jsx": "typescript",
}
# These are generated, dependency, VCS, or runtime trees rather than source
# content.  `files()` is the single population contract used by indexing and
# freshness checks; keep exclusions here so the two paths cannot drift.
# T24b: "ledger" belongs here too. kmap.sh's cmd_index/cmd_scan deliberately
# append a receipt to ledger/RECEIPTS.jsonl right after indexing (the same
# ledger every other fleet command writes to). Since that directory sits
# inside the indexed repo, leaving it out of this set meant the index's own
# receipt write changed a "current" file the very next freshness check would
# re-hash -- so every query answered stale=true immediately after a fresh
# index. ledger/ is evidence (hash-chained receipts, lesson/promotion
# records), never source, so nothing in it should ever be a code-graph node.
IGNORED_DIRS = {
    ".git", ".hg", ".svn", ".code-graph", "node_modules", "var", "dist",
    ".venv", "venv", "__pycache__", ".mypy_cache", ".pytest_cache", "ledger",
}
SUPPORTED = {"bash", "python", "typescript"}


def rid(root: Path) -> str:
    return "repo:" + str(root)


def fid(repo: str, path: str) -> str:
    return "file:" + repo + ":" + path


def sid(file: str, name: str, line: int) -> str:
    # Canonical identity is repo root + relative file + symbol name. Line is
    # evidence only, so moving a function does not detach its history/impact.
    del line
    return "symbol:" + file + ":" + name


def cid(repo: str, commit_hash: str) -> str:
    return "commit:" + repo + ":" + commit_hash


def connect(db: Path) -> sqlite3.Connection:
    db.parent.mkdir(parents=True, exist_ok=True)
    c = sqlite3.connect(str(db))
    c.row_factory = sqlite3.Row
    c.executescript(SCHEMA)
    return c


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def head(root: Path) -> str | None:
    r = subprocess.run(["git", "-C", str(root), "rev-parse", "HEAD"],
                       capture_output=True, text=True, check=False)
    return r.stdout.strip() if r.returncode == 0 else None


def language(path: Path) -> str | None:
    lang = EXTENSIONS.get(path.suffix.lower())
    if lang:
        return lang
    try:
        line = path.open("rb").readline(256)
    except OSError:
        return None
    if b"bash" in line or (line.startswith(b"#!") and b"sh" in line):
        return "bash"
    return None


def _db_exclusions(root: Path, db: Path | None) -> set[str]:
    if db is None:
        return set()
    try:
        rel = db.expanduser().resolve().relative_to(root.resolve())
    except ValueError:
        return set()
    rel = str(rel).replace(os.sep, "/")
    return {rel, rel + "-wal", rel + "-shm", rel + "-journal"}


def files(root: Path, db: Path | None = None):
    db_exclusions = _db_exclusions(root, db)
    for base, dirs, names in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in IGNORED_DIRS)
        for name in sorted(names):
            p = Path(base) / name
            rel = str(p.relative_to(root)).replace(os.sep, "/")
            if rel in db_exclusions:
                continue
            if p.is_file() and not p.is_symlink():
                yield rel, p, language(p)


def descendants(node):
    for child in node.children:
        yield child
        yield from descendants(child)


def text(node) -> str:
    return node.text.decode("utf-8", errors="replace")


def first_named(node, types):
    for child in node.children:
        if child.type in types:
            return child
    return None


def parser(lang: str) -> Parser:
    grammars = {
        "bash": tree_sitter_bash.language(),
        "python": tree_sitter_python.language(),
        "typescript": tree_sitter_typescript.language_typescript(),
    }
    return Parser(Language(grammars[lang]))


def parse_bash(root):
    symbols, sources, calls = [], [], []
    for n in descendants(root):
        if n.type == "function_definition":
            name = first_named(n, {"word"})
            if name:
                symbols.append((text(name), n.start_point[0] + 1))
        elif n.type == "command":
            command_name = first_named(n, {"command_name"})
            if not command_name:
                continue
            command = text(command_name).strip()
            if command in {"source", "."}:
                args = [c for c in n.children if c.is_named and c.type != "command_name"]
                if args:
                    sources.append(text(args[0]).strip())
            elif command:
                calls.append((command, n.start_point[0] + 1))
    return symbols, sources, calls


def parse_python(root):
    symbols, sources, calls = [], [], []
    for n in descendants(root):
        if n.type in {"function_definition", "async_function_definition"}:
            name = first_named(n, {"identifier"})
            if name:
                symbols.append((text(name), n.start_point[0] + 1))
        elif n.type in {"import_statement", "import_from_statement"}:
            sources.append(text(n))
        elif n.type == "call":
            fn = n.child_by_field_name("function")
            if fn:
                calls.append((text(fn).split(".")[-1], n.start_point[0] + 1))
    return symbols, sources, calls


def parse_typescript(root):
    symbols, sources, calls = [], [], []
    for n in descendants(root):
        if n.type in {"function_declaration", "method_definition", "generator_function_declaration"}:
            name = n.child_by_field_name("name")
            if name:
                symbols.append((text(name), n.start_point[0] + 1))
        elif n.type == "import_statement":
            sources.append(text(n))
        elif n.type == "call_expression":
            fn = n.child_by_field_name("function")
            if fn:
                calls.append((text(fn).split(".")[-1], n.start_point[0] + 1))
    return symbols, sources, calls


def parse(path: Path, lang: str | None):
    if lang not in SUPPORTED:
        return [], [], [], "unsupported language"
    try:
        tree = parser(lang).parse(path.read_bytes())
        fn = {"bash": parse_bash, "python": parse_python, "typescript": parse_typescript}[lang]
        symbols, sources, calls = fn(tree.root_node)
        error = "tree-sitter syntax errors" if tree.root_node.has_error else None
        return symbols, sources, calls, error
    except (OSError, UnicodeError, ValueError) as e:
        return [], [], [], f"{type(e).__name__}: {e}"


def import_literals(raw: str) -> list[str]:
    values = re.findall(r"['\"]([^'\"]+)['\"]", raw)
    return values or [raw.strip().strip("'\"")]


def normalize(value: str) -> str:
    value = value.strip().strip("'\"")
    value = re.sub(r"\$[A-Za-z_][A-Za-z0-9_]*/", "", value)
    return value


def resolve(raw: str, source_rel: str, root: Path, roots: list[Path]):
    for value in import_literals(raw):
        value = normalize(value)
        python_module = re.match(r"^from\s+([.]?[A-Za-z_][A-Za-z0-9_.]*)\s+import\s+", raw)
        python_import = re.match(r"^import\s+([A-Za-z_][A-Za-z0-9_.]*)", raw)
        if python_module:
            value = python_module.group(1)
        elif python_import:
            value = python_import.group(1)
        path_like = value.startswith(".") or value.startswith("/") or "/" in value or value.endswith((".sh", ".bash", ".py", ".ts", ".tsx", ".js"))
        if value.startswith(".") or value.startswith("/"):
            candidates = [(root / Path(source_rel).parent / value).resolve()] if not value.startswith("/") else [Path(value).resolve()]
        elif path_like:
            candidates = [(root / value).resolve()]
            if "registry/" in value:
                candidates.append((root / value[value.index("registry/"):]).resolve())
        else:
            candidates = []
        for candidate in candidates:
            try:
                exists = candidate.is_file()
            except OSError:
                exists = False
            if not exists:
                continue
            for other in roots:
                try:
                    return other, str(candidate.relative_to(other)).replace(os.sep, "/")
                except ValueError:
                    pass
        if python_module or python_import:
            module = (python_module or python_import).group(1).replace(".", "/")
            module = module.lstrip("/")
            for suffix in (".py", "/__init__.py"):
                candidate = (root / (module + suffix)).resolve()
                if candidate.is_file():
                    return root, str(candidate.relative_to(root)).replace(os.sep, "/")
    return None


def edge(c, source, target, kind, confidence):
    if source != target:
        c.execute("INSERT OR IGNORE INTO edges VALUES (?,?,?,?)",
                  (source, target, kind, confidence))


def ensure_folders(c, repo: str, rel: str):
    parts = list(Path(rel).parent.parts)
    current = []
    for part in parts:
        current.append(part)
        path = "/".join(current)
        c.execute("INSERT OR IGNORE INTO folders VALUES (?,?,?)",
                  ("folder:" + repo + ":" + path, repo, path))


def repo_rows(c):
    return list(c.execute("SELECT * FROM repos ORDER BY root"))


def rebuild_edges(c, roots):
    # Structural edges are cheap to rebuild; parsing is incremental because facts
    # are persisted per file. This also prevents cross-repo edges from lingering.
    c.execute("DELETE FROM edges WHERE kind IN ('repo-contains-folder','folder-contains-folder','folder-contains-file','file-contains-symbol','file-sources-file','symbol-calls-symbol')")
    all_files = list(c.execute("SELECT * FROM files"))
    rows = [r for r in all_files if r["parsed"]]
    by_key = {(r["repo_id"], r["path"]): r for r in rows}
    symbols = list(c.execute("SELECT s.*,f.repo_id,f.path FROM symbols s JOIN files f ON f.id=s.file_id"))
    by_name = {}
    for s in symbols:
        by_name.setdefault(s["name"], []).append(s)
        edge(c, s["file_id"], s["id"], "file-contains-symbol", "exact")
    for repo in c.execute("SELECT id FROM repos"):
        root_folder = "folder:" + repo["id"] + ":."
        c.execute("INSERT OR IGNORE INTO folders VALUES (?,?,?)", (root_folder, repo["id"], "."))
        edge(c, repo["id"], root_folder, "repo-contains-folder", "exact")
    for folder in c.execute("SELECT id,repo_id,path FROM folders"):
        parent = str(Path(folder["path"]).parent)
        if parent == "." and folder["path"] != ".":
            parent_id = "folder:" + folder["repo_id"] + ":."
            edge(c, parent_id, folder["id"], "folder-contains-folder", "exact")
        elif folder["path"] != ".":
            parent_id = "folder:" + folder["repo_id"] + ":" + parent
            edge(c, parent_id, folder["id"], "folder-contains-folder", "exact")
    for file in all_files:
        parent = str(Path(file["path"]).parent)
        folder_id = "folder:" + file["repo_id"] + ":" + (parent if parent != "." else ".")
        edge(c, folder_id, file["id"], "folder-contains-file", "exact")
    roots_by_id = {rid(r): r for r in roots}
    facts = {}
    for r in rows:
        f = c.execute("SELECT sources_json,calls_json FROM facts WHERE file_id=?", (r["id"],)).fetchone()
        if f:
            facts[r["id"]] = (json.loads(f["sources_json"]), json.loads(f["calls_json"]))
        else:
            p = roots_by_id[r["repo_id"]] / r["path"]
            ss, src, calls, err = parse(p, r["language"])
            facts[r["id"]] = (src, calls)
    source_targets = {}
    for r in rows:
        srcs, _calls = facts[r["id"]]
        root = roots_by_id.get(r["repo_id"])
        if root is None:
            continue
        for raw in srcs:
            resolved = resolve(raw, r["path"], root, roots)
            if not resolved:
                continue
            target = by_key.get((rid(resolved[0]), resolved[1]))
            if target:
                source_targets.setdefault(r["id"], set()).add(target["id"])
                edge(c, r["id"], target["id"], "file-sources-file", "exact")
    for r in rows:
        _srcs, calls = facts[r["id"]]
        visible = [s for s in symbols if s["file_id"] == r["id"]]
        for target_file in source_targets.get(r["id"], set()):
            visible.extend(s for s in symbols if s["file_id"] == target_file)
        visible_names = {s["name"] for s in visible}
        own = [s for s in symbols if s["file_id"] == r["id"]]
        for name, line in calls:
            if name not in visible_names:
                continue
            for target in (s for s in visible if s["name"] == name):
                caller = next((s for s in reversed(own) if s["line"] <= line), None)
                if not caller:
                    continue
                confidence = "heuristic" if r["language"] == "bash" else ("exact" if target["file_id"] == r["id"] else "heuristic")
                edge(c, caller["id"], target["id"], "symbol-calls-symbol", confidence)


def index_commits(c, repo: str, root: Path, skip_if_head_unchanged: bool = False):
    seen = indexed = skipped = 0
    if skip_if_head_unchanged:
        return {"commits_seen": 0, "commits_indexed": 0, "commits_skipped_existing": 0}
    existing = {r[0] for r in c.execute("SELECT hash FROM commits WHERE repo_id=?", (repo,))}
    mirror = Path(tempfile.mkdtemp(prefix="kmap-git-"))
    try:
        # PyDriller 2.10 writes a temporary setting to .git/config while opening
        # a repository. Fleet's .git is intentionally read-only in this runner,
        # so use a local, keyless clone in the writable temp area. No network is
        # involved and commit hashes/paths remain those of the indexed root.
        clone = subprocess.run(["git", "clone", "--no-local", "--quiet", str(root), str(mirror)],
                               capture_output=True, text=True, check=False)
        if clone.returncode != 0:
            return {"commits_seen": 0, "commits_indexed": 0, "commits_skipped_existing": 0}
        for commit in Repository(str(mirror), order="reverse").traverse_commits():
            seen += 1
            if commit.hash in existing:
                skipped += 1
                continue
            indexed += 1
            commit_id = cid(repo, commit.hash)
            c.execute("INSERT OR IGNORE INTO commits VALUES (?,?,?,?,?)",
                      (commit_id, repo, commit.hash,
                       commit.author_date.isoformat() if commit.author_date else None,
                       commit.msg))
            changed = {}
            touched = []
            for modified in commit.modified_files:
                rel = (modified.new_path or modified.old_path or "").replace("\\", "/")
                row = c.execute("SELECT id FROM files WHERE repo_id=? AND path=?", (repo, rel)).fetchone()
                if not row:
                    continue
                touched.append(row["id"])
                edge(c, commit_id, row["id"], "commit-touches-file", "exact")
                names = set()
                for method in modified.changed_methods or []:
                    name = getattr(method, "name", None)
                    if name:
                        names.add(name.split(".")[-1])
                changed[row["id"]] = names
            for file_id in touched:
                candidates = list(c.execute("SELECT id,name FROM symbols WHERE file_id=?", (file_id,)))
                names = changed.get(file_id, set())
                selected = [s for s in candidates if s["name"] in names] if names else candidates
                for s in selected:
                    edge(c, commit_id, s["id"], "commit-changes-symbol", "exact" if names else "heuristic")
    except Exception:
        # Code indexing remains useful for a non-git directory; history is empty.
        pass
    finally:
        shutil.rmtree(mirror, ignore_errors=True)
    return {"commits_seen": seen, "commits_indexed": indexed, "commits_skipped_existing": skipped}


def index_repo(db: Path, root_arg: str):
    started = time.perf_counter()
    root = Path(root_arg).expanduser().resolve()
    if not root.is_dir():
        raise ValueError(f"repo is not a directory: {root_arg}")
    repo = rid(root)
    c = connect(db)
    old = {r["path"]: r for r in c.execute("SELECT * FROM files WHERE repo_id=?", (repo,))}
    previous = c.execute("SELECT head FROM repos WHERE id=?", (repo,)).fetchone()
    current_head = head(root)
    now = time.time()
    c.execute("INSERT INTO repos VALUES (?,?,?,?) ON CONFLICT(id) DO UPDATE SET head=excluded.head,indexed_at=excluded.indexed_at",
              (repo, str(root), current_head, now))
    roots = [Path(r["root"]) for r in repo_rows(c)]
    counts = {"considered": 0, "parsed_now": 0, "skipped_unchanged": 0,
              "skipped_unsupported": 0, "parse_errors": 0}
    seen = set()
    with c:
        for rel, path, lang in files(root, db):
            counts["considered"] += 1
            seen.add(rel)
            d = digest(path)
            old_row = old.get(rel)
            file = fid(repo, rel)
            if old_row and old_row["content_hash"] == d:
                counts["skipped_unchanged"] += 1
                continue
            if lang not in SUPPORTED:
                counts["skipped_unsupported"] += 1
                c.execute("""INSERT INTO files VALUES (?,?,?,?,?,?,?,?,?)
                             ON CONFLICT(id) DO UPDATE SET language=excluded.language,
                             content_hash=excluded.content_hash,parsed=excluded.parsed,
                             skip_reason=excluded.skip_reason,parse_error=excluded.parse_error,
                             updated_at=excluded.updated_at""",
                          (file, repo, rel, lang, d, 0, "unsupported language", None, now))
                c.execute("DELETE FROM symbols WHERE file_id=?", (file,))
                c.execute("DELETE FROM facts WHERE file_id=?", (file,))
                continue
            symbols, sources, calls, error = parse(path, lang)
            counts["parsed_now"] += 1
            counts["parse_errors"] += int(bool(error))
            c.execute("""INSERT INTO files VALUES (?,?,?,?,?,?,?,?,?)
                         ON CONFLICT(id) DO UPDATE SET language=excluded.language,
                         content_hash=excluded.content_hash,parsed=excluded.parsed,
                         skip_reason=excluded.skip_reason,parse_error=excluded.parse_error,
                         updated_at=excluded.updated_at""",
                      # tree-sitter recovery still yields useful symbols and
                      # edges; retain them and expose the syntax issue separately.
                      (file, repo, rel, lang, d, 1, None, error, now))
            c.execute("DELETE FROM symbols WHERE file_id=?", (file,))
            c.execute("DELETE FROM facts WHERE file_id=?", (file,))
            for name, line in symbols:
                c.execute("INSERT OR IGNORE INTO symbols VALUES (?,?,?,?,?)",
                          (sid(file, name, line), file, name, line, lang))
            c.execute("INSERT INTO facts VALUES (?,?,?)",
                      (file, json.dumps(sources), json.dumps(calls)))
            ensure_folders(c, repo, rel)
        for rel in set(old) - seen:
            c.execute("DELETE FROM files WHERE repo_id=? AND path=?", (repo, rel))
        rebuild_edges(c, roots)
        commit_counts = index_commits(c, repo, root, bool(previous and previous["head"] == current_head))
    c.close()
    c = connect(db)
    f = c.execute("SELECT COUNT(*) total,COALESCE(SUM(parsed),0) parsed FROM files WHERE repo_id=?", (repo,)).fetchone()
    s = c.execute("SELECT COUNT(*) FROM symbols WHERE file_id IN (SELECT id FROM files WHERE repo_id=?)", (repo,)).fetchone()[0]
    language_counts = {r["language"]: r["count"] for r in c.execute("SELECT language,COUNT(*) count FROM symbols WHERE file_id IN (SELECT id FROM files WHERE repo_id=?) GROUP BY language", (repo,))}
    e = c.execute("SELECT COUNT(*) FROM edges WHERE source_id LIKE ? OR target_id LIKE ?", ("%"+repo+"%", "%"+repo+"%")).fetchone()[0]
    commits = c.execute("SELECT COUNT(*) FROM commits WHERE repo_id=?", (repo,)).fetchone()[0]
    c.close()
    files_out = {
        "considered": counts["considered"], "indexed": f["total"], "parsed_total": f["parsed"],
        "parsed_now": counts["parsed_now"], "skipped_unchanged": counts["skipped_unchanged"],
        "skipped_unsupported": counts["skipped_unsupported"], "parse_errors": counts["parse_errors"],
        "why_skipped": {"unchanged_content_hash": counts["skipped_unchanged"],
                        "unsupported_language": counts["skipped_unsupported"]},
    }
    return {
        "repo": str(root), "repo_id": repo, "files": files_out, "symbols": s,
        "symbols_by_language": language_counts, "edges": e,
        "commits": commits, "commit_denominator": commit_counts,
        "duration_ms": round((time.perf_counter() - started) * 1000, 2),
    }


def fresh(c, repos, db):
    reasons = []
    for repo in repos:
        root = Path(repo["root"])
        if not root.is_dir():
            reasons.append("repo_missing:" + str(root))
            continue
        if head(root) != repo["head"]:
            reasons.append("git_head_changed:" + str(root))
        stored = {r["path"]: r["content_hash"] for r in c.execute("SELECT path,content_hash FROM files WHERE repo_id=?", (repo["id"],))}
        current = {rel: digest(path) for rel, path, _ in files(root, db)}
        if stored != current:
            reasons.append("content_hash_changed:" + str(root))
    return not reasons, reasons


def selected_repos(c, repo_arg):
    rows = repo_rows(c)
    if repo_arg:
        root = str(Path(repo_arg).expanduser().resolve())
        rows = [r for r in rows if r["root"] == root]
    return rows


def stale(reasons, json_mode, kind):
    result = {"status": "stale", "stale": True, "kind": kind, "reasons": reasons, "hits": []}
    if json_mode:
        print(json.dumps(result, sort_keys=True))
    else:
        print("stale: " + ", ".join(reasons))
    return 6


def run_impact(c, args, json_mode):
    repos = selected_repos(c, args.repo)
    if not repos:
        raise ValueError("path_unregistered: no indexed repo matches --repo")
    ok, reasons = fresh(c, repos, args.db)
    if not ok:
        return stale(reasons, json_mode, "impact")
    if args.symbol:
        where = ["s.name=?"]
        values = [args.symbol]
        if args.repo:
            where.append("f.repo_id=?")
            values.append(repos[0]["id"])
        targets = list(c.execute("SELECT s.id,s.name,f.path FROM symbols s JOIN files f ON f.id=s.file_id WHERE " + " AND ".join(where), values))
        kind, target = "symbol-calls-symbol", args.symbol
    else:
        absolute = str(Path(args.file).expanduser().resolve())
        targets = []
        for r in repos:
            try:
                rel = str(Path(absolute).relative_to(Path(r["root"]))).replace(os.sep, "/")
            except ValueError:
                continue
            row = c.execute("SELECT id,path FROM files WHERE repo_id=? AND path=?", (r["id"], rel)).fetchone()
            if row:
                targets.append(row)
        kind, target = "file-sources-file", args.file
    if not targets:
        result = {"status": "ok", "stale": False, "kind": "impact", "target": target, "target_count": 0, "hits": []}
        print(json.dumps(result, sort_keys=True) if json_mode else "impact: 0 dependents")
        return 0
    ids = [r["id"] for r in targets]
    marks = ",".join("?" for _ in ids)
    sql = f"""
    WITH RECURSIVE closure(node_id,depth,confidence,path) AS (
      SELECT e.source_id,1,e.confidence,'|'||e.target_id||'|'||e.source_id||'|'
      FROM edges e WHERE e.target_id IN ({marks}) AND e.kind=?
      UNION ALL
      SELECT e.source_id,c.depth+1,
        CASE WHEN c.confidence='heuristic' OR e.confidence='heuristic' THEN 'heuristic' ELSE e.confidence END,
        c.path||e.source_id||'|'
      FROM edges e JOIN closure c ON e.target_id=c.node_id
      WHERE e.kind=? AND c.depth < ? AND instr(c.path,'|'||e.source_id||'|')=0
    )
    SELECT DISTINCT cl.node_id,cl.depth,cl.confidence,
      COALESCE(f.path,tf.path) path,s.name symbol
    FROM closure cl
    LEFT JOIN symbols s ON s.id=cl.node_id
    LEFT JOIN files f ON f.id=s.file_id
    LEFT JOIN files tf ON tf.id=cl.node_id
    ORDER BY cl.depth,path,symbol
    """
    closure_values = ids + [kind, kind, max(1, args.depth)]
    hits = [dict(r) for r in c.execute(sql, closure_values)]
    result = {"status": "ok", "stale": False, "kind": "impact", "target": target,
              "target_count": len(targets), "depth": args.depth, "hits": hits}
    if json_mode:
        print(json.dumps(result, sort_keys=True))
    else:
        print(f"impact: {len(hits)} dependents")
        for h in hits:
            print(f"{h['depth']}\t{h['confidence']}\t{h['path']}\t{h.get('symbol') or h['path']}")
    return 0


def run_history(c, args, json_mode):
    repos = selected_repos(c, args.repo)
    if not repos:
        raise ValueError("path_unregistered: no indexed repo matches --repo")
    ok, reasons = fresh(c, repos, args.db)
    if not ok:
        return stale(reasons, json_mode, "history")
    values = [args.symbol]
    clause = ""
    if args.repo:
        clause = " AND f.repo_id=?"
        values.append(repos[0]["id"])
    rows = c.execute(
        "SELECT DISTINCT cm.hash,cm.author_date,cm.message,f.path "
        "FROM commits cm JOIN edges e ON e.source_id=cm.id "
        "JOIN symbols s ON s.id=e.target_id JOIN files f ON f.id=s.file_id "
        "WHERE s.name=?"+clause+" AND e.kind='commit-changes-symbol' "
        "ORDER BY cm.author_date DESC,cm.hash", values)
    result = {"status": "ok", "stale": False, "kind": "history", "symbol": args.symbol,
              "commits": [dict(r) for r in rows]}
    if json_mode:
        print(json.dumps(result, sort_keys=True))
    else:
        for r in result["commits"]:
            print(f"{r['hash']}\t{r['path']}\t{r['message'] or ''}")
    return 0


def args(argv):
    json_mode = "--json" in argv
    db_arg = None
    clean = []
    i = 0
    while i < len(argv):
        if argv[i] == "--json":
            i += 1
        elif argv[i] == "--db":
            if i + 1 >= len(argv):
                raise ValueError("--db requires a path")
            db_arg, i = argv[i+1], i+2
        else:
            clean.append(argv[i])
            i += 1
    p = argparse.ArgumentParser(prog="kmap-index.sh")
    sub = p.add_subparsers(dest="command", required=True)
    ip = sub.add_parser("index")
    ip.add_argument("--repo", required=True)
    ap = sub.add_parser("impact")
    group = ap.add_mutually_exclusive_group(required=True)
    group.add_argument("--symbol")
    group.add_argument("--file")
    ap.add_argument("--repo")
    ap.add_argument("--depth", type=int, default=8)
    hp = sub.add_parser("history")
    hp.add_argument("--symbol", required=True)
    hp.add_argument("--repo")
    ns = p.parse_args(clean)
    if getattr(ns, "depth", 1) < 1:
        raise ValueError("--depth must be >= 1")
    if db_arg:
        db = Path(db_arg).expanduser()
    else:
        configured = os.environ.get("FLEET_KMAP_DB")
        # Keep runtime state outside any indexed repository by default.  Use
        # --db or FLEET_KMAP_DB for a durable/custom location.
        db = Path(configured).expanduser() if configured else Path(tempfile.gettempdir()) / "fleet-kmap/kmap.sqlite3"
    return ns, json_mode, db


def main(argv):
    json_mode = "--json" in argv
    try:
        ns, json_mode, db = args(argv)
        if ns.command == "index":
            result = index_repo(db, ns.repo)
            print(json.dumps(result, sort_keys=True) if json_mode else (
                f"repo={result['repo']}\n"
                f"files={result['files']['indexed']} parsed_total={result['files']['parsed_total']} "
                f"parsed_now={result['files']['parsed_now']} skipped_unchanged={result['files']['skipped_unchanged']} "
                f"skipped_unsupported={result['files']['skipped_unsupported']} parse_errors={result['files']['parse_errors']} "
                f"why_skipped={result['files']['why_skipped']}\n"
                f"symbols={result['symbols']} edges={result['edges']} commits={result['commits']} duration_ms={result['duration_ms']}"
            ))
            return 0
        c = connect(db)
        try:
            ns.db = db
            return run_impact(c, ns, json_mode) if ns.command == "impact" else run_history(c, ns, json_mode)
        finally:
            c.close()
    except (ValueError, sqlite3.Error) as e:
        if json_mode:
            print(json.dumps({"status": "error", "error": str(e)}, sort_keys=True))
        else:
            print("error: " + str(e), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
