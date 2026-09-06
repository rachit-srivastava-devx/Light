"""proxy/prompts.py: the domain/agents/*.md loader (contract C1 — prompts live in domain/, never
inlined in app.py).
"""

import pytest

import orb_relay.proxy.prompts as prompts_module
from orb_relay.proxy.prompts import PromptNotFoundError, load_agent_prompt


@pytest.fixture(autouse=True)
def clear_prompt_cache():
    """The loader is `lru_cache`d at module scope; several tests here monkeypatch
    `DOMAIN_AGENTS_DIR`, so the cache must not leak a fixture's fake content into a later test (or
    a real file's content into an earlier assertion about a fixture).
    """
    load_agent_prompt.cache_clear()
    yield
    load_agent_prompt.cache_clear()


def test_loads_real_teach_prompt_from_domain_agents_dir() -> None:
    text = load_agent_prompt("teach.v1")
    on_disk = (prompts_module.DOMAIN_AGENTS_DIR / "teach.v1.md").read_text(encoding="utf-8").strip()
    assert text == on_disk
    assert "check" in text.lower()  # "checks understanding" — contract C5's language slot


def test_loads_real_converse_and_focus_companion_prompts() -> None:
    assert "any topic" in load_agent_prompt("converse.v1").lower()
    assert load_agent_prompt("focus-companion.v1")  # non-empty, no crash


def test_missing_prompt_fails_closed_not_silently(tmp_path, monkeypatch) -> None:
    monkeypatch.setattr(prompts_module, "DOMAIN_AGENTS_DIR", tmp_path)
    with pytest.raises(PromptNotFoundError):
        load_agent_prompt("does-not-exist.v1")


def test_empty_prompt_file_fails_closed(tmp_path, monkeypatch) -> None:
    (tmp_path / "blank.v1.md").write_text("   \n  ", encoding="utf-8")
    monkeypatch.setattr(prompts_module, "DOMAIN_AGENTS_DIR", tmp_path)
    with pytest.raises(PromptNotFoundError):
        load_agent_prompt("blank.v1")


def test_prompt_is_genuinely_read_from_disk_not_hardcoded(tmp_path, monkeypatch) -> None:
    """The strong version of C1: prove the runtime path reads the file's *content*, not merely
    that a correctly-named file exists somewhere.
    """
    marker = "UNIQUE_MARKER_38271: this text only exists in this temp fixture file."
    (tmp_path / "fixture.v1.md").write_text(marker, encoding="utf-8")
    monkeypatch.setattr(prompts_module, "DOMAIN_AGENTS_DIR", tmp_path)
    assert load_agent_prompt("fixture.v1") == marker


def test_prompt_is_cached_across_calls(tmp_path, monkeypatch) -> None:
    (tmp_path / "cached.v1.md").write_text("version A", encoding="utf-8")
    monkeypatch.setattr(prompts_module, "DOMAIN_AGENTS_DIR", tmp_path)
    first = load_agent_prompt("cached.v1")
    (tmp_path / "cached.v1.md").write_text("version B", encoding="utf-8")
    second = load_agent_prompt("cached.v1")
    assert first == second == "version A"  # the on-disk rewrite is not observed -- caching, by design
