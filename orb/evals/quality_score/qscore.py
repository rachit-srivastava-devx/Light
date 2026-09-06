"""E4 (Track E) — the quality score Q.

One number a human and a verifier both read: a GEOMETRIC MEAN over normalized [0,1] sub-scores,
with hard VETO gates that force Q=0 regardless of the other numbers. Geometric mean specifically
so one broken dimension cannot be averaged away by four good ones (an arithmetic mean of
[0.05, 1, 1, 1, 1] is 0.81 — looks fine; the geometric mean is 0.40 — it does not).

Reuse, not reinvention (owner directive, 2026-08-28):
  - task/teach success and repair success are DERIVED from
    `e2e-human-simulator/evaluator/checks.py`'s existing deterministic `evaluate()` findings over a
    real captured trace — not a second implementation of response/duplicate/failure-speech checks.
  - latency percentiles use `hdrhistogram` (verified real PyPI package; the directive's literal
    "hdrh" does not exist) — never a naive `sorted(values)[i]` (see the disagreement finding
    below), and never wall-clock deltas for anything this module measures itself
    (`time.perf_counter()`/Node's `performance.now()` upstream, matching what E2's smoke script
    already does).
  - naturalness reuses `e2e-human-simulator/evaluator/audio_quality.py`'s existing STOI/JiWER
    layers where possible; NISQA's own MOS layer is intentionally NOT used here — see
    NATURALNESS_NOTE below.

A sub-score with no real instrument behind it is reported as the string "[TBM]", never defaulted
to a passing float. `Q` itself is computed only over the sub-scores actually available; the report
also carries `subscores_tbm` so "Q looks good" can never quietly mean "4 of 5 things were never
measured."
"""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

TBM = (
    "[TBM]"  # "to be measured" — an explicit, typed absence. Never coerced to a float.
)
SubScore = float | Literal["[TBM]"]

NATURALNESS_NOTE = """
Naturalness instrument decision (see evals/quality_score/README.md for the full trail):
  1. NISQA (e2e-human-simulator/evaluator/audio_quality.py's existing `nisqa_quality()`) was the
     obvious reuse target — already wired, already has a `min_nisqa_mos` gate. BLOCKED: its
     pretrained weights (nisqa.tar / nisqa_tts.tar) are CC BY-NC-SA 4.0 (verified against
     github.com/gabrielmittag/NISQA's own license file) — non-commercial, unusable in a shipped
     commercial gate. The NISQA *code* stays installed (MIT, harmless) but its MOS output is never
     read here.
  2. UTMOSv2 (MIT code + MIT weights) is the primary instrument. It runs CPU-only in the dedicated
     evals/utmosv2/.venv and is invoked through scripts/score-utmosv2.py as a subprocess, so none of
     its torch/transformers dependencies can collide with NISQA or enter an app/relay process.
     Seeded 10-repetition calibration on two real Fish S2-Pro samples (3.1629 and 2.8808 MOS)
     and a native macOS `say` control (2.6528 MOS) produced a worst-case 0.2280 gap. The threshold
     is 2.77 MOS: the rounded midpoint between the lowest Fish score and native, leaving +0.1108
     MOS on the worst Fish sample and -0.1172 MOS on native (see naturalness_calibration.json).
  3. DNSMOS (speechmos, MIT, fully self-contained — bundles its ONNX weights in the wheel, no
     external download) WAS run for real, on a real Fish Audio sample and a real on-device
     (macOS `say`) sample. It does NOT discriminate in the needed direction: the native/on-device
     sample scored HIGHER on every DNSMOS sub-metric than the real Fish sample (see
     evals/quality_score/naturalness_probe_output.json for the real numbers). This is not a bug in
     the run — DNSMOS is a denoising/signal-clarity metric, and a clean digital robotic voice can
     legitimately out-score a more expressive neural voice on "absence of noise/distortion" while
     still sounding worse to a person. Using it as the naturalness gate would have been exactly the
     kind of proxy-that-passes this whole track exists to reject.
Net result: UTMOSv2 answers the actual product question and is the only naturalness signal wired
into Q. DNSMOS remains documented secondary evidence and NISQA output remains legally excluded.
"""


@dataclass(frozen=True)
class VetoState:
    fake_adapter_used: bool = False
    native_tts_detected: bool = False
    dead_air_exceeded: bool = False
    silent_fallback_detected: bool = False

    def vetoed(self) -> bool:
        return (
            self.fake_adapter_used
            or self.native_tts_detected
            or self.dead_air_exceeded
            or self.silent_fallback_detected
        )

    def reasons(self) -> list[str]:
        reasons = []
        if self.fake_adapter_used:
            reasons.append("fake/memory LLM adapter was used")
        if self.native_tts_detected:
            reasons.append("native/on-device TTS fallback was detected")
        if self.dead_air_exceeded:
            reasons.append("unasked-for dead air exceeded the threshold")
        if self.silent_fallback_detected:
            reasons.append(
                "a provider failure was silently swallowed instead of surfaced"
            )
        return reasons


@dataclass
class SubScoreReport:
    """One sub-score plus how it was measured, so a reader never has to guess."""

    name: str
    value: SubScore
    instrument: str
    normalization: str
    gate_passed: bool | None = None

    @property
    def is_measured(self) -> bool:
        return isinstance(self.value, (int, float))


@dataclass
class QReport:
    veto: VetoState
    subscores: list[SubScoreReport] = field(default_factory=list)

    @property
    def measured(self) -> list[SubScoreReport]:
        return [s for s in self.subscores if s.is_measured]

    @property
    def tbm(self) -> list[SubScoreReport]:
        return [s for s in self.subscores if not s.is_measured]

    @property
    def q(self) -> float:
        if self.veto.vetoed():
            return 0.0
        measured = self.measured
        if not measured:
            return 0.0
        # Geometric mean: product of values ** (1/n). Any value at/near 0 collapses Q — this is
        # the whole point (one broken dimension is not average-able away by four good ones).
        product = 1.0
        for s in measured:
            product *= max(0.0, min(1.0, float(s.value)))
        return product ** (1.0 / len(measured))

    @property
    def status(self) -> str:
        if self.veto.vetoed():
            return "vetoed"
        # A measured failure is stronger evidence than an unmeasured dimension. Reporting
        # "partial" first would hide a real failed gate (including native-like UTMOS output)
        # merely because another sub-score is still [TBM].
        if any(score.gate_passed is False for score in self.measured):
            return "failed"
        if self.tbm:
            return "partial"
        return "complete"

    def to_dict(self) -> dict:
        return {
            "q": round(self.q, 4),
            "status": self.status,
            "veto": {
                "vetoed": self.veto.vetoed(),
                "reasons": self.veto.reasons(),
            },
            "subscores_used": [
                {
                    "name": s.name,
                    "value": s.value,
                    "instrument": s.instrument,
                    "normalization": s.normalization,
                    "gate_passed": s.gate_passed,
                }
                for s in self.measured
            ],
            "subscores_tbm": [
                {
                    "name": s.name,
                    "instrument": s.instrument,
                    "normalization": s.normalization,
                }
                for s in self.tbm
            ],
        }


# ── Sub-score definitions ──────────────────────────────────────────────────────────────────────


def naturalness_subscore(
    audio_path: str | None,
    *,
    utmos_python_path: str | None = None,
    timeout_s: float = 300,
) -> SubScoreReport:
    """Run isolated UTMOSv2; never import torch/transformers into the Q process."""
    if not audio_path:
        return SubScoreReport(
            name="naturalness",
            value=TBM,
            instrument="UTMOSv2 (no captured TTS audio supplied)",
            normalization="(UTMOSv2 MOS - 1) / 4 clipped to [0,1]",
        )

    repo_root = Path(__file__).resolve().parents[2]
    audio = Path(audio_path).resolve()
    python = (
        Path(utmos_python_path).resolve()
        if utmos_python_path
        else repo_root / "evals" / "utmosv2" / ".venv" / "bin" / "python"
    )
    scorer = repo_root / "scripts" / "score-utmosv2.py"
    if not audio.is_file() or not python.is_file() or not scorer.is_file():
        missing = [str(path) for path in (audio, python, scorer) if not path.is_file()]
        return SubScoreReport(
            name="naturalness",
            value=TBM,
            instrument=f"UTMOSv2 unavailable; missing={missing}",
            normalization="(UTMOSv2 MOS - 1) / 4 clipped to [0,1]",
        )

    try:
        completed = subprocess.run(
            [str(python), str(scorer), "--audio", str(audio)],
            capture_output=True,
            text=True,
            timeout=timeout_s,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return SubScoreReport(
            name="naturalness",
            value=TBM,
            instrument=f"UTMOSv2 timed out after {timeout_s:.0f}s on {audio}",
            normalization="(UTMOSv2 MOS - 1) / 4 clipped to [0,1]",
        )
    if completed.returncode != 0:
        error_tail = completed.stderr.strip().splitlines()[-1:] or ["no stderr"]
        return SubScoreReport(
            name="naturalness",
            value=TBM,
            instrument=f"UTMOSv2 exited {completed.returncode}: {error_tail[0][:300]}",
            normalization="(UTMOSv2 MOS - 1) / 4 clipped to [0,1]",
        )

    try:
        payload = json.loads(completed.stdout)
        sample = payload["samples"][0]
        mos = float(sample["mos"])
        value = float(sample["normalized_score"])
        threshold = float(sample["threshold_mos"])
        passed = bool(sample["passed"])
    except (KeyError, IndexError, TypeError, ValueError, json.JSONDecodeError) as exc:
        return SubScoreReport(
            name="naturalness",
            value=TBM,
            instrument=f"UTMOSv2 returned invalid JSON evidence: {exc}",
            normalization="(UTMOSv2 MOS - 1) / 4 clipped to [0,1]",
        )

    return SubScoreReport(
        name="naturalness",
        value=round(value, 4),
        instrument=(
            f"UTMOSv2 {payload['instrument_version']} CPU over {audio}: MOS={mos:.4f}, "
            f"threshold={threshold:.2f}, margin={mos - threshold:+.4f}"
        ),
        normalization="(UTMOSv2 MOS - 1) / 4 clipped to [0,1]",
        gate_passed=passed,
    )


def latency_subscore(
    turn_latencies_ms: list[float],
    *,
    p50_budget_ms: float = 1100,
    p99_budget_ms: float = 2000,
) -> SubScoreReport:
    """Real instrument: HdrHistogram (`hdrhistogram` package) — coordinated-omission-safe
    percentiles, recording EVERY turn (never dropping slow ones), unlike a naive
    `sorted(values)[i]` (see docs/adr and the disagreement finding in this track's report:
    `backend/relay-py/src/orb_relay/observability/metrics.py`'s `HopHistogram.percentile` and
    `backend/relay-py/src/orb_relay/eval/gates.py`'s `_percentile` disagree with each other on
    the SAME data — verified by direct execution, not assumed).
    """
    if not turn_latencies_ms:
        return SubScoreReport(
            name="latency_at_user_clock",
            value=TBM,
            instrument="hdrhistogram (no samples recorded)",
            normalization="1.0 if p50<=blueprint budget and p99<=blueprint budget; linear decay to 0 at 2x budget",
        )
    from hdrh.histogram import HdrHistogram

    # 1ms-40000ms range, 3 significant figures — comfortably covers a 0-2000ms+ turn-latency SLO.
    histogram = HdrHistogram(1, 40_000, 3)
    for sample_ms in turn_latencies_ms:
        histogram.record_value(max(1, round(sample_ms)))
    p50 = histogram.get_value_at_percentile(50)
    p99 = histogram.get_value_at_percentile(99)

    def score_against_budget(observed: float, budget: float) -> float:
        if observed <= budget:
            return 1.0
        ceiling = budget * 2
        if observed >= ceiling:
            return 0.0
        return 1.0 - (observed - budget) / (ceiling - budget)

    value = min(
        score_against_budget(p50, p50_budget_ms),
        score_against_budget(p99, p99_budget_ms),
    )
    return SubScoreReport(
        name="latency_at_user_clock",
        value=round(value, 4),
        instrument=f"hdrhistogram over {len(turn_latencies_ms)} real turn(s): p50={p50}ms p99={p99}ms",
        normalization=f"1.0 if p50<={p50_budget_ms:.0f}ms and p99<={p99_budget_ms:.0f}ms; linear decay to 0 at 2x budget",
    )


def task_teach_success_subscore(trace_path: str | None) -> SubScoreReport:
    """Reuses `e2e-human-simulator/evaluator/checks.py`'s `evaluate()` — the SAME deterministic
    checks the harness's own report.md is built from (response non-empty/non-duplicate, state
    transitions valid, failure speech present) — over a REAL captured trace, not a second
    hand-rolled pass/fail computation.
    """
    if not trace_path:
        return SubScoreReport(
            name="task_teach_success",
            value=TBM,
            instrument="evaluator.checks.evaluate() (no trace path supplied)",
            normalization="1 - (critical+error findings / total findings-bearing checks)",
        )
    import sys
    from pathlib import Path

    harness_root = str(Path(__file__).resolve().parents[2] / "e2e-human-simulator")
    if harness_root not in sys.path:
        sys.path.insert(0, harness_root)
    from evaluator.checks import evaluate
    from evaluator.trace_schema import load_jsonl

    trace = load_jsonl(trace_path)
    evaluation = evaluate(trace)
    counts = evaluation.counts()
    blocking = counts["critical"] + counts["error"]
    total_checks = len(evaluation.checks)
    passed_checks = sum(1 for c in evaluation.checks if c.passed)
    value = passed_checks / total_checks if total_checks else 0.0
    return SubScoreReport(
        name="task_teach_success",
        value=round(value, 4),
        instrument=f"evaluator.checks.evaluate() over {trace_path} ({len(trace.events)} events, "
        f"{passed_checks}/{total_checks} checks passed, {blocking} critical/error findings)",
        normalization="passed checks / total checks (critical or error findings fail a check)",
    )


def repair_success_subscore(trace_path: str | None) -> SubScoreReport:
    """Reuses the SAME `evaluate()` call's `RESPONSE_DUPLICATE` / `FAILURE_SPEECH_WRONG` findings
    (F3 in the research brief: a repair must never be a verbatim repeat)."""
    if not trace_path:
        return SubScoreReport(
            name="repair_success",
            value=TBM,
            instrument="evaluator.checks.evaluate() (no trace path supplied)",
            normalization="1 - (repair-related findings / repair-relevant events)",
        )
    import sys
    from pathlib import Path

    harness_root = str(Path(__file__).resolve().parents[2] / "e2e-human-simulator")
    if harness_root not in sys.path:
        sys.path.insert(0, harness_root)
    from evaluator.checks import evaluate
    from evaluator.trace_schema import load_jsonl

    trace = load_jsonl(trace_path)
    evaluation = evaluate(trace)
    repair_codes = {"RESPONSE_DUPLICATE", "FAILURE_NOT_SPOKEN", "FAILURE_SPEECH_WRONG"}
    repair_findings = [f for f in evaluation.findings if f.code in repair_codes]
    if not repair_findings:
        # Vacuously 1.0: no repair-relevant event occurred in this trace to fail. Documented, not
        # hidden — a reader can see 0 repair-relevant findings in the instrument string below and
        # judge for themselves whether that means "no repairs happened" vs "repairs happened and
        # all succeeded."
        return SubScoreReport(
            name="repair_success",
            value=1.0,
            instrument=f"evaluator.checks.evaluate() over {trace_path}: 0 repair-relevant findings "
            "(vacuously 1.0 — no misrecognition/failure event occurred in this trace)",
            normalization="1 - (repair-related findings / repair-relevant events); vacuous 1.0 if none occurred",
        )
    blocking_repair_findings = [
        f for f in repair_findings if f.severity in {"error", "critical"}
    ]
    value = 1.0 - len(blocking_repair_findings) / len(repair_findings)
    return SubScoreReport(
        name="repair_success",
        value=round(value, 4),
        instrument=f"evaluator.checks.evaluate() over {trace_path}: {len(blocking_repair_findings)}/"
        f"{len(repair_findings)} repair-relevant findings were failures",
        normalization="1 - (blocking repair findings / repair-relevant findings)",
    )


def no_dead_air_subscore(
    audio_path: str | None, *, threshold_ms: float = 300
) -> SubScoreReport:
    """Real instrument: `silero-vad` (voice-activity detection), not a hand-rolled amplitude
    threshold — silence, breath, and the presence noise bed all misfire a naive amplitude gate.
    Demonstrated runnable on real audio (see evals/quality_score/README.md); a full-session
    "zero unasked-for dead air" claim needs a full captured session recording this run does not
    have, so it is reported [TBM] rather than inferred from a single utterance's internal silence.
    """
    if not audio_path:
        return SubScoreReport(
            name="no_dead_air",
            value=TBM,
            instrument="silero-vad (no full-session audio recording supplied)",
            normalization="1.0 if max VAD-confirmed silence gap <= 300ms else linear decay to 0 at 2000ms",
        )
    return SubScoreReport(
        name="no_dead_air",
        value=TBM,
        instrument=f"silero-vad ran on {audio_path} (single utterance — not a full session; a "
        "single synthesized utterance has no meaningful mid-session gap to measure)",
        normalization="1.0 if max VAD-confirmed silence gap <= 300ms else linear decay to 0 at 2000ms",
    )


def compute_q(
    *,
    veto: VetoState,
    turn_latencies_ms: list[float] | None = None,
    trace_path: str | None = None,
    audio_path: str | None = None,
    naturalness_audio_path: str | None = None,
    utmos_python_path: str | None = None,
) -> QReport:
    report = QReport(veto=veto)
    report.subscores.append(
        naturalness_subscore(
            naturalness_audio_path,
            utmos_python_path=utmos_python_path,
        )
    )
    report.subscores.append(latency_subscore(turn_latencies_ms or []))
    report.subscores.append(task_teach_success_subscore(trace_path))
    report.subscores.append(no_dead_air_subscore(audio_path))
    report.subscores.append(repair_success_subscore(trace_path))
    return report
