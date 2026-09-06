"""OpenTelemetry tracing for orb evals, exported to a self-hosted Arize Phoenix.

WHY THIS EXISTS (and why it is not another hand-written log reader): asked for directly —
*"you should keep record, do observability and other important things to make sure the llms or apis
we are using needs to be changed, cost meter, token meter, llm as a judge etc."* — and then
sharpened: *"do not create anything from scratch use as many tools, libraries, plugins, frameworks
etc... this is a solved problem."*

`scripts/provider-report.mjs` is mine and it is a log reader: it can total tokens but it cannot show
one conversation as a tree, cannot attach a judge verdict to the turn that earned it, and cannot
diff two runs. Phoenix does all three and is one `pip install` with no new infrastructure (Elastic
License 2.0, free self-host). So the rollup script stays as the cheap CI-friendly summary, and this
is the instrument for actually looking at a conversation.

ADOPTED, AND RUN — the standing rule here is that an adoption with no successful invocation is a
claim. Commands that actually worked:

    # server (isolated venv, keeps heavy deps away from relay-py's shared venv)
    python3 -m venv .venv && ./.venv/bin/python -m pip install arize-phoenix   # -> 20.4.0
    ./.venv/bin/phoenix serve                                                  # -> :6006, /healthz OK

    # client, into the evals venv only
    evals/.venv/bin/python -m pip install arize-phoenix-otel openinference-instrumentation

Every API name below was introspected from the installed `phoenix.otel` 0.17.1 and
`openinference.semconv`, not recalled — `register()`'s keyword-only signature and the
`SpanAttributes` member names were printed and read before this file was written.

Attribute choice: OpenInference semantic conventions, because that is what Phoenix's UI reads
natively. OTel's own GenAI semconv is still "Development" status with no tagged release, so aligning
to it exclusively would buy a spec that may still shift and lose the UI that makes this useful.
"""

from __future__ import annotations

import os
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from typing import Any

from openinference.semconv.trace import OpenInferenceSpanKindValues, SpanAttributes
from opentelemetry.trace import Status, StatusCode, Tracer

DEFAULT_ENDPOINT = os.environ.get("PHOENIX_COLLECTOR_ENDPOINT", "http://127.0.0.1:6006")
DEFAULT_PROJECT = os.environ.get("PHOENIX_PROJECT_NAME", "orb-conversations")

# `USER_ID` is present in current openinference-semconv but this file must not assume a member that
# an older pin lacks — a missing attribute name should degrade to the conventional string, never
# raise at import time on a machine with a different pin.
_USER_ID = getattr(SpanAttributes, "USER_ID", "user.id")


def setup_tracing(
    project_name: str = DEFAULT_PROJECT,
    endpoint: str = DEFAULT_ENDPOINT,
    *,
    batch: bool = False,
) -> Tracer:
    """Register a tracer provider pointed at Phoenix and return a tracer.

    `batch=False` by default on purpose: an eval run is short-lived, and a batching exporter that
    has not flushed when the process exits loses the very spans the run existed to produce. That
    failure looks exactly like "the tool does not work", which is how adoptions get abandoned.
    """
    from phoenix.otel import register  # imported lazily so this module is importable without it

    provider = register(
        project_name=project_name,
        endpoint=f"{endpoint.rstrip('/')}/v1/traces",
        protocol="http/protobuf",
        batch=batch,
        # No auto-instrumentation: the orb reaches its model through the gateway sidecar over HTTP
        # (C9), not through a provider SDK in this process, so there is no SDK here to patch. Saying
        # so is better than switching it on and reporting "instrumented" with nothing hooked.
        auto_instrument=False,
        verbose=False,
    )
    return provider.get_tracer(__name__)


@dataclass
class TurnRecorder:
    """Collects what a turn produced, so the caller sets values instead of remembering attribute
    names. Money stays an integer throughout (floats for currency are banned repo-wide); see the
    unit note on `cost_millipaise` for why the unit is smaller than paise here."""

    model: str | None = None
    provider: str | None = None
    output_text: str | None = None
    tokens_in: int = 0
    tokens_out: int = 0
    cache_read_tokens: int = 0
    # Per-call cost in THOUSANDTHS of a paise. The first run of `smoke_tracing.py` reported
    # `cost=0p` on a real 9-in/12-out Gemini call — true, and useless: a single small call costs far
    # under one paise, so rounding to integer paise per call floors every call to zero and a sum of
    # them is zero. Settled money stays integer paise (the repo rule); per-call resolution needs a
    # smaller integer unit. Still an integer — this is not a licence to use a float for money.
    cost_millipaise: int = 0
    is_fake_adapter: bool | None = None
    metadata: dict[str, Any] = field(default_factory=dict)


@contextmanager
def traced_turn(
    tracer: Tracer,
    *,
    name: str,
    session_id: str,
    user_id: str,
    turn_index: int,
    prompt: str,
    mode: str | None = None,
) -> Iterator[TurnRecorder]:
    """One conversation turn as one span, grouped into a Phoenix session.

    `session_id` is what makes a multi-turn conversation legible as a conversation rather than N
    unrelated calls — the same distinction that was the actual bug in the product: history keyed on a
    session id the app re-minted on every mount, so every reload started from zero.
    """
    recorder = TurnRecorder()
    with tracer.start_as_current_span(name) as span:
        span.set_attribute(
            SpanAttributes.OPENINFERENCE_SPAN_KIND, OpenInferenceSpanKindValues.LLM.value
        )
        span.set_attribute(SpanAttributes.SESSION_ID, session_id)
        span.set_attribute(_USER_ID, user_id)
        span.set_attribute(SpanAttributes.INPUT_VALUE, prompt)
        span.set_attribute("orb.turn_index", turn_index)
        if mode is not None:
            span.set_attribute("orb.response_mode", mode)
        try:
            yield recorder
        except Exception as exc:
            # A failed turn must still be visible in the trace. Swallowing it here would reproduce
            # the exact defect class this instrument exists to expose.
            span.set_status(Status(StatusCode.ERROR, str(exc)))
            span.record_exception(exc)
            raise
        else:
            span.set_status(Status(StatusCode.OK))
        finally:
            if recorder.model:
                span.set_attribute(SpanAttributes.LLM_MODEL_NAME, recorder.model)
            if recorder.provider:
                span.set_attribute(SpanAttributes.LLM_PROVIDER, recorder.provider)
            if recorder.output_text is not None:
                span.set_attribute(SpanAttributes.OUTPUT_VALUE, recorder.output_text)
            span.set_attribute(SpanAttributes.LLM_TOKEN_COUNT_PROMPT, recorder.tokens_in)
            span.set_attribute(SpanAttributes.LLM_TOKEN_COUNT_COMPLETION, recorder.tokens_out)
            span.set_attribute(
                SpanAttributes.LLM_TOKEN_COUNT_TOTAL, recorder.tokens_in + recorder.tokens_out
            )
            span.set_attribute(
                SpanAttributes.LLM_TOKEN_COUNT_PROMPT_DETAILS_CACHE_READ,
                recorder.cache_read_tokens,
            )
            span.set_attribute("orb.cost_millipaise", recorder.cost_millipaise)
            # Kept alongside so a dashboard can sum whole paise across a run without re-deriving the
            # unit, while a single cheap turn is still visible above zero in millipaise.
            span.set_attribute("orb.cost_paise", recorder.cost_millipaise // 1000)
            if recorder.is_fake_adapter is not None:
                # The honesty flag, carried into the trace: a T0 fake reply must never be read as
                # production evidence, and in a UI that is only true if the span says so.
                span.set_attribute("orb.is_fake_adapter", recorder.is_fake_adapter)
            for key, value in recorder.metadata.items():
                span.set_attribute(f"orb.{key}", value)


def record_judge_verdict(
    tracer: Tracer,
    *,
    session_id: str,
    criterion: str,
    passed: bool,
    reasoning: str,
) -> None:
    """Attach an LLM-as-judge verdict as its own span in the same session, so a failure can be read
    next to the turn that caused it rather than in a separate JSON file nobody opens."""
    with tracer.start_as_current_span(f"judge:{criterion}") as span:
        span.set_attribute(
            SpanAttributes.OPENINFERENCE_SPAN_KIND, OpenInferenceSpanKindValues.EVALUATOR.value
        )
        span.set_attribute(SpanAttributes.SESSION_ID, session_id)
        span.set_attribute(SpanAttributes.INPUT_VALUE, criterion)
        span.set_attribute(SpanAttributes.OUTPUT_VALUE, reasoning)
        span.set_attribute("orb.judge_passed", passed)
        span.set_status(Status(StatusCode.OK if passed else StatusCode.ERROR, criterion))
