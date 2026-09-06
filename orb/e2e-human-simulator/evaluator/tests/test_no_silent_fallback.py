"""E3 (Track E, anti-mirage) — no-silent-fallback regression test.

The scar: every agent once reported "tested, latency good, works as imagined" while the app was
silently talking through on-device native TTS instead of Fish Audio. The fix already in the code
(`apps/mobile/src/voice/FishSpeech.ts`, `apps/mobile/src/App.tsx`) is currently held up only by
comments and convention — nothing fails if someone quietly reintroduces a native-TTS fallback or
flips `native_tts_fallback: false` to `true`.

This module is a STATIC, external, source-text regression test — not a live audio proof. It reads
`apps/mobile/src/**` as plain text (never imports it: `e2e-human-simulator` is a black-box tool by
charter — see its README) and asserts the textual invariants that make a silent fallback
impossible today. It is explicitly NOT a substitute for a live run: `e2e-human-simulator`'s
`provider-failure` journey (in `journeys.json` and `run_config.local.json`) is the dynamic
counterpart — a real simulator run with an injected `tts_provider_failure` fault, asserting
`spoken_failure: true` / `error_state: true` / `failure_reply_count: 1` on the captured trace. Run
both; this file exists because the dynamic journey alone does not fail if the *reason* it still
passes changes from "no fallback happened" to "a fallback happened but nobody logged it."

Track E does not own `apps/**` and does not edit it — only reads it.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

# e2e-human-simulator/evaluator/tests/test_no_silent_fallback.py -> repo root is 3 parents up.
_REPO_ROOT = Path(__file__).resolve().parents[3]
_FISH_SPEECH_PATH = _REPO_ROOT / "apps" / "mobile" / "src" / "voice" / "FishSpeech.ts"
_APP_TSX_PATH = _REPO_ROOT / "apps" / "mobile" / "src" / "App.tsx"

# Any of these appearing in the production speech call path is the exact regression this test
# exists to catch: a native/on-device synthesizer standing in for Fish Audio. Each pattern
# requires call/import SHAPE (a following `(`, or a quoted import specifier), not a bare name —
# both real files legitimately mention "AVSpeechSynthesizer" and "SpeechSynthesizer" BY NAME in
# prose comments explaining why the code must never call them (e.g. App.tsx: "Never fall back to
# AVSpeechSynthesizer: it ignores the selected Fish voice..."). A bare-substring denylist would
# false-positive on the very comment that documents the invariant this test protects.
NATIVE_TTS_DENYLIST_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("expo-speech import", re.compile(r"""from\s+['"]expo-speech['"]""")),
    ("expo-speech require", re.compile(r"""require\(\s*['"]expo-speech['"]\s*\)""")),
    ("react-native-tts import", re.compile(r"""from\s+['"]react-native-tts['"]""")),
    ("react-native-tts require", re.compile(r"""require\(\s*['"]react-native-tts['"]\s*\)""")),
    ("AVSpeechSynthesizer call", re.compile(r"\bAVSpeechSynthesizer\s*\(")),
    ("SpeechSynthesizer call", re.compile(r"\bSpeechSynthesizer\s*\(")),
    ("NativeModules.Tts usage", re.compile(r"\bNativeModules\.Tts\s*[.(]")),
    ("Speech.speak call", re.compile(r"\bSpeech\.speak\s*\(")),
)

# The exact events the production `speak()` fallback-disabled paths are known to log today
# (apps/mobile/src/App.tsx, `voice.fish_unavailable` / `voice.fish_send_failed`). A regression
# that silently drops one of these events without removing the fallback-disabled comment/pattern
# entirely would not be caught by a bare substring count, so the check below binds
# `native_tts_fallback: false` to one of these specific event names, not just "appears somewhere."
KNOWN_FALLBACK_DISABLED_EVENTS = ("voice.fish_unavailable", "voice.fish_send_failed")

_FALSE_FLAG_RE = re.compile(r"native_tts_fallback:\s*false", re.IGNORECASE)
_TRUE_FLAG_RE = re.compile(r"native_tts_fallback:\s*true", re.IGNORECASE)
# Captures the devLogger.log('event.name', { ... native_tts_fallback: false ... }) span so the
# flag can be attributed to a specific event name rather than merely counted in the file.
_LOG_CALL_WITH_FLAG_RE = re.compile(
    r"devLogger\.log\(\s*'([a-zA-Z0-9_.]+)'\s*,\s*\{(?P<fields>(?:[^{}]|\{[^{}]*\})*)\}",
    re.DOTALL,
)


class NoSilentFallbackFindings(list):
    """A list of human-readable violation strings; empty means the invariant holds."""


def check_no_silent_fallback(*, fish_speech_text: str, app_tsx_text: str) -> NoSilentFallbackFindings:
    """Pure function over source text — the same function the real-file tests below call and the
    self-test mutates, so there is exactly one place this invariant is implemented."""
    findings = NoSilentFallbackFindings()

    if "device-TTS fallback" not in fish_speech_text and "device TTS fallback" not in fish_speech_text:
        findings.append(
            "FishSpeech.ts no longer documents the no-device-TTS-fallback contract in a comment "
            "(expected text about a 'device-TTS fallback')."
        )

    for label, pattern in NATIVE_TTS_DENYLIST_PATTERNS:
        if pattern.search(fish_speech_text):
            findings.append(f"FishSpeech.ts references a native TTS API: {label}.")
        if pattern.search(app_tsx_text):
            findings.append(f"App.tsx references a native TTS API: {label}.")

    if _TRUE_FLAG_RE.search(app_tsx_text):
        findings.append(
            "App.tsx contains 'native_tts_fallback: true' — a silent fallback is being reported "
            "as enabled."
        )

    false_flag_count = len(_FALSE_FLAG_RE.findall(app_tsx_text))
    if false_flag_count < len(KNOWN_FALLBACK_DISABLED_EVENTS):
        findings.append(
            f"App.tsx has only {false_flag_count} 'native_tts_fallback: false' occurrence(s); "
            f"expected at least {len(KNOWN_FALLBACK_DISABLED_EVENTS)} "
            f"(one per {KNOWN_FALLBACK_DISABLED_EVENTS})."
        )

    # Attribute each false-flag occurrence to a devLogger.log(...) call and confirm the event name
    # is one of the known fallback-disabled events, not an unrelated log call that happens to
    # mention the same field name.
    attributed_events = set()
    for match in _LOG_CALL_WITH_FLAG_RE.finditer(app_tsx_text):
        event_name, fields = match.group(1), match.group("fields")
        if _FALSE_FLAG_RE.search(fields):
            attributed_events.add(event_name)
    missing = set(KNOWN_FALLBACK_DISABLED_EVENTS) - attributed_events
    if missing:
        findings.append(
            f"Expected devLogger.log(...) calls for {sorted(missing)} to carry "
            "'native_tts_fallback: false' in their fields object; not found by structural match "
            f"(events actually carrying the flag: {sorted(attributed_events)})."
        )
    unexpected = attributed_events - set(KNOWN_FALLBACK_DISABLED_EVENTS)
    if unexpected:
        # Not necessarily wrong (a new guarded call site is fine), but worth surfacing so the
        # known-events list above is kept honest rather than silently going stale.
        findings.append(
            f"INFO: additional event(s) also carry 'native_tts_fallback: false': {sorted(unexpected)}. "
            "If this is a deliberate new guarded call site, add it to KNOWN_FALLBACK_DISABLED_EVENTS "
            "in this file."
        )

    return findings


def _non_info(findings: NoSilentFallbackFindings) -> list[str]:
    return [f for f in findings if not f.startswith("INFO:")]


class NoSilentFallbackRegressionTests(unittest.TestCase):
    """Runs the static check against the REAL, current source files."""

    @classmethod
    def setUpClass(cls) -> None:
        if not _FISH_SPEECH_PATH.is_file():
            raise unittest.SkipTest(f"FishSpeech.ts not found at {_FISH_SPEECH_PATH}")
        if not _APP_TSX_PATH.is_file():
            raise unittest.SkipTest(f"App.tsx not found at {_APP_TSX_PATH}")
        cls.fish_speech_text = _FISH_SPEECH_PATH.read_text(encoding="utf-8")
        cls.app_tsx_text = _APP_TSX_PATH.read_text(encoding="utf-8")

    def test_no_silent_fallback_invariants_hold_on_real_source(self) -> None:
        findings = check_no_silent_fallback(
            fish_speech_text=self.fish_speech_text, app_tsx_text=self.app_tsx_text
        )
        blocking = _non_info(findings)
        self.assertEqual(
            blocking,
            [],
            "no-silent-fallback invariant violated:\n" + "\n".join(blocking),
        )

    def test_fish_speech_contract_comment_present(self) -> None:
        self.assertIn(
            "must never grow a device-TTS fallback",
            self.fish_speech_text,
            "FishSpeech.ts's explicit no-fallback contract comment is missing or was reworded.",
        )

    def test_app_tsx_never_falls_back_to_avspeechsynthesizer_comment(self) -> None:
        self.assertIn(
            "Never fall back to AVSpeechSynthesizer",
            self.app_tsx_text,
            "App.tsx's explicit no-fallback contract comment is missing or was reworded.",
        )


class SelfTestTheCheckCatchesAMutation(unittest.TestCase):
    """A check that never fires on realistic bad input is worse than no check: it is a false sense
    of coverage. This proves `check_no_silent_fallback` actually flags known-bad text, using
    synthetic in-memory mutations (never writing to `apps/**`).
    """

    GOOD_APP_TSX = """
            devLogger.log('voice.fish_unavailable', {
              tenant_id: tenantId,
              native_tts_fallback: false,
            }, 'error');
            devLogger.log('voice.fish_send_failed', {
              tenant_id: tenantId,
              native_tts_fallback: false,
              error: 'x',
            }, 'error');
            // Never fall back to AVSpeechSynthesizer: it ignores the selected Fish voice.
    """
    GOOD_FISH_SPEECH = "// must never grow a device-TTS fallback"

    def test_flags_a_flipped_boolean(self) -> None:
        mutated = self.GOOD_APP_TSX.replace(
            "native_tts_fallback: false,\n            }, 'error');\n            devLogger.log('voice.fish_send_failed'",
            "native_tts_fallback: true,\n            }, 'error');\n            devLogger.log('voice.fish_send_failed'",
        )
        self.assertNotEqual(mutated, self.GOOD_APP_TSX, "test setup bug: mutation did not change the text")
        findings = _non_info(
            check_no_silent_fallback(fish_speech_text=self.GOOD_FISH_SPEECH, app_tsx_text=mutated)
        )
        self.assertTrue(
            any("native_tts_fallback: true" in f for f in findings),
            f"flipped boolean was not caught; findings={findings}",
        )

    def test_flags_a_reintroduced_native_tts_call(self) -> None:
        mutated = self.GOOD_APP_TSX + "\n  await Speech.speak(text, { voice: voiceId });\n"
        findings = _non_info(
            check_no_silent_fallback(fish_speech_text=self.GOOD_FISH_SPEECH, app_tsx_text=mutated)
        )
        self.assertTrue(
            any("Speech.speak call" in f for f in findings),
            f"reintroduced native TTS call was not caught; findings={findings}",
        )

    def test_flags_a_removed_disabled_event(self) -> None:
        mutated = self.GOOD_APP_TSX.split("devLogger.log('voice.fish_send_failed'")[0]
        findings = _non_info(
            check_no_silent_fallback(fish_speech_text=self.GOOD_FISH_SPEECH, app_tsx_text=mutated)
        )
        self.assertTrue(
            any("voice.fish_send_failed" in f for f in findings),
            f"removed guarded call site was not caught; findings={findings}",
        )

    def test_flags_a_removed_contract_comment(self) -> None:
        findings = _non_info(
            check_no_silent_fallback(fish_speech_text="// nothing to see here", app_tsx_text=self.GOOD_APP_TSX)
        )
        self.assertTrue(
            any("device-TTS fallback" in f for f in findings),
            f"removed contract comment was not caught; findings={findings}",
        )

    def test_passes_on_the_good_fixture(self) -> None:
        findings = _non_info(
            check_no_silent_fallback(fish_speech_text=self.GOOD_FISH_SPEECH, app_tsx_text=self.GOOD_APP_TSX)
        )
        self.assertEqual(findings, [])


if __name__ == "__main__":
    unittest.main()
