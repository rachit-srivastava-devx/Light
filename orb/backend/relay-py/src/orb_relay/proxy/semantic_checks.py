"""Semantic step checks the JSON schema cannot express (docs/BUILD-DIGEST.md §2 error taxonomy,
§6 atomicity rubric).

Each check is a separate named predicate returning an explicit reason, rather than one large
`validate()` — the atomicity rubric is the product-defining metric (§6) and its four clauses are
graded independently by the eval harness, so they must be independently callable and testable.

These are *heuristics over text*, deliberately: §4 forbids letting a model decide anything
structural, and the eval harness (§6, atomizer-goldens-300) is what measures how good they are.
Their thresholds are judgment calls and marked as such.
"""

from __future__ import annotations

from .schemas import AtomizerStep

# Judgment call (no §-number): a step naming two actions joined by sequencing words or a second
# action verb violates rubric clause 1 (single physical action). The verb list avoids rejecting
# harmless noun conjunctions such as "pick up a pen and notebook".
SEQUENCING_MARKERS = (" and then ", " then ", " after that ", " followed by ", "; then ")
COMPOUND_ACTION_MARKERS = (
    " and find ",
    " and open ",
    " and type ",
    " and submit ",
    " and send ",
    " and click ",
    " and write ",
    " and fill ",
    " and review ",
)

# Rubric clause 3 (§6): "no embedded sub-decision". A step that asks the user to choose is not
# atomic — deciding is the expensive part for an ADHD brain, which is the whole premise (§1).
DECISION_MARKERS = (
    "decide",
    "choose",
    "figure out",
    "work out",
    "determine",
    "pick which",
    "if you want",
    "or maybe",
)

# Rubric clause 2 (§6): initiation cost under ~2 minutes. `est_min` is the model's own estimate;
# a first step estimated above this is a first-step-startable failure (§3: >=99% startable).
FIRST_STEP_MAX_EST_MIN = 2

# A low minute estimate is not enough if the first instruction still describes an unbounded
# cleanup. These markers catch the common Gemini shape "clear everything" and force the repair
# call to name one small object or bounded area instead.
FIRST_STEP_BROAD_SCOPE_MARKERS = ("everything", "all of ", "the entire ", "the whole ")


def violates_single_action(step: AtomizerStep) -> str | None:
    """Rubric clause 1. Returns the offending marker, or None."""
    text = f" {step.step_text.lower()} "
    for marker in SEQUENCING_MARKERS:
        if marker in text:
            return f"sequencing marker {marker.strip()!r} — step names more than one action"
    for marker in COMPOUND_ACTION_MARKERS:
        if marker in text:
            return f"compound-action marker {marker.strip()!r} — step names more than one action"
    return None


def contains_embedded_decision(step: AtomizerStep) -> str | None:
    """Rubric clause 3. Returns the offending marker, or None."""
    text = step.step_text.lower()
    for marker in DECISION_MARKERS:
        if marker in text:
            return f"decision marker {marker!r} — step asks the user to decide, not to act"
    return None


def lacks_observable_done_signal(step: AtomizerStep) -> str | None:
    """Rubric clause 4. The schema already requires `done_signal` to be non-empty; this catches the
    degenerate case where the model echoes the step text back as its own completion signal, which
    carries no information about *how you'd know* you were done.
    """
    if step.done_signal.strip().lower() == step.step_text.strip().lower():
        return "done_signal merely repeats step_text — no observable completion signal"
    return None


def first_step_too_costly_to_start(step: AtomizerStep) -> str | None:
    """Rubric clause 2, applied to the first step only (§3: first-step-startable >= 99%)."""
    if step.est_min > FIRST_STEP_MAX_EST_MIN:
        return (
            f"first step est_min={step.est_min} exceeds the {FIRST_STEP_MAX_EST_MIN}-minute "
            "initiation-cost bar"
        )
    return None


def first_step_too_broad_to_start(step: AtomizerStep) -> str | None:
    """Reject an unbounded first instruction even when the model claims it takes <=2 minutes."""
    text = step.step_text.lower()
    for marker in FIRST_STEP_BROAD_SCOPE_MARKERS:
        if marker in text:
            return (
                f"broad-scope marker {marker.strip()!r} — first step must name one small target "
                "or bounded area"
            )
    return None


def check_step_atomic(step: AtomizerStep, *, is_first: bool) -> str | None:
    """Runs every applicable clause, returning the first violation's reason or None.

    Order is the rubric's order (§6), so the reported reason is stable across runs — the eval
    harness buckets failures by reason, and a reason that changed with dict ordering would make
    those buckets meaningless.
    """
    checks = [violates_single_action, contains_embedded_decision, lacks_observable_done_signal]
    for check in checks:
        reason = check(step)
        if reason is not None:
            return reason
    if is_first:
        return first_step_too_costly_to_start(step) or first_step_too_broad_to_start(step)
    return None
