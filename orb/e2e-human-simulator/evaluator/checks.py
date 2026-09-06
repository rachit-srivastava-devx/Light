"""Deterministic checks over an exported human-simulation trace."""

from __future__ import annotations

from dataclasses import dataclass, field
import re
from typing import Any, Iterable

from .trace_schema import Trace, TraceEvent, events_of, first_value

ALLOWED_FISH_TAGS = {
    "warm",
    "chuckle",
    "laugh",
    "excited",
    "friendly",
    "playful",
    "sigh",
    "emphasis",
    "pause",
    "short pause",
    "long pause",
    "curious",
    "gentle",
    "celebratory",
}
MAX_FISH_TAGS = {
    "chuckle": 1,
    "laugh": 1,
    "excited": 1,
    "friendly": 1,
    "playful": 1,
    "sigh": 1,
    "emphasis": 2,
    "pause": 3,
    "short pause": 3,
    "long pause": 1,
}
TAG_PATTERN = re.compile(r"\[([^\]]+)\]")
WORD_PATTERN = re.compile(r"[\w']+", re.UNICODE)
RESPONSE_EVENTS = {"assistant.response", "assistant.reply", "speech.response", "app.speak"}
STT_EVENTS = {"stt.final", "transcript.final"}
FAILURE_EVENTS = {"failure", "relay.failure", "gateway.failure", "tts.failure", "stt.failure"}

VALID_STATES = {"booting", "greeting", "idle", "listening", "processing", "speaking", "success", "error", "paused", "closed"}
ALLOWED_TRANSITIONS = {
    "booting": {"booting", "greeting", "idle", "listening", "error", "closed"},
    "greeting": {"greeting", "speaking", "idle", "error", "closed"},
    # A final transcript can arrive while the app's user-visible session remains idle. The relay
    # is the source of truth for the processing edge, so idle -> processing is valid. The app can
    # also proactively speak again after a completed greeting (idle -> speaking).
    "idle": {"idle", "listening", "processing", "speaking", "greeting", "paused", "error", "closed"},
    "listening": {"listening", "processing", "speaking", "idle", "paused", "error", "closed"},
    "processing": {"processing", "speaking", "success", "error", "idle", "closed"},
    "speaking": {"speaking", "idle", "listening", "success", "error", "paused", "closed"},
    "success": {"success", "idle", "speaking", "listening", "closed"},
    "error": {"error", "idle", "listening", "greeting", "closed"},
    "paused": {"paused", "idle", "listening", "closed"},
    "closed": {"closed"},
}


@dataclass(frozen=True)
class Finding:
    code: str
    severity: str
    message: str
    evidence: tuple[str, ...] = ()
    suggestion: str = ""


@dataclass
class CheckResult:
    name: str
    findings: list[Finding] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        return not any(finding.severity in {"error", "critical"} for finding in self.findings)


@dataclass
class Evaluation:
    trace: Trace
    checks: list[CheckResult]

    @property
    def findings(self) -> list[Finding]:
        return [finding for result in self.checks for finding in result.findings]

    @property
    def passed(self) -> bool:
        return not any(finding.severity in {"error", "critical"} for finding in self.findings)

    def counts(self) -> dict[str, int]:
        return {severity: sum(finding.severity == severity for finding in self.findings) for severity in ("critical", "error", "warn", "info")}


@dataclass(frozen=True)
class CheckConfig:
    max_word_error_rate: float = 0.35
    require_markup: bool = True
    max_response_chars: int = 480
    latency_budgets_ms: dict[str, float] = field(default_factory=lambda: {
        "stt": 2500,
        "gateway": 5000,
        "tts_first_audio": 3000,
        "first_response": 8000,
    })


def _finding(code: str, severity: str, message: str, *evidence: str, suggestion: str = "") -> Finding:
    return Finding(code, severity, message, tuple(evidence), suggestion)


def _label(event: TraceEvent) -> str:
    return f"line {event.line} seq {event.seq} ({event.event})"


def _data_text(event: TraceEvent) -> str:
    speech = event.data.get("speech") if isinstance(event.data.get("speech"), dict) else event.data
    value = first_value(speech, "spoken_text", "text", "plain_text", "response_text")
    return value if isinstance(value, str) else ""


def _turn_id(event: TraceEvent) -> str | None:
    value = first_value(event.data, "turn_id", "utterance_id", "request_id")
    return str(value) if value is not None else None


def _words(value: str) -> list[str]:
    return [word.lower() for word in WORD_PATTERN.findall(value)]


def word_error_rate(reference: str, hypothesis: str) -> float:
    expected, actual = _words(reference), _words(hypothesis)
    if not expected:
        return 0.0 if not actual else 1.0
    row = list(range(len(actual) + 1))
    for index, expected_word in enumerate(expected, start=1):
        next_row = [index]
        for actual_index, actual_word in enumerate(actual, start=1):
            cost = 0 if expected_word == actual_word else 1
            next_row.append(min(next_row[-1] + 1, row[actual_index] + 1, row[actual_index - 1] + cost))
        row = next_row
    return row[-1] / len(expected)


def _expectations(trace: Trace) -> dict[str, Any]:
    for event in trace.events:
        if event.event in {"run.expectations", "scenario.expectations"}:
            return event.data
    return {}


def check_schema(trace: Trace) -> CheckResult:
    result = CheckResult("trace schema")
    for error in trace.schema_errors:
        result.findings.append(_finding("TRACE_SCHEMA_INVALID", "error", error, suggestion="Fix the trace exporter before trusting downstream results."))
    if not trace.events:
        result.findings.append(_finding("TRACE_EMPTY", "critical", "Trace contains no valid events.", suggestion="Capture at least app boot, mic, transcript, response, and state events."))
    return result


def check_transcripts(trace: Trace, config: CheckConfig) -> CheckResult:
    result = CheckResult("transcript")
    expected = _expectations(trace).get("expected_transcripts", [])
    finals = events_of(trace, *STT_EVENTS)
    if not finals:
        result.findings.append(_finding("STT_NO_FINAL", "error", "No final STT transcript was captured.", suggestion="Emit one stt.final event per completed user turn."))
        return result
    for index, event in enumerate(finals):
        text = first_value(event.data, "text", "transcript")
        if not isinstance(text, str) or not text.strip():
            result.findings.append(_finding("STT_EMPTY", "error", "Final transcript is empty.", _label(event), suggestion="Log the provider final transcript before sending it to the gateway."))
            continue
        expected_text = event.data.get("expected_text")
        if expected_text is None and index < len(expected) and isinstance(expected[index], dict):
            expected_text = expected[index].get("text")
        if isinstance(expected_text, str) and expected_text.strip():
            rate = word_error_rate(expected_text, text)
            allowed = float(event.data.get("max_word_error_rate", config.max_word_error_rate))
            if rate > allowed:
                result.findings.append(_finding("STT_WORD_ERROR_RATE", "error", f"Transcript WER {rate:.2f} exceeds allowed {allowed:.2f}: {text!r}", _label(event), suggestion=f"Improve capture/provider settings; expected {expected_text!r}."))
    return result


def _nearest_after(events: list[TraceEvent], start: TraceEvent, names: set[str]) -> TraceEvent | None:
    start_ms = start.timestamp_ms()
    candidates = [event for event in events if event.event in names and event.seq > start.seq]
    if start_ms is not None:
        candidates = [event for event in candidates if event.timestamp_ms() is None or event.timestamp_ms() >= start_ms]
    return candidates[0] if candidates else None


def _duration_ms(start: TraceEvent, end: TraceEvent) -> float | None:
    a, b = start.timestamp_ms(), end.timestamp_ms()
    if a is None or b is None:
        return None
    return max(0.0, b - a)


def check_latency(trace: Trace, config: CheckConfig) -> CheckResult:
    result = CheckResult("latency")
    events = trace.ordered_events
    turn_starts = events_of(trace, "turn.end", "mic.end_of_turn", "user.turn.end", "inject.end_of_turn", "app.end_of_turn")
    finals = events_of(trace, *STT_EVENTS)
    responses = events_of(trace, *RESPONSE_EVENTS)
    gateways = events_of(trace, "gateway.response", "llm.response")
    tts = events_of(trace, "tts.started", "tts.first_audio", "relay_audio.first_chunk")
    for start in turn_starts:
        final = _nearest_after(events, start, STT_EVENTS)
        response = _nearest_after(events, start, RESPONSE_EVENTS)
        if final:
            duration = _duration_ms(start, final)
            if duration is not None and duration > config.latency_budgets_ms["stt"]:
                result.findings.append(_finding("LATENCY_STT", "error", f"STT took {duration:.0f}ms; budget is {config.latency_budgets_ms['stt']:.0f}ms.", _label(start), _label(final), suggestion="Measure capture buffering and provider round-trip separately; start partial STT earlier."))
        elif response:
            result.findings.append(_finding("LATENCY_STT_MISSING", "error", "A response arrived without a final transcript after the turn ended.", _label(start), _label(response), suggestion="Capture and log the final STT event before invoking response generation."))
        if response:
            duration = _duration_ms(start, response)
            if duration is not None and duration > config.latency_budgets_ms["first_response"]:
                result.findings.append(_finding("LATENCY_FIRST_RESPONSE", "error", f"First response took {duration:.0f}ms; budget is {config.latency_budgets_ms['first_response']:.0f}ms.", _label(start), _label(response), suggestion="Stream the first spoken response and expose gateway/provider latency."))
    for event in gateways:
        duration = event.data.get("latency_ms")
        if isinstance(duration, (int, float)) and duration > config.latency_budgets_ms["gateway"]:
            result.findings.append(_finding("LATENCY_GATEWAY", "error", f"Gateway reported {duration:.0f}ms; budget is {config.latency_budgets_ms['gateway']:.0f}ms.", _label(event), suggestion="Inspect provider latency and avoid serial backend calls on the critical path."))
    for event in tts:
        duration = event.data.get("latency_ms")
        if isinstance(duration, (int, float)) and duration > config.latency_budgets_ms["tts_first_audio"]:
            result.findings.append(_finding("LATENCY_TTS_FIRST_AUDIO", "error", f"First TTS audio reported {duration:.0f}ms; budget is {config.latency_budgets_ms['tts_first_audio']:.0f}ms.", _label(event), suggestion="Stream audio as soon as the first TTS chunk is ready."))
    return result


def check_states(trace: Trace) -> CheckResult:
    result = CheckResult("orb state")
    states: list[tuple[TraceEvent, str]] = []
    for event in events_of(trace, "orb.state", "ui.state"):
        state = first_value(event.data, "state", "orb_state")
        if not isinstance(state, str) or state not in VALID_STATES:
            result.findings.append(_finding("STATE_UNKNOWN", "error", f"Unknown orb state {state!r}.", _label(event), suggestion=f"Use one of: {', '.join(sorted(VALID_STATES))}."))
            continue
        states.append((event, state))
    if not states:
        result.findings.append(_finding("STATE_NO_EVENTS", "error", "No orb state events were captured.", suggestion="Log every user-visible state transition with a timestamp."))
        return result
    if states[0][1] not in {"booting", "greeting", "idle"}:
        result.findings.append(_finding("STATE_BAD_INITIAL", "error", f"Initial orb state is {states[0][1]!r}.", _label(states[0][0]), suggestion="Start in booting, greeting, or idle before accepting speech."))
    for (previous_event, previous), (current_event, current) in zip(states, states[1:]):
        if current not in ALLOWED_TRANSITIONS[previous]:
            result.findings.append(_finding("STATE_INVALID_TRANSITION", "error", f"Invalid state transition {previous!r} → {current!r}.", _label(previous_event), _label(current_event), suggestion="Make the state machine emit an explicit intermediate state or correct the transition."))
    return result


def check_responses(trace: Trace, config: CheckConfig) -> CheckResult:
    result = CheckResult("response quality")
    responses = events_of(trace, *RESPONSE_EVENTS)
    finals = events_of(trace, *STT_EVENTS)
    if finals and not responses:
        result.findings.append(_finding("RESPONSE_MISSING", "error", "User speech produced no assistant response.", _label(finals[-1]), suggestion="Trace the final transcript into gateway completion, TTS, and playback."))
    seen: dict[str, TraceEvent] = {}
    for event in responses:
        text = _data_text(event).strip()
        if not text:
            result.findings.append(_finding("RESPONSE_EMPTY", "error", "Assistant response has no speakable text.", _label(event), suggestion="Reject empty model output and speak a clear recovery message."))
            continue
        plain = event.data.get("plain_text")
        if isinstance(event.data.get("speech"), dict):
            plain = event.data["speech"].get("plain_text", plain)
        if len(re.sub(TAG_PATTERN, "", text).strip()) > config.max_response_chars:
            result.findings.append(_finding("RESPONSE_TOO_LONG", "warn", f"Response exceeds {config.max_response_chars} characters.", _label(event), suggestion="Keep spoken turns short and split long explanations into steps."))
        key = re.sub(r"[^a-z0-9 ]", "", re.sub(TAG_PATTERN, "", text).lower()).strip()
        if key in seen:
            result.findings.append(_finding("RESPONSE_DUPLICATE", "error", "The same assistant response was repeated.", _label(seen[key]), _label(event), suggestion="Deduplicate by turn and use the current transcript/state when generating a response."))
        elif key:
            seen[key] = event
        if key in {"im here", "i am here", "im still here", "i am still here"}:
            result.findings.append(_finding("RESPONSE_GENERIC_PRESENCE", "warn", "Assistant used a generic presence response without evidence of user intent.", _label(event), suggestion="Acknowledge the user’s words or ask one useful follow-up instead of repeating presence copy."))
        if isinstance(plain, str) and TAG_PATTERN.search(plain):
            result.findings.append(_finding("RESPONSE_PLAIN_TEXT_MARKUP", "error", "plain_text still contains Fish markup.", _label(event), suggestion="Keep tagged spoken_text for TTS and strip tags from UI/log-facing plain_text."))
        if text.startswith("{") and "spoken_text" not in text:
            result.findings.append(_finding("RESPONSE_RAW_JSON", "warn", "Assistant response appears to expose a raw JSON object to speech.", _label(event), suggestion="Parse the speech envelope before sending text to TTS."))
    return result


def check_failure_speech(trace: Trace) -> CheckResult:
    result = CheckResult("failure speech")
    responses = events_of(trace, *RESPONSE_EVENTS)
    phrases = {
        "backend": ("unable to connect", "can't connect", "couldn't connect", "backend", "try again"),
        "provider": ("unable to connect", "couldn't connect", "try again", "temporarily unavailable"),
        "gateway": ("unable to connect", "couldn't connect", "try again"),
        "stt": ("couldn't understand", "couldn't make that out", "say that again", "try again"),
        "tts": ("unable to speak", "audio", "voice", "try again"),
        "mic": ("microphone", "mic", "permission", "allow"),
        "timeout": ("taking longer", "try again", "connect"),
    }
    for failure in events_of(trace, *FAILURE_EVENTS):
        kind = str(first_value(failure.data, "kind", "failure_type", "reason") or "backend").lower()
        expected = next((value for key, value in phrases.items() if key in kind), phrases["backend"])
        response = _nearest_after(trace.ordered_events, failure, RESPONSE_EVENTS)
        if response is None:
            result.findings.append(_finding("FAILURE_NOT_SPOKEN", "error", f"Failure {kind!r} had no spoken recovery response.", _label(failure), suggestion="Map every provider/relay failure to a short, human-readable spoken message."))
            continue
        text = _data_text(response).lower()
        if not any(phrase in text for phrase in expected):
            result.findings.append(_finding("FAILURE_SPEECH_WRONG", "error", f"Spoken response does not explain {kind!r} failure: {_data_text(response)!r}.", _label(failure), _label(response), suggestion="Speak what failed and what the user can do next; do not fall back to generic presence copy."))
    return result


def check_ui_evidence(trace: Trace) -> CheckResult:
    """Require evidence from the visible simulator, not only network events."""
    result = CheckResult("visible UI evidence")
    screenshots = events_of(trace, "ui.screenshot", "ui.frame")
    if not screenshots:
        result.findings.append(_finding("UI_SCREENSHOT_MISSING", "warn", "No visible simulator screenshot was linked to the trace.", suggestion="Capture at least launch, listening, speaking, and failure frames from the physical simulator."))
    for screenshot in screenshots:
        prompt = str(first_value(screenshot.data, "blocking_prompt", "prompt", "visible_prompt", "observation", "ocr_text") or "").lower()
        if "mic error" in prompt or "microphone error" in prompt or "microphone initialization" in prompt:
            if screenshot.data.get("known_simulator_mic_error") is True:
                result.findings.append(_finding("UI_MIC_ERROR_KNOWN_SIMULATOR", "warn", "The simulator has no device mic input; injected PCM remains testable.", _label(screenshot), suggestion="Keep this exception scoped to the injected simulator lane."))
            else:
                result.findings.append(_finding("UI_MIC_ERROR", "error", "The visible app is showing a microphone error state.", _label(screenshot), suggestion="Fix native microphone initialization and expose the concrete error to the user."))
        elif "microphone" in prompt or "permission" in prompt:
            result.findings.append(_finding("UI_PERMISSION_BLOCKED", "error", "The visible app is blocked by a microphone permission prompt.", _label(screenshot), suggestion="Complete the permission journey before evaluating speech or orb states."))
    states = events_of(trace, "orb.state", "ui.state")
    if states and not any(event.data.get("ui_confirmed") is True for event in states):
        result.findings.append(_finding("STATE_UI_NOT_CONFIRMED", "warn", "Orb state events are transport-inferred; no screenshot or UI instrumentation confirms the animation state.", _label(states[0]), suggestion="Capture a screenshot per state or add a UI test-only accessibility state surface."))
    return result


def check_fish_markup(trace: Trace, config: CheckConfig) -> CheckResult:
    result = CheckResult("Fish markup")
    for event in events_of(trace, *RESPONSE_EVENTS):
        speech = event.data.get("speech") if isinstance(event.data.get("speech"), dict) else event.data
        spoken = speech.get("spoken_text") if isinstance(speech, dict) else None
        if not isinstance(spoken, str):
            if config.require_markup:
                result.findings.append(_finding("FISH_MARKUP_MISSING_SPOKEN_TEXT", "warn", "Response has no spoken_text field to send to Fish Audio.", _label(event), suggestion="Keep a typed speech envelope and pass spoken_text to TTS."))
            continue
        tags = [match.group(1).strip().lower() for match in TAG_PATTERN.finditer(spoken)]
        unknown = sorted(set(tags) - ALLOWED_FISH_TAGS)
        if unknown:
            result.findings.append(_finding("FISH_MARKUP_UNKNOWN_TAG", "error", f"Unsupported Fish tag(s): {', '.join(unknown)}.", _label(event), suggestion=f"Allow only: {', '.join(sorted(ALLOWED_FISH_TAGS))}."))
        for tag, maximum in MAX_FISH_TAGS.items():
            if tags.count(tag) > maximum:
                result.findings.append(_finding("FISH_MARKUP_OVERUSED", "warn", f"Fish tag [{tag}] appears {tags.count(tag)} times; maximum is {maximum}.", _label(event), suggestion="Use markup as prosody, not as decoration on every phrase."))
        plain = speech.get("plain_text")
        if isinstance(plain, str) and TAG_PATTERN.search(plain):
            result.findings.append(_finding("FISH_MARKUP_NOT_STRIPPED", "error", "Fish tags leaked into plain_text.", _label(event), suggestion="Strip bracket tags for UI and transcript comparison while retaining them for TTS."))
        if config.require_markup and not tags:
            result.findings.append(_finding("FISH_MARKUP_ABSENT", "warn", "Response contains no expressive Fish markup.", _label(event), suggestion="Add one natural prosody cue such as [emphasis], [chuckle], or [short pause] when appropriate."))
    return result


def evaluate(trace: Trace, config: CheckConfig | None = None) -> Evaluation:
    config = config or CheckConfig()
    checks = [
        check_schema(trace),
        check_transcripts(trace, config),
        check_latency(trace, config),
        check_states(trace),
        check_responses(trace, config),
        check_failure_speech(trace),
        check_fish_markup(trace, config),
        check_ui_evidence(trace),
    ]
    return Evaluation(trace, checks)
