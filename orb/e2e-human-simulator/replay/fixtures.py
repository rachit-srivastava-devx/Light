"""JSON scripted utterance fixtures for black-box replay journeys."""

from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
from typing import Any, Mapping


@dataclass(frozen=True)
class ScriptedUtterance:
    """A human-like turn with an expected transcript and a relative WAV path."""

    id: str
    phrase: str
    wav: str
    expected_transcript: str
    tags: tuple[str, ...] = ()
    pause_before_ms: int = 0
    pause_after_ms: int = 0

    def resolve_wav(self, base_dir: str | Path) -> Path:
        """Resolve a fixture path without allowing it to escape the fixture directory."""

        root = Path(base_dir).resolve()
        candidate = (root / self.wav).resolve()
        if candidate != root and root not in candidate.parents:
            raise ValueError(f"fixture WAV escapes its directory: {self.wav}")
        return candidate


def _non_negative_int(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(f"{field} must be a non-negative integer")
    return value


def _utterance(raw: Mapping[str, Any], base_dir: Path) -> ScriptedUtterance:
    required = ("id", "expected_transcript", "wav")
    missing = [field for field in required if field not in raw]
    if missing:
        raise ValueError(f"utterance missing required fields: {', '.join(missing)}")
    identifier = raw["id"]
    phrase = raw.get("phrase", raw["expected_transcript"])
    transcript = raw["expected_transcript"]
    wav = raw["wav"]
    if not isinstance(identifier, str) or not identifier.strip():
        raise ValueError("utterance.id must be a non-empty string")
    if not isinstance(phrase, str) or not phrase.strip():
        raise ValueError(f"utterance {identifier!r} phrase must be non-empty")
    if not isinstance(transcript, str):
        raise ValueError(f"utterance {identifier!r} expected_transcript must be non-empty")
    if not isinstance(wav, str) or not wav.strip():
        raise ValueError(f"utterance {identifier!r} wav must be a non-empty path")

    tags = raw.get("tags", [])
    if not isinstance(tags, list) or not all(isinstance(tag, str) and tag.strip() for tag in tags):
        raise ValueError(f"utterance {identifier!r} tags must be an array of non-empty strings")
    return ScriptedUtterance(
        id=identifier,
        phrase=phrase,
        wav=wav,
        expected_transcript=transcript,
        tags=tuple(tags),
        pause_before_ms=_non_negative_int(raw.get("pause_before_ms", 0), "pause_before_ms"),
        pause_after_ms=_non_negative_int(raw.get("pause_after_ms", 0), "pause_after_ms"),
    )


def load_fixture(path: str | Path) -> list[ScriptedUtterance]:
    """Load and validate a versioned JSON fixture file."""

    fixture_path = Path(path)
    try:
        document = json.loads(fixture_path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValueError(f"fixture file does not exist: {fixture_path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"fixture is not valid JSON: {fixture_path}: {exc}") from exc

    if not isinstance(document, dict) or document.get("version") != 1:
        raise ValueError("fixture.version must be 1")
    raw_utterances = document.get("utterances")
    if not isinstance(raw_utterances, list) or not raw_utterances:
        raise ValueError("fixture.utterances must be a non-empty array")

    utterances: list[ScriptedUtterance] = []
    seen: set[str] = set()
    for index, raw in enumerate(raw_utterances):
        if not isinstance(raw, dict):
            raise ValueError(f"fixture.utterances[{index}] must be an object")
        item = _utterance(raw, fixture_path.parent)
        if item.id in seen:
            raise ValueError(f"duplicate utterance id: {item.id}")
        seen.add(item.id)
        utterances.append(item)
    return utterances


def load_fixture_manifest(path: str | Path) -> list[ScriptedUtterance]:
    """Compatibility name used by the replay CLI and evaluator."""

    return load_fixture(path)
