"""The lld.v1 decomposer: a goal -> a ModuleBrief (Speed-of-Thought P0 contract §1.2,
docs/SPEED-OF-THOUGHT-P0-CONTRACT.md).

A sibling of `proxy/atomizer.py`'s `atomize()`, NOT a generic parameterised over it. Both are
   structured decoding -> validate -> repair-once -> fail-closed
and both reuse `_strip_markdown_fence` and the bounded-single-repair shape
(`MAX_REPAIR_ATTEMPTS = 1`). They differ at the one place that matters: `atomize()`'s fail-closed
target is `FALLBACK_STEP`, a plausible-looking guess that costs the user 30 seconds if wrong.
There is NO equivalent for a ModuleBrief here — a guessed module brief costs a whole lane, not 30
seconds — so `decompose()`'s fail-closed branch returns a `clarify_request`, never a fabricated
brief. `atomize()` itself is unchanged; this module does not import from it.
"""

from __future__ import annotations

import json
import time
from dataclasses import dataclass
from enum import Enum
from typing import Literal

from ..cost.meter import UsageDelta
from ..observability.devlog import dev_log, truncate
from .gateway_client import GatewayClient
from .lld_schemas import ModuleBrief, validate_module_brief, validate_module_brief_detailed

# Same cap rationale as atomizer.py: this is filler-covered (contract §4.1's DECOMPOSE state),
# but still a metered model call.
DECOMPOSER_MAX_TOKENS = 768

# §1.2: exactly one bounded re-ask, same discipline as the atomizer.
MAX_REPAIR_ATTEMPTS = 1

# F03 §3.1(2): a brief failing many checks must not push the repair prompt past
# DECOMPOSER_MAX_TOKENS' useful input budget. Named constant, not an inline literal, so the
# tradeoff (more named violations vs. prompt budget) is visible and adjustable in one place.
MAX_REJECTION_DETAIL_CHARS = 400

# The pre-F03 constant message, kept as the fallback for the one case a per-error render cannot
# cover: `validate_module_brief` (pydantic) rejected the payload but
# `validate_module_brief_detailed` (the hand-written mirror) found no error to name -- a genuine
# disagreement between the two validators, not something the repair prompt should render as an
# empty string.
_GENERIC_VALIDATION_FAILURE = "module brief failed lld.v1 schema validation"

SYSTEM_PROMPT = """You turn one build goal into a single ModuleBrief.

Return ONLY a JSON object, no prose, matching exactly this shape (lld.v1 §2):
{
  "node_id": str,            // ^[a-z0-9][a-z0-9-]{2,63}$
  "grain": "module" | "leaf",
  "purpose": str,             // 1..200 chars
  "owner": str,
  "owner_path": str,          // the one repo path prefix this module owns, no ".."
  "interface": [{"name": str, "signature": str}],
  "data_owned": [{"store": str, "owned_by_node": str}],   // owned_by_node MUST equal node_id
  "deps": [str],               // other node_ids only
  "registry": {"kind": "install" | "extract" | "build_new", ...},
  "acceptance": {"given": str, "when": str, "then": str, "oracle_kind": "test" | "property" | "metric", "artifact": str},
  "non_goals": [str],
  "open_questions": [str],     // MUST be [] for the brief to be freeze-eligible
  "guarantees": [{"claim": str, "derivation": {...}, "label": str}],
  "alternatives": [{"option": str, "why_killed": str, "revive_trigger": str}],
  "failure_story": {"trigger": str, "blast_radius": str, "fail_safe": str}
}

Rules:
- Never invent an owner, a store, or a dependency that was not stated or clearly implied.
- Every guarantee must show its derivation: a number needs real arithmetic in `calc`; a structural
  claim must name the file/type/gate that enforces it.
- Never emit content_hash, freeze_id, depth_score, stamped_by, state, or version — those fields do
  not belong on a ModuleBrief; they are stamped later by the gate, never by you."""

# §2's absent-by-design fields, generic and stable across failures: the model never gets to name
# which of these it "meant" to fill in, because on a decode/validation failure there is no trusted
# partial brief to read specific gaps from. Naming the whole required shape is the honest floor.
DEFAULT_MISSING_ON_DECOMPOSE_FAILURE: tuple[str, ...] = (
    "interface",
    "data_owned",
    "acceptance",
    "registry_verdict",
)


class DecomposeKind(str, Enum):
    """Which outcome the pipeline reached. Exactly two — there is no third, "guessed" outcome."""

    BRIEF = "brief"
    CLARIFY_REQUEST = "clarify_request"


@dataclass(frozen=True)
class DecomposeRejection:
    detail: str


@dataclass(frozen=True)
class DecomposeResult:
    kind: Literal["brief", "clarify_request"]
    brief: ModuleBrief | None
    missing: tuple[str, ...]
    usage: UsageDelta
    # Every rejection seen on the way, in order — the decompose-side mirror of atomizer.py's
    # `AtomizeResult.rejections`, so a caller debugging a bad session sees why it fell to clarify.
    rejections: tuple[DecomposeRejection, ...]


def _add_usage(left: UsageDelta, right: UsageDelta) -> UsageDelta:
    return UsageDelta(
        llm_tokens_in=left.llm_tokens_in + right.llm_tokens_in,
        llm_tokens_out=left.llm_tokens_out + right.llm_tokens_out,
        tts_chars_novel=left.tts_chars_novel + right.tts_chars_novel,
        stt_seconds_billed=left.stt_seconds_billed + right.stt_seconds_billed,
    )


def _strip_markdown_fence(text: str) -> str:
    """Byte-for-byte the same defence as atomizer.py's helper of the same name (kept as a separate
    copy rather than a shared import, per the contract's "sibling, not a generic" decision — see
    module docstring); some models wrap JSON in a markdown fence even when told not to.
    """
    stripped = text.strip()
    if not stripped.startswith("```"):
        return text
    lines = stripped.split("\n")
    if lines and lines[0].startswith("```"):
        lines = lines[1:]
    if lines and lines[-1].strip() == "```":
        lines = lines[:-1]
    return "\n".join(lines)


def _rejection_detail_from_errors(parsed: dict[str, object]) -> str:
    """§0.2/§3.1: render the DETAILED validator's full error list, not one constant string, so the
    repair prompt names the actual violation(s) (`"<path>: <message>"`, joined, in the validator's
    own declared order) instead of a blind re-ask. Capped at `MAX_REJECTION_DETAIL_CHARS` so a
    brief failing many checks cannot push the repair prompt past `DECOMPOSER_MAX_TOKENS`' useful
    input budget.
    """
    result = validate_module_brief_detailed(parsed)
    if not result.errors:
        # The two validators disagree (pydantic rejected it, the hand-written mirror found
        # nothing to name) -- fall back rather than hand the model an empty detail string.
        return _GENERIC_VALIDATION_FAILURE
    detail = "; ".join(f"{error.path}: {error.message}" for error in result.errors)
    if len(detail) > MAX_REJECTION_DETAIL_CHARS:
        detail = detail[:MAX_REJECTION_DETAIL_CHARS]
    return detail


def _parse_and_validate(raw_text: str) -> ModuleBrief | DecomposeRejection:
    try:
        parsed = json.loads(_strip_markdown_fence(raw_text))
    except json.JSONDecodeError as exc:
        return DecomposeRejection(detail=f"not JSON: {exc}")
    if not isinstance(parsed, dict):
        return DecomposeRejection(detail=f"expected object, got {type(parsed).__name__}")

    # Success path stays on the pydantic validator (typed object, not re-derived from the
    # hand-written mirror's error list) -- only the FAILURE path's message changes (§3.1(1)).
    brief = validate_module_brief(parsed)
    if brief is None:
        return DecomposeRejection(detail=_rejection_detail_from_errors(parsed))
    return brief


def _repair_prompt(goal: str, rejection: DecomposeRejection) -> str:
    return (
        f"Goal: {goal}\n\n"
        f"Your previous answer was rejected: {rejection.detail}\n"
        "Return corrected JSON only."
    )


async def decompose(
    goal: str,
    *,
    gateway: GatewayClient,
    tenant_id: str,
) -> DecomposeResult:
    """Runs the decompose pipeline. Never raises on model misbehaviour; on exhausting the one
    repair attempt it fails closed to a `clarify_request` (§1.2) rather than any fabricated brief.
    A `GatewayError` (budget rejection, missing tenant) still propagates, same as `atomize()`.
    """
    rejections: list[DecomposeRejection] = []
    prompt = goal
    usage = UsageDelta()

    for attempt in range(MAX_REPAIR_ATTEMPTS + 1):
        started = time.perf_counter()
        completion = await gateway.complete(
            tenant_id=tenant_id,
            system=SYSTEM_PROMPT,
            user_text=prompt,
            max_tokens=DECOMPOSER_MAX_TOKENS,
        )
        latency_ms = (time.perf_counter() - started) * 1000
        dev_log(
            "lld_decomposer.llm_attempt",
            tenant_id=tenant_id,
            attempt=attempt,
            prompt=truncate(prompt),
            raw_response=truncate(completion.text),
            llm_tokens_in=completion.usage.llm_tokens_in,
            llm_tokens_out=completion.usage.llm_tokens_out,
            latency_ms=round(latency_ms, 1),
        )
        usage = _add_usage(usage, completion.usage)
        result = _parse_and_validate(completion.text)
        if isinstance(result, ModuleBrief):
            return DecomposeResult(
                kind=DecomposeKind.BRIEF.value,
                brief=result,
                missing=(),
                usage=usage,
                rejections=tuple(rejections),
            )

        rejections.append(result)
        dev_log(
            "lld_decomposer.rejected",
            level="warn",
            tenant_id=tenant_id,
            attempt=attempt,
            detail=result.detail,
        )
        prompt = _repair_prompt(goal, result)

    dev_log(
        "lld_decomposer.clarify_request",
        level="warn",
        tenant_id=tenant_id,
        rejections=[r.detail for r in rejections],
    )
    return DecomposeResult(
        kind=DecomposeKind.CLARIFY_REQUEST.value,
        brief=None,
        missing=DEFAULT_MISSING_ON_DECOMPOSE_FAILURE,
        usage=usage,
        rejections=tuple(rejections),
    )
