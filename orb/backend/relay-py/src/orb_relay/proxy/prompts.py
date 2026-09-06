"""Loads the domain's versioned agent prompts from disk (`domain/agents/*.md`).

Placement rule (product `AGENTS.md` / `CLAUDE.md`: "Domain belongs in `domain/`; reusable behavior
belongs in the registry"): ADHD-specific prompt text must live in `domain/`, never be inlined in
backend source. Before this module existed, `/v1/respond`'s system prompt was a literal string in
`app.py` — this is the fix. It mirrors the loading pattern `eval/gates.py` already uses for
`domain/evalsets/` and `domain/policies/` JSON fixtures: `Path(__file__).resolve().parents[N]` up to
the product root, no new runtime dependency (C3).
"""

from __future__ import annotations

import json
from functools import cache
from pathlib import Path

# backend/relay-py/src/orb_relay/proxy/prompts.py -> product root is 5 parents up (same depth as
# eval/gates.py's LOCAL_CORPUS_PATH/POLICY_PATH, which use parents[5] for the identical reason).
DOMAIN_AGENTS_DIR = Path(__file__).resolve().parents[5] / "domain" / "agents"


class PromptNotFoundError(RuntimeError):
    """A required domain prompt file is missing or empty. Fail closed: never silently fall back to
    inlined text, and never serve an empty system prompt as if it were a real one.
    """


@cache
def load_agent_prompt(name: str) -> str:
    """Load and cache one versioned prompt file's full text.

    `name` is the file stem, e.g. `"teach.v1"` -> `domain/agents/teach.v1.md`. Cached because this
    is read on every `/v1/respond` call; a new prompt revision ships as a new filename
    (`teach.v2.md`), never an in-place edit of `teach.v1.md` (see `domain/README.md`), so the cache
    can safely live for the process lifetime.
    """
    path = DOMAIN_AGENTS_DIR / f"{name}.md"
    try:
        text = path.read_text(encoding="utf-8").strip()
    except FileNotFoundError as exc:
        raise PromptNotFoundError(f"domain agent prompt not found: {path}") from exc
    if not text:
        raise PromptNotFoundError(f"domain agent prompt is empty: {path}")
    return text


@cache
def load_wait_companion_turns(name: str = "wait-companion.v1") -> tuple[str, ...]:
    """Load the domain-owned wait pool and reject empty or duplicate spoken turns.

    The pool is data, not a model prompt: wait company must be available during provider failure and
    must replay identically. Normalizing whitespace/case here matches the listener-facing repeat
    invariant without importing the conversation guard back into this leaf module.
    """
    path = DOMAIN_AGENTS_DIR / f"{name}.json"
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise PromptNotFoundError(f"wait companion pool not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise PromptNotFoundError(f"wait companion pool is invalid JSON: {path}: {exc}") from exc

    turns = raw.get("turns") if isinstance(raw, dict) else None
    if not isinstance(turns, list) or not turns:
        raise PromptNotFoundError(f"wait companion pool has no turns: {path}")
    cleaned = tuple(
        " ".join(turn.split()) for turn in turns if isinstance(turn, str) and turn.strip()
    )
    keys = tuple(turn.casefold() for turn in cleaned)
    if len(cleaned) != len(turns) or len(set(keys)) != len(keys):
        raise PromptNotFoundError(
            f"wait companion pool has empty, non-string, or duplicate turns: {path}"
        )
    return cleaned
