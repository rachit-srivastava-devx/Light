"""The Atomizer pipeline: task -> schema-locked steps (docs/BUILD-DIGEST.md §2/§4).

    structured decoding -> validate -> repair-once -> fail-closed -> use by reference

The boxing rules (§4) are the point of this module. The model fills a schema slot and nothing
else: it never picks a state, a route, or a tone, and its output reaches the session only after
passing both the schema (`schemas.validate_atomizer_output`) and the semantic rubric
(`semantic_checks`). On a second failure the pipeline does NOT retry again and does NOT pass
through degraded output — it returns a deterministic fallback step (§5's degrade ladder rung 4:
"crew LLM/novel atomization -> memory reuse -> templated -> deterministic starter step").
"""

from __future__ import annotations

import json
import time
from dataclasses import dataclass
from enum import Enum

from ..cost.meter import UsageDelta
from ..observability.devlog import dev_log, truncate
from .gateway_client import GatewayClient
from .schemas import (
    AtomizerErrorCode,
    AtomizerOutput,
    AtomizerStep,
    AtomizerValidationError,
    validate_atomizer_output,
)
from .semantic_checks import check_step_atomic

# §3: the atomizer is off the voice hot path (a filler covers it), but it is still a cheap-model
# call — cap the output so a runaway generation can't blow the per-session reservation (INV5).
ATOMIZER_MAX_TOKENS = 512

# §2: exactly one bounded re-ask. Not a retry loop — two attempts, then fail closed.
MAX_REPAIR_ATTEMPTS = 1

SYSTEM_PROMPT = """You break one task into ADHD-atomic steps.

Return ONLY a JSON object, no prose, matching exactly:
{"steps":[{"step_text":str,"est_min":int,"done_signal":str}],"steps_total":int}

Rules:
- step_text: one single physical action, at most 120 characters. Never two actions joined by a sequencing word or a second action verb.
- Never ask the user to decide, choose, or figure anything out. Only to act.
- est_min: whole minutes, 1 to 15. The FIRST step must be 1 or 2 — it must be startable now.
- The FIRST step must name one small target or bounded area. Do not say "everything", "all of", "the entire", or "the whole".
- done_signal: how you would observably know the step is finished. Never repeat step_text.
- steps_total must equal the number of steps, and be between 1 and 12."""

# §5 degrade ladder rung 4 — the deterministic floor. Deliberately generic and deliberately
# trivial to start: when the model has failed twice, the product's job is to get the user moving
# at all, not to be clever.
FALLBACK_STEP = AtomizerStep(
    step_text="Open the thing you need for this and look at it for one minute.",
    est_min=1,
    done_signal="the thing is open in front of you",
)


class AtomizeSource(str, Enum):
    """Which rung of the ladder produced the result. Surfaced on the response envelope's
    `meta.source` (§2) so the eval harness can track fallback-rate (§6 online proxies).
    """

    MODEL = "model"
    MODEL_REPAIRED = "model_repaired"
    FALLBACK = "fallback"


@dataclass(frozen=True)
class AtomizeResult:
    output: AtomizerOutput
    source: AtomizeSource
    usage: UsageDelta
    # Every rejection seen on the way, in order. The eval harness buckets by these (§6), and an
    # operator debugging a bad session needs to know the model was repaired, not just that it
    # eventually succeeded.
    rejections: tuple[AtomizerValidationError, ...]


def _add_usage(left: UsageDelta, right: UsageDelta) -> UsageDelta:
    return UsageDelta(
        llm_tokens_in=left.llm_tokens_in + right.llm_tokens_in,
        llm_tokens_out=left.llm_tokens_out + right.llm_tokens_out,
        tts_chars_novel=left.tts_chars_novel + right.tts_chars_novel,
        stt_seconds_billed=left.stt_seconds_billed + right.stt_seconds_billed,
    )


def _strip_markdown_fence(text: str) -> str:
    """Some models (confirmed live: Gemini 2.5 Flash-Lite) wrap JSON responses in a markdown code
    fence ("```json\\n{...}\\n```") even when the system prompt says "no prose" — this is a model
    default, not something the prompt reliably suppresses. `json.loads` fails immediately on the
    fence, so every such response would otherwise burn both repair attempts and fail closed for a
    reason that has nothing to do with the JSON's actual validity. Strips at most one leading and
    one trailing fence; anything else about the text (including genuinely malformed JSON inside
    the fence) still fails validation normally afterward."""
    stripped = text.strip()
    if not stripped.startswith("```"):
        return text
    lines = stripped.split("\n")
    if lines and lines[0].startswith("```"):
        lines = lines[1:]
    if lines and lines[-1].strip() == "```":
        lines = lines[:-1]
    return "\n".join(lines)


def _parse_and_validate(raw_text: str) -> AtomizerOutput | AtomizerValidationError:
    """Schema layer, then semantic layer. Both must pass before the output may be used."""
    try:
        parsed = json.loads(_strip_markdown_fence(raw_text))
    except json.JSONDecodeError as exc:
        return AtomizerValidationError(
            code=AtomizerErrorCode.SCHEMA_INVALID, detail=f"not JSON: {exc}"
        )
    if not isinstance(parsed, dict):
        return AtomizerValidationError(
            code=AtomizerErrorCode.SCHEMA_INVALID, detail=f"expected object, got {type(parsed).__name__}"
        )

    result = validate_atomizer_output(parsed)
    if isinstance(result, AtomizerValidationError):
        return result

    for index, step in enumerate(result.steps):
        reason = check_step_atomic(step, is_first=index == 0)
        if reason is not None:
            return AtomizerValidationError(
                code=AtomizerErrorCode.STEP_NOT_ATOMIC, detail=f"step {index + 1}: {reason}"
            )
    return result


def _repair_prompt(task: str, error: AtomizerValidationError) -> str:
    """The bounded re-ask. It states the specific violation rather than repeating the whole system
    prompt — a model that just failed a constraint needs the constraint named, not restated rules.
    """
    return (
        f"Task: {task}\n\n"
        f"Your previous answer was rejected: [{error.code.value}] {error.detail}\n"
        "Return corrected JSON only."
    )


async def atomize(
    task: str,
    *,
    gateway: GatewayClient,
    tenant_id: str,
) -> AtomizeResult:
    """Runs the full pipeline. Never raises on model misbehaviour — a bad model output degrades to
    the fallback step (§5). It *does* propagate `GatewayError`, because a 402 budget rejection or a
    missing tenant is a caller/system condition, not something to paper over with a fallback step.
    """
    rejections: list[AtomizerValidationError] = []
    prompt = task
    usage = UsageDelta()

    for attempt in range(MAX_REPAIR_ATTEMPTS + 1):
        started = time.perf_counter()
        completion = await gateway.complete(
            tenant_id=tenant_id,
            system=SYSTEM_PROMPT,
            user_text=prompt,
            max_tokens=ATOMIZER_MAX_TOKENS,
        )
        latency_ms = (time.perf_counter() - started) * 1000
        dev_log(
            "atomizer.llm_attempt",
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
        if isinstance(result, AtomizerOutput):
            source = AtomizeSource.MODEL if attempt == 0 else AtomizeSource.MODEL_REPAIRED
            return AtomizeResult(
                output=result,
                source=source,
                usage=usage,
                rejections=tuple(rejections),
            )

        rejections.append(result)
        # A rejection here IS the "broke silently" case the user cares about: the model returned
        # 200 OK with content that failed schema/semantic validation. Surface it loudly in dev.
        dev_log(
            "atomizer.rejected",
            level="warn",
            tenant_id=tenant_id,
            attempt=attempt,
            code=result.code.value,
            detail=result.detail,
        )
        prompt = _repair_prompt(task, result)

    dev_log(
        "atomizer.fallback",
        level="warn",
        tenant_id=tenant_id,
        rejections=[f"{r.code.value}: {r.detail}" for r in rejections],
    )
    return AtomizeResult(
        output=AtomizerOutput(steps=[FALLBACK_STEP], steps_total=1),
        source=AtomizeSource.FALLBACK,
        usage=usage,
        rejections=tuple(rejections),
    )
