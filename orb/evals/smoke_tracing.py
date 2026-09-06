"""Prove the Phoenix adoption end to end: trace a REAL model call, then read the span back out.

The standing scar this guards: two orchestration tools were once adopted and defended for a full day
without either having ever executed. So this does not assert "the exporter did not raise" — an
exporter that silently drops spans also does not raise. It makes a real call to the running gateway
sidecar (real provider, real tokens, real money), then queries Phoenix's own read API and asserts the
span came back WITH the model name on it. If Phoenix is not receiving, this fails.

    evals/.venv/bin/python evals/smoke_tracing.py

Exit 0 = a real span with real provider identity is retrievable from Phoenix.
Exit 6 = the chain is broken somewhere, and the message says where.

Requires: `phoenix serve` running, and the gateway sidecar up (`scripts/dev.sh`).
"""

from __future__ import annotations

import json
import os
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from observability import setup_tracing, traced_turn

PHOENIX = os.environ.get("PHOENIX_COLLECTOR_ENDPOINT", "http://127.0.0.1:6006")
SIDECAR = os.environ.get("ORB_GATEWAY_URL", "http://127.0.0.1:8082")
PROJECT = "orb-tracing-smoke"
SESSION = f"smoke-{int(time.time())}"


def _post_json(url: str, body: dict, timeout: float = 30.0) -> dict:
    request = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read())


def _get_json(url: str, timeout: float = 30.0) -> dict:
    with urllib.request.urlopen(url, timeout=timeout) as response:
        return json.loads(response.read())


def main() -> int:
    tracer = setup_tracing(project_name=PROJECT)

    prompt = "Say one short warm sentence."
    try:
        with traced_turn(
            tracer,
            name="gateway.complete",
            session_id=SESSION,
            user_id="smoke-user",
            turn_index=0,
            prompt=prompt,
            mode="converse",
        ) as turn:
            body = _post_json(
                f"{SIDECAR}/v1/complete",
                {
                    "tenant_id": "t0",
                    "system": prompt,
                    "messages": [{"role": "user", "content": "hello"}],
                    "max_tokens": 32,
                },
            )
            usage = body.get("usage", {}) or {}
            cost = body.get("cost", {}) or {}
            turn.model = body.get("model")
            turn.provider = body.get("adapter")
            turn.is_fake_adapter = body.get("is_fake_adapter")
            turn.tokens_in = int(usage.get("input_tokens", 0) or 0)
            turn.tokens_out = int(usage.get("output_tokens", 0) or 0)
            turn.cache_read_tokens = int(usage.get("cache_read_tokens", 0) or 0)
            # The sidecar reports rupees as a float; this is the boundary where money becomes an
            # integer and stays one. Millipaise, not paise: a single small turn costs under a paise,
            # and rounding to paise here reported `cost=0p` on the first real run.
            turn.cost_millipaise = round(float(cost.get("inr", 0.0) or 0.0) * 100 * 1000)
            turn.output_text = "".join(
                block.get("text", "")
                for block in body.get("content", [])
                if block.get("type") == "text"
            )
            turn.metadata["tier"] = str(cost.get("tier", "unknown"))
    except urllib.error.URLError as exc:
        print(f"FAIL  cannot reach the gateway sidecar at {SIDECAR} ({exc}). Start scripts/dev.sh.")
        return 6

    print(
        f"real call: model={turn.model} adapter={turn.provider} "
        f"tokens={turn.tokens_in}/{turn.tokens_out} "
        f"cost={turn.cost_millipaise / 1000:.3f}p fake={turn.is_fake_adapter}"
    )

    if turn.is_fake_adapter is True:
        print(
            "FAIL  the sidecar answered with the T0 FAKE adapter. This smoke check exists to prove"
        )
        print("      a REAL provider call is traced; a fake reply would make the trace a fixture.")
        print("      Set ORB_LLM_GATEWAY_ADAPTER=gemini (or anthropic) and restart the sidecar.")
        return 6

    # Flush before reading back. With batch=False this is already synchronous, but a future change to
    # batching must not turn this check into a race that passes by luck.
    from opentelemetry import trace as otel_trace

    provider = otel_trace.get_tracer_provider()
    if hasattr(provider, "force_flush"):
        provider.force_flush()

    # Read the span back out of Phoenix. Poll briefly: the write is over HTTP and the assertion must
    # not be a coin flip on ingestion timing.
    # Match on THIS run's session id, not "the first span in the project". The first version of this
    # check took spans[0], which on a fresh project is our span and on every later run is a previous
    # run's — so it passed once and then failed with "missing attributes" while the real span was
    # sitting right there. A check that passes by luck is worse than no check.
    url = f"{PHOENIX}/v1/projects/{PROJECT}/spans"
    spans: list[dict] = []
    ours: list[dict] = []
    for _ in range(20):
        try:
            spans = _get_json(url).get("data", []) or []
        except urllib.error.HTTPError as exc:
            if exc.code != 404:  # 404 until the project exists, which is normal on a first run
                print(f"FAIL  Phoenix read API returned {exc.code} for {url}")
                return 6
        ours = [
            span
            for span in spans
            if span.get("name") == "gateway.complete"
            and SESSION in json.dumps(span.get("attributes", {}) or {})
        ]
        if ours:
            break
        time.sleep(0.5)

    print(
        f"phoenix: {len(spans)} span(s) in project {PROJECT!r}, "
        f"{len(ours)} from THIS run (session {SESSION})"
    )
    if not ours:
        if not spans:
            print(f"FAIL  no spans at all in project {PROJECT!r}. The export did not arrive —")
            print(f"      is `phoenix serve` running at {PHOENIX}?")
        else:
            print("FAIL  spans exist but none carries this run's session id — the export from THIS")
            print("      process did not land. Earlier runs' spans are not evidence for this one.")
        return 6

    attributes = ours[0].get("attributes", {}) or {}
    flat = json.dumps(attributes)
    checks = {
        "model name on the span": turn.model is not None and turn.model in flat,
        "token counts on the span": '"llm"' in flat or "token_count" in flat,
        "session id on the span": SESSION in flat,
    }
    for label, ok in checks.items():
        print(f"{'PASS' if ok else 'FAIL'}  {label}")
    if not all(checks.values()):
        print("\nThe span exists but is missing attributes — Phoenix would render an empty trace.")
        return 6

    print(f"\nALL CHECKS PASSED — open {PHOENIX} and look at project {PROJECT!r}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
