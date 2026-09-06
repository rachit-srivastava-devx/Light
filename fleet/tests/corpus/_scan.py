"""Shared source scanner for the Python-backed corpus detectors."""

import os
from pathlib import Path


PRUNED_DIRS = {
    ".codebase-memory",
    ".git",
    ".venv",
    "__pycache__",
    "docs",
    "node_modules",
    "site-packages",
    "target",
    "target-shared",
    "tests",
    "tmp",
    "var",
}


def lines(base):
    """Yield non-comment source lines outside generated and detector-owned trees."""
    base = Path(base)
    for current, dirs, files in os.walk(base):
        current = Path(current)
        dirs[:] = [name for name in dirs if name not in PRUNED_DIRS]
        for name in files:
            path = current / name
            if path.suffix in {".md", ".pyc"}:
                continue
            try:
                text = path.read_text(errors="replace").splitlines()
            except OSError:
                continue
            for number, source in enumerate(text, 1):
                if source.lstrip().startswith("#"):
                    continue
                if path.name == ".gitignore" and source.strip().rstrip("/") in PRUNED_DIRS:
                    continue
                yield path, number, source
