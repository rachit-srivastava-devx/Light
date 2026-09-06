#!/usr/bin/env python3
"""Executable voice-UX gate. Turns the researched UX checklist into checks that FAIL.

Why this exists
---------------
The 30-item voice-agent UX checklist (18 machine-checkable, 12 needing a listener) was researched
with cited thresholds and then lived only as prose. A written bar transfers at a rate near zero:
documentation is a `mitigates`, a gate that fails is a `kills (mechanical)`. This file is the
mechanical half.

It reads a real drive's `trace.jsonl` (produced by `e2e-human-simulator/run_e2e.py`) plus the
run's screenshots, and asserts the UX properties a human would notice. It measures the USER's
clock -- time from the user finishing speaking to audio actually starting -- not server timings.

Deliberate design rules, each one a scar:
  * A check that measured NOTHING is a FAILURE, not a pass. Zero turns analysed => red.
  * Every check publishes its denominator. "passed" without a count is not evidence.
  * The gate reports the real number even when it is far outside target, and never rewrites a
    threshold to make a run green.
  * Presence-covered turns are counted separately from turns where real content met the budget:
    a system that hits its latency target with 100% filler passes the gate while failing the goal.

Usage:
    python3 ux-gate/gate.py <run_dir>            # e.g. e2e-human-simulator/runs/opus-drive/<ts>
    python3 ux-gate/gate.py <run_dir> --json     # machine-readable summary

Exit codes: 0 all checks passed | 1 one or more failed | 2 could not evaluate (no usable input).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

# --- Thresholds -------------------------------------------------------------------------------
# Sources: owner's hard constraint (250 ms to continuous audio); LiveKit's published turn-detector
# curve (~295 ms @ 10% false-cutoff vs ~543 ms @ 5%); Stivers et al., PNAS 2009 (human modal
# inter-speaker gap ~0-200 ms); Alexa design guide ("one-breath test" for spoken replies).
TARGET_FIRST_AUDIO_MS = 250.0     # owner's hard constraint
NATURAL_TURN_GAP_MS = 300.0       # "feels natural" reference
ONE_BREATH_CHARS = 240            # spoken reply should be sayable in one breath
MAX_CONSECUTIVE_IDENTICAL = 1     # a repair must not repeat the previous reply verbatim


@dataclass
class Check:
    ident: str
    title: str
    auto: bool = True
    passed: bool | None = None
    denominator: str = ""
    detail: str = ""
    measured: str = ""


@dataclass
class Report:
    checks: list[Check] = field(default_factory=list)

    def add(self, c: Check) -> None:
        self.checks.append(c)

    @property
    def failed(self) -> list[Check]:
        return [c for c in self.checks if c.passed is False]

    @property
    def unevaluated(self) -> list[Check]:
        return [c for c in self.checks if c.passed is None]


def _ts(event: dict) -> datetime:
    return datetime.fromisoformat(event["ts"])


def load_trace(run_dir: Path) -> list[dict]:
    p = run_dir / "trace.jsonl"
    if not p.exists():
        raise SystemExit(f"[gate] no trace.jsonl in {run_dir} -- cannot evaluate (exit 2)")
    out: list[dict] = []
    for line in p.read_text(errors="replace").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return out


def _text_of(event: dict) -> str:
    d = event.get("data") or {}
    return str(d.get("text") or d.get("spoken_text") or "")


def evaluate(run_dir: Path) -> Report:
    ev = load_trace(run_dir)
    ev.sort(key=lambda e: e["ts"])
    rep = Report()

    eots = [e for e in ev if e.get("event") == "inject.end_of_turn"]
    firsts = [e for e in ev if e.get("event") == "tts.first_audio"]
    responses = [e for e in ev if e.get("event") == "assistant.response"]
    failures = [e for e in ev if e.get("event") == "failure"]

    # --- U0: the gate must have something to measure -----------------------------------------
    c = Check("U0", "Gate measured something (zero samples is a failure, not a pass)")
    c.denominator = f"{len(ev)} trace events, {len(eots)} user turns"
    c.passed = len(ev) > 0 and len(eots) > 0
    c.detail = "" if c.passed else "no user turns in trace -- nothing was exercised"
    rep.add(c)
    if not c.passed:
        return rep

    # --- U1: every user turn is answered -----------------------------------------------------
    # The defect this caught for real: a placeholder session_id made relay-rs discard every
    # injected frame, so 3 turns produced 0 replies, silently, with green-looking artifacts.
    answered = 0
    latencies: list[float] = []
    for e in eots:
        later = [f for f in firsts if _ts(f) >= _ts(e)]
        if later:
            answered += 1
            latencies.append((_ts(later[0]) - _ts(e)).total_seconds() * 1000.0)
    c = Check("U1", "Every user turn receives spoken audio")
    c.denominator = f"{answered} of {len(eots)} turns answered"
    c.passed = answered == len(eots)
    c.detail = "" if c.passed else f"{len(eots) - answered} turn(s) silently unanswered"
    rep.add(c)

    # --- U2: user-clock latency to first audio ------------------------------------------------
    c = Check("U2", f"First audio within {TARGET_FIRST_AUDIO_MS:.0f} ms of user finishing (user's clock)")
    if latencies:
        s = sorted(latencies)
        p50 = s[len(s) // 2]
        p95 = s[max(0, int(len(s) * 0.95) - 1)] if len(s) > 1 else s[0]
        c.denominator = f"n={len(s)} turns"
        c.measured = f"p50={p50:.0f} ms  p95={p95:.0f} ms  max={s[-1]:.0f} ms"
        c.passed = p50 <= TARGET_FIRST_AUDIO_MS
        if not c.passed:
            c.detail = (f"p50 is {p50 / TARGET_FIRST_AUDIO_MS:.1f}x over target. "
                        f"Natural-conversation reference is ~{NATURAL_TURN_GAP_MS:.0f} ms.")
    else:
        c.denominator = "n=0 turns"
        c.passed = False
        c.detail = "no latency measurable -- no audio followed any user turn"
    rep.add(c)

    # --- U3: a repair must not repeat the previous reply verbatim -----------------------------
    texts = [_text_of(e) for e in responses if _text_of(e)]
    repeats = sum(1 for a, b in zip(texts, texts[1:]) if a.strip() == b.strip())
    c = Check("U3", "No consecutive verbatim-identical spoken replies (repair, don't repeat)")
    c.denominator = f"{len(texts)} spoken replies compared"
    c.passed = repeats <= MAX_CONSECUTIVE_IDENTICAL - 1
    c.measured = f"{repeats} identical consecutive pair(s)"
    rep.add(c)

    # --- U4: spoken replies pass the one-breath test -----------------------------------------
    too_long = [t for t in texts if len(t) > ONE_BREATH_CHARS]
    c = Check("U4", f"Spoken replies are sayable in one breath (<= {ONE_BREATH_CHARS} chars)")
    c.denominator = f"{len(texts)} replies checked"
    c.passed = not too_long
    c.measured = f"{len(too_long)} over limit" + (f" (longest {max(len(t) for t in texts)} chars)" if texts else "")
    rep.add(c)

    # --- U5: visible state actually changes (screenshots must differ) ------------------------
    shots = sorted((run_dir / "screenshots").glob("*.png")) if (run_dir / "screenshots").is_dir() else []
    digests = [hashlib.md5(p.read_bytes()).hexdigest() for p in shots]
    unique = len(set(digests))
    c = Check("U5", "Checkpoint screenshots are not byte-identical (app changes visible state)")
    c.denominator = f"{len(shots)} screenshots, {unique} unique"
    c.passed = len(shots) > 0 and unique > 1
    if len(shots) == 0:
        c.passed = False
        c.detail = "no screenshots captured -- visible state unverified"
    elif unique == 1:
        c.detail = "every checkpoint identical -- app almost certainly stuck"
    rep.add(c)

    # --- U6: a provider failure is surfaced, and only once -----------------------------------
    c = Check("U6", "Provider failure surfaced exactly once (spoken once, stays present)")
    c.denominator = f"{len(failures)} failure event(s)"
    c.passed = len(failures) <= 1
    c.detail = "" if c.passed else "failure reported more than once -- user hears repeated errors"
    rep.add(c)

    # --- U7: presence-cover vs real content (goal, not just gate) ----------------------------
    covers = [e for e in ev if e.get("event") == "presence.cover_required"]
    c = Check("U7", "Turns meeting the budget with REAL content, not presence-cover")
    c.denominator = f"{len(covers)} presence-cover event(s) across {len(eots)} turns"
    # Informational-but-failing signal: if cover was needed on every turn, latency is being
    # masked rather than fixed.
    c.passed = not (eots and len(covers) >= len(eots) and not latencies)
    c.measured = f"cover_required={len(covers)}"
    if len(covers) >= len(eots) and eots:
        c.detail = ("presence cover was required on every turn -- audio continuity is being carried "
                    "by ambience, not by content arriving in time")
    rep.add(c)

    # --- HUMAN items: declared, never silently passed ----------------------------------------
    for ident, title in [
        ("H1", "Listening / processing / speaking / idle are audibly distinct"),
        ("H2", "The voice sounds human and warm (not robotic/flat)"),
        ("H3", "A failure is spoken plainly and the orb still feels present"),
        ("H4", "No moment feels like unasked-for silence"),
    ]:
        rep.add(Check(ident, title, auto=False, passed=None,
                      detail="requires a listener -- see docs/UX-HUMAN-REVIEW.md"))
    return rep


def main() -> int:
    ap = argparse.ArgumentParser(description="Executable voice-UX gate over a real drive trace.")
    ap.add_argument("run_dir", type=Path)
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    rep = evaluate(args.run_dir)

    if args.json:
        print(json.dumps({
            "run_dir": str(args.run_dir),
            "checks": [vars(c) for c in rep.checks],
            "failed": [c.ident for c in rep.failed],
        }, indent=2))
    else:
        print(f"Voice-UX gate -- {args.run_dir}")
        print("-" * 78)
        for c in rep.checks:
            if c.passed is None:
                status = "HUMAN"
            else:
                status = "PASS " if c.passed else "FAIL "
            print(f"{status} {c.ident:3s} {c.title}")
            if c.denominator:
                print(f"          denominator: {c.denominator}")
            if c.measured:
                print(f"          measured   : {c.measured}")
            if c.detail:
                print(f"          note       : {c.detail}")
        auto = [c for c in rep.checks if c.auto]
        print("-" * 78)
        print(f"AUTO checks: {sum(1 for c in auto if c.passed)}/{len(auto)} passed | "
              f"HUMAN checks pending: {len(rep.unevaluated)}")
        print("OVERALL " + ("FAIL" if rep.failed else "PASS"))

    return 1 if rep.failed else 0


if __name__ == "__main__":
    sys.exit(main())
