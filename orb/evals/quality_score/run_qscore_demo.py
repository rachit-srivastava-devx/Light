"""E4 (Track E) — run Q on real evidence, then make the hard-veto effect observable.

Usage:
    evals/.venv/bin/python evals/quality_score/run_qscore_demo.py
"""

from __future__ import annotations

import json
from pathlib import Path

from qscore import QReport, VetoState, compute_q

REPO_ROOT = Path(__file__).resolve().parents[2]
REAL_TRACE = (
    REPO_ROOT
    / "e2e-human-simulator"
    / "runs"
    / "live-20260807"
    / "trace"
    / "trace-final.jsonl"
)
REAL_FISH_AUDIO = (
    REPO_ROOT / "evals" / "quality_score" / "samples" / "fish_tts_sample.wav"
)


def main() -> int:
    print("=== Q, real evidence (real trace + real Fish audio, no veto) ===")
    real = compute_q(
        veto=VetoState(),
        # The two REAL /v1/respond latencies this track's own E2 smoke script measured against
        # the live backend chain today (see the Track E report) — not synthetic numbers.
        turn_latencies_ms=[979.0, 1238.0],
        trace_path=str(REAL_TRACE) if REAL_TRACE.is_file() else None,
        naturalness_audio_path=str(REAL_FISH_AUDIO)
        if REAL_FISH_AUDIO.is_file()
        else None,
    )
    print(json.dumps(real.to_dict(), indent=2))

    # The real session above already has Q=0 because repair_success=0. To demonstrate that a veto
    # independently collapses a non-zero score, reuse the real measured naturalness result alone:
    # 0.5407 without a veto, exactly 0.0 with a fake/memory-adapter veto. This is a veto-mechanics
    # proof, not a second end-to-end Q claim.
    measured_naturalness = next(
        score for score in real.subscores if score.name == "naturalness"
    )
    pre_veto = QReport(veto=VetoState(), subscores=[measured_naturalness])
    print("\n=== Veto mechanics control (real measured naturalness, no veto) ===")
    print(json.dumps(pre_veto.to_dict(), indent=2))

    print("\n=== Same real measured naturalness, fake/memory-adapter veto ===")
    vetoed = QReport(veto=VetoState(fake_adapter_used=True), subscores=[measured_naturalness])
    print(json.dumps(vetoed.to_dict(), indent=2))
    assert pre_veto.q > 0.0, "control score must be non-zero before applying veto"
    assert vetoed.q == 0.0, "veto must force Q=0 regardless of good sub-scores"

    print(f"\ntrace used: {REAL_TRACE} (exists={REAL_TRACE.is_file()})")
    print(f"Fish audio used: {REAL_FISH_AUDIO} (exists={REAL_FISH_AUDIO.is_file()})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
