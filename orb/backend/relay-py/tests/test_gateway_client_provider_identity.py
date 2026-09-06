"""Provider identity must survive the Python hop, or we cannot answer "should we switch provider".

`scripts/provider-report.mjs` buckets every model call by `row.model`. Before this, the Python
client logged `usage` and `latency_ms` but dropped `model`, `adapter`, and `is_fake_adapter` from
the sidecar's response body — so the report printed:

    (model not recorded)
      calls 1191 · p50 61.6ms · p95 1374ms

1,191 calls in ONE bucket. The dev-logs proved the calls were not homogeneous at all — a census of
the sidecar's own rows found 298 real `gemini-2.5-flash-lite`, 55 real `claude-haiku-4-5`, and 961
`fake-llm-memory`. A p50 of 61.6ms is the fake adapter's p50; averaging a T0 stub in with a real
provider makes the number worse than absent, because it looks authoritative.

The data was never missing. It was in the response body and the reader threw it away
(`gateway-sidecar/src/index.ts` returns `{...normalized, adapter, is_fake_adapter}`, and
`normalizeCompletionResponse` returns `{...result, content, speech}` — so `model` is preserved).

These tests pin the three fields onto the log row so the provider question stays answerable from
our own telemetry.
"""

from __future__ import annotations

import httpx
import pytest
from orb_relay.proxy import gateway_client as gc


def _client(body: dict[str, object]) -> gc.HttpGatewayClient:
    """A sidecar that answers once with `body`. No network, no running sidecar."""

    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, json=body)

    return gc.HttpGatewayClient(
        "http://sidecar.invalid",
        httpx.AsyncClient(transport=httpx.MockTransport(handler)),
    )


@pytest.fixture()
def rows(monkeypatch: pytest.MonkeyPatch) -> list[tuple[str, dict[str, object]]]:
    captured: list[tuple[str, dict[str, object]]] = []
    monkeypatch.setattr(gc, "dev_log", lambda event, **fields: captured.append((event, fields)))
    return captured


def _completion_row(rows: list[tuple[str, dict[str, object]]]) -> dict[str, object]:
    matching = [fields for event, fields in rows if event == "gateway_client.completion"]
    assert len(matching) == 1, f"expected exactly one completion row, got {len(matching)}"
    return matching[0]


REAL_GEMINI_BODY: dict[str, object] = {
    # Shaped like a real sidecar response: this is the body the Gemini adapter produced for the
    # 298 calls already in dev-logs/, not an invented shape.
    "content": [{"type": "text", "text": "One small thing: open the folder."}],
    "usage": {"input_tokens": 900, "output_tokens": 26, "cache_read_tokens": 160},
    "model": "gemini-2.5-flash-lite",
    "adapter": "gemini",
    "is_fake_adapter": False,
}


@pytest.mark.asyncio
async def test_real_provider_identity_reaches_the_log_row(rows: list) -> None:
    await _client(REAL_GEMINI_BODY).complete(
        tenant_id="t0", system="s", user_text="u", max_tokens=64
    )
    row = _completion_row(rows)
    assert row["model"] == "gemini-2.5-flash-lite"
    assert row["adapter"] == "gemini"
    assert row["is_fake_adapter"] is False


@pytest.mark.asyncio
async def test_a_t0_fake_reply_is_labelled_as_fake(rows: list) -> None:
    """C-anti-slop: T0 fake metadata must never be presented as production evidence. If this field
    goes missing, fake latency silently improves the real provider's p50."""
    await _client(
        {
            "content": [{"type": "text", "text": "[fake] hello"}],
            "usage": {"input_tokens": 10, "output_tokens": 3},
            "model": "fake-llm-memory",
            "adapter": "memory",
            "is_fake_adapter": True,
        }
    ).complete(tenant_id="t0", system="s", user_text="u", max_tokens=64)
    row = _completion_row(rows)
    assert row["model"] == "fake-llm-memory"
    assert row["is_fake_adapter"] is True


@pytest.mark.asyncio
async def test_a_body_without_identity_logs_none_rather_than_crashing(rows: list) -> None:
    """The missing-field case, walked deliberately: an older sidecar, or a future adapter that
    omits `model`, must degrade to an unattributed row — never to a 500 on the voice path. The
    report's own `?? '(model not recorded)'` fallback then does the right thing."""
    await _client(
        {"content": [{"type": "text", "text": "hi"}], "usage": {"output_tokens": 1}}
    ).complete(tenant_id="t0", system="s", user_text="u", max_tokens=64)
    row = _completion_row(rows)
    assert row["model"] is None
    assert row["adapter"] is None
    assert row["is_fake_adapter"] is None


@pytest.mark.asyncio
async def test_usage_accounting_is_unchanged_by_the_new_fields(rows: list) -> None:
    """Guard the thing that was already correct: cache-read and cache-creation tokens count as
    input for cost purposes. This is the regression a careless edit to that dev_log call would
    cause, and money is the one place this repo has no tolerance for drift."""
    completion = await _client(REAL_GEMINI_BODY).complete(
        tenant_id="t0", system="s", user_text="u", max_tokens=64
    )
    assert completion.usage.llm_tokens_in == 900 + 160
    assert completion.usage.llm_tokens_out == 26
