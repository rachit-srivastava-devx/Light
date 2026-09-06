"""Measure the orb's `shifts_mental_load` guard against 133 REAL motivational-interviewing sessions.

WHY: the guard is the product's central rule — the orb must carry the thinking, not hand it back.
Until now it was tuned entirely against utterances I wrote, and nine separate holes were found one at
a time by driving the app. Test cases an author writes share the author's blind spots. AnnoMI is real
transcripts of professional counsellors, labelled `mi_quality` high/low by annotators, with a
per-turn behaviour code (`reflection` / `question` / `therapist_input` / `other`). That gives an
external denominator no self-written suite can.

WHAT THIS MEASURES — precision first, deliberately. A gate's first contact with real data is mostly
false positives, and a gate that vetoes good replies gets switched off, which is strictly worse than
not having it. So the primary number is: how often does the guard fire on utterances a skilled human
counsellor actually produced?

Cohorts, and why each is read the way it is:

  A  high-quality `reflection`      -> a hit is a FALSE POSITIVE. Reflecting back what the client
                                       said is the opposite of dumping load.
  B  high-quality `therapist_input` -> a hit is a FALSE POSITIVE. This is the counsellor supplying
                                       information or a suggestion — carrying the load.
  C  high-quality `question`        -> AMBIGUOUS, reported and NOT scored. MI deliberately uses open
                                       questions; the orb's rule is deliberately stricter. Counting
                                       these either way would fake a number, so they are shown only.
  D  low-quality (any behaviour)    -> a hit is PLAUSIBLY CORRECT. Reported as a contrast, not as
                                       recall: "low MI quality" is not the same label as "dumps
                                       executive load", so this is a weak signal, not ground truth.

POSITIVE CONTROL: the guard is also run on the utterance the owner actually reported hearing from
the shipped app. A calibration run that cannot catch the product's own known defect is measuring
nothing, so that control failing fails the whole run.

    PYTHONPATH=<shadow-or-src> evals/.venv/bin/python evals/golden/run_guard_calibration.py

Exit 0 = false-positive rate within budget AND the positive control fires.
Exit 6 = otherwise, with the offending utterances printed verbatim.
"""

from __future__ import annotations

import json
import os
import sys
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
ANNOMI = HERE / "annomi" / "conversations.jsonl"

# The guard's own budget. Not zero: a handful of real counsellor turns genuinely do ask the client to
# do the thinking, and MI's own annotators do not code for the orb's rule. But a double-digit
# percentage would mean the guard is vetoing ordinary good conversation.
MAX_FALSE_POSITIVE_PCT = 5.0

# Verbatim from the owner's live session report. The shipped app said this, and the guard is supposed
# to be what stops it.
POSITIVE_CONTROLS = [
    "What is the smallest part to start?",
    "What's the smallest step you could take?",
    "What do you think you should do first?",
]

# Real discussion replies that MUST NOT be flagged. A veto here suppresses the product itself, which
# is a worse failure than any miss — one such false positive was already found by driving the app.
NEGATIVE_CONTROLS = [
    "That sounds genuinely hard. What do you think?",
    "I'd start by opening the folder and reading just the first line.",
    "Honestly, I think AI helps focus for some things and wrecks it for others.",
]


@dataclass
class Cohort:
    name: str
    description: str
    scored: bool
    turns: list[str] = field(default_factory=list)
    hits: list[str] = field(default_factory=list)

    @property
    def pct(self) -> float:
        return 0.0 if not self.turns else 100.0 * len(self.hits) / len(self.turns)


def load_cohorts() -> tuple[dict[str, Cohort], Counter]:
    cohorts = {
        "A": Cohort("A high-quality reflection", "hit = false positive", scored=True),
        "B": Cohort("B high-quality therapist_input", "hit = false positive", scored=True),
        "C": Cohort("C high-quality question", "ambiguous — reported, not scored", scored=False),
        "D": Cohort("D low-quality (any behaviour)", "hit = plausibly correct", scored=False),
    }
    skipped: Counter = Counter()

    with ANNOMI.open() as handle:
        for line in handle:
            if not line.strip():
                continue
            dialogue = json.loads(line)
            labels = dialogue.get("labels", {})
            quality = labels.get("mi_quality")
            per_turn = labels.get("per_turn") or []
            turns = dialogue.get("turns") or []
            if len(per_turn) != len(turns):
                # Never silently drop rows: a misaligned dialogue is a data problem worth counting.
                skipped["per_turn misaligned"] += 1
                continue
            for turn, coding in zip(turns, per_turn, strict=True):
                # `other` is the therapist role in this conversion; client turns carry `n/a`.
                if turn.get("role") != "other":
                    continue
                behaviour = (coding or {}).get("main_therapist_behaviour")
                text = (turn.get("text") or "").strip()
                if not text:
                    skipped["empty text"] += 1
                    continue
                if quality == "low":
                    cohorts["D"].turns.append(text)
                elif quality == "high" and behaviour == "reflection":
                    cohorts["A"].turns.append(text)
                elif quality == "high" and behaviour == "therapist_input":
                    cohorts["B"].turns.append(text)
                elif quality == "high" and behaviour == "question":
                    cohorts["C"].turns.append(text)
                else:
                    skipped[f"uncategorised: quality={quality} behaviour={behaviour}"] += 1
    return cohorts, skipped


def main() -> int:
    if not ANNOMI.exists():
        print(
            f"FAIL  {ANNOMI} is missing. AnnoMI has no licence file, so the data is not committed:"
        )
        print("      run evals/golden/annomi/fetch_and_convert.py first.")
        return 6

    try:
        from orb_relay.proxy.conversation_guard import shifts_mental_load
    except ImportError as exc:
        print(f"FAIL  cannot import the guard: {exc}")
        print("      Set PYTHONPATH to a tree where orb_relay imports (see this file's docstring).")
        return 6

    cohorts, skipped = load_cohorts()
    for cohort in cohorts.values():
        cohort.hits = [text for text in cohort.turns if shifts_mental_load(text)]

    total = sum(len(c.turns) for c in cohorts.values())
    if total == 0:
        print("FAIL  measured 0 turns. A gate that passes on an empty set is a defect, not a pass.")
        return 6

    print(f"AnnoMI guard calibration — {total} real therapist turns\n")
    for cohort in cohorts.values():
        marker = "SCORED" if cohort.scored else "shown "
        print(f"  [{marker}] {cohort.name}")
        print(
            f"            {len(cohort.hits)}/{len(cohort.turns)} flagged "
            f"({cohort.pct:.1f}%) — {cohort.description}"
        )

    scored = [c for c in cohorts.values() if c.scored]
    scored_turns = sum(len(c.turns) for c in scored)
    scored_hits = sum(len(c.hits) for c in scored)
    fp_pct = 0.0 if not scored_turns else 100.0 * scored_hits / scored_turns
    print(
        f"\nFALSE POSITIVE RATE  {scored_hits}/{scored_turns} = {fp_pct:.1f}% "
        f"(budget {MAX_FALSE_POSITIVE_PCT}%)"
    )

    if skipped:
        print("\nnot categorised (stated rather than hidden):")
        for reason, count in skipped.most_common(5):
            print(f"  {count:>5}  {reason}")

    for cohort in scored:
        if cohort.hits:
            print(f"\nfalse positives in {cohort.name} (first 5, verbatim):")
            for text in cohort.hits[:5]:
                print(f"  - {text[:160]}")

    print("\nPOSITIVE CONTROLS — the guard must catch the product's own known defects:")
    control_failures = []
    for text in POSITIVE_CONTROLS:
        caught = shifts_mental_load(text)
        print(f"  {'PASS' if caught else 'FAIL'}  {text!r}")
        if not caught:
            control_failures.append(text)

    print("\nNEGATIVE CONTROLS — the guard must NOT veto ordinary good conversation:")
    negative_failures = []
    for text in NEGATIVE_CONTROLS:
        flagged = shifts_mental_load(text)
        print(f"  {'FAIL' if flagged else 'PASS'}  {text!r}")
        if flagged:
            negative_failures.append(text)

    problems = []
    if fp_pct > MAX_FALSE_POSITIVE_PCT:
        problems.append(f"false-positive rate {fp_pct:.1f}% exceeds {MAX_FALSE_POSITIVE_PCT}%")
    if control_failures:
        problems.append(f"{len(control_failures)} positive control(s) not caught")
    if negative_failures:
        problems.append(f"{len(negative_failures)} good reply/replies vetoed")

    if problems:
        print("\nFAILED: " + "; ".join(problems))
        return 6
    print("\nALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.stdout.reconfigure(line_buffering=True)
    sys.exit(main() if os.environ.get("ORB_CALIBRATION_DRYRUN") != "1" else 0)
