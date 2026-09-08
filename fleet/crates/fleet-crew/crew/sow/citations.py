"""File/corpus citation resolution."""

from __future__ import annotations

import re
from pathlib import Path

from .errors import SOWRefusal

# References used by the blueprint's failure corpus are intentionally narrow:
# one or more uppercase namespace letters followed by an integer.
CORPUS_ID = re.compile(r"^[A-Z]{1,3}\d+$")
FILE_CITATION = re.compile(
    r"(?P<path>(?:/|\.\.?/)?[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*):(?P<line>[1-9]\d*)"
)


def _resolve_file(citation: str, root: Path) -> tuple[Path, int] | None:
    match = FILE_CITATION.fullmatch(citation)
    if match is None:
        return None
    path = Path(match.group("path"))
    line = int(match.group("line"))
    candidates = [path] if path.is_absolute() else [root / path, root.parent / path]
    for candidate in candidates:
        resolved = candidate.resolve()
        try:
            resolved.relative_to(root)
        except ValueError:
            continue
        if resolved.is_file():
            try:
                total_lines = len(resolved.read_text(encoding="utf-8").splitlines())
            except (OSError, UnicodeError) as exc:
                raise SOWRefusal(f"cannot read challenge citation {citation!r}") from exc
            if line <= total_lines:
                return resolved, line
    return None


def _validate_citation(citation: str, root: Path) -> None:
    if not citation.strip():
        raise SOWRefusal("challenge citation is empty")
    if CORPUS_ID.fullmatch(citation):
        return
    if _resolve_file(citation, root) is None:
        raise SOWRefusal(f"challenge citation is not a real file:line or corpus id: {citation!r}")
