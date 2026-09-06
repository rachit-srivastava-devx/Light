"""HTTP client for the gateway-sidecar — the only door to a model from Python (C9).

No provider SDK is importable from here by construction: the sidecar owns `@pe/llm-gateway`
(docs/adr/0004-backend-language-split.md). Callers inject a `GatewayClient`, so the atomizer
pipeline is testable against a fake without a network or a running sidecar.
"""

from __future__ import annotations

import time
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Protocol

import httpx

from ..cost.meter import UsageDelta
from ..observability.devlog import dev_log, truncate


class GatewayError(RuntimeError):
    """Sidecar returned a non-2xx. `status` distinguishes the cases the sidecar maps explicitly:
    400 = missing tenant (caller bug, C8), 402 = budget exhausted (in-path spend guard, C12),
    502 = upstream provider failure. Callers must not retry 400 or 402 blindly.
    """

    def __init__(self, status: int, code: str, message: str) -> None:
        super().__init__(f"gateway {status} {code}: {message}")
        self.status = status
        self.code = code


@dataclass(frozen=True)
class GatewayCompletion:
    text: str
    usage: UsageDelta


class GatewayClient(Protocol):
    """The seam every LLM-touching module depends on. Kept to one method: this product only ever
    needs a blocking completion (§4 — streaming is the Rust relay's concern, on the audio plane).
    """

    async def complete(
        self,
        *,
        tenant_id: str,
        system: str,
        user_text: str,
        max_tokens: int,
        response_mode: str | None = None,
        history: Sequence[dict[str, str]] = (),
    ) -> GatewayCompletion: ...


class HttpGatewayClient:
    """Production implementation. `base_url` points at the sidecar, never at a provider."""

    def __init__(self, base_url: str, client: httpx.AsyncClient) -> None:
        self._base_url = base_url.rstrip("/")
        self._client = client

    async def complete(
        self,
        *,
        tenant_id: str,
        system: str,
        user_text: str,
        max_tokens: int,
        response_mode: str | None = None,
        history: Sequence[dict[str, str]] = (),
    ) -> GatewayCompletion:
        started = time.perf_counter()
        try:
            payload: dict[str, object] = {
                "tenant_id": tenant_id,
                "system": system,
                "messages": [*history, {"role": "user", "content": user_text}],
                "max_tokens": max_tokens,
            }
            if response_mode is not None:
                payload["response_mode"] = response_mode
            response = await self._client.post(
                f"{self._base_url}/v1/complete",
                json=payload,
            )
        except httpx.RequestError as exc:
            # A sidecar that is down/unreachable is an infrastructure condition, not a bug to
            # surface as an unhandled 500 with a stack trace. Found by manual verification against
            # a live server with no sidecar running — the unit tests could not see it, because
            # they inject a gateway and never exercise the transport (LESSONS L2's lesson again,
            # one layer down). 503: the caller may retry this one, unlike 400/402.
            dev_log(
                "gateway_client.unreachable",
                level="error",
                tenant_id=tenant_id,
                base_url=self._base_url,
                error=str(exc),
                latency_ms=round((time.perf_counter() - started) * 1000, 1),
            )
            raise GatewayError(503, "GATEWAY_UNREACHABLE", str(exc)) from exc
        latency_ms = round((time.perf_counter() - started) * 1000, 1)
        if response.status_code >= 400:
            body = response.json() if response.content else {}
            dev_log(
                "gateway_client.error_response",
                level="error",
                tenant_id=tenant_id,
                status=response.status_code,
                code=str(body.get("error", "UNKNOWN")),
                message=str(body.get("message", "")),
                latency_ms=latency_ms,
            )
            raise GatewayError(
                response.status_code,
                str(body.get("error", "UNKNOWN")),
                str(body.get("message", "")),
            )
        payload = response.json()
        # The gateway's CompletionResponse carries `content: ContentBlock[]`; the atomizer only
        # ever asks for text, so anything that isn't a text block is a contract violation upstream
        # rather than something to silently skip.
        blocks = payload.get("content", [])
        usage = payload.get("usage", {})
        text = "".join(b.get("text", "") for b in blocks if b.get("type") == "text")
        dev_log(
            "gateway_client.completion",
            tenant_id=tenant_id,
            system=truncate(system),
            user_text=truncate(user_text),
            response_text=truncate(text),
            usage=usage,
            latency_ms=latency_ms,
            # Provider identity, carried from the sidecar's response body rather than inferred.
            # Without these three, every call in `scripts/provider-report.mjs` lands in one
            # "(model not recorded)" bucket, so the question "should we move to Gemini / stay on
            # Claude" cannot be answered from our own telemetry — the numbers exist but are not
            # attributable. `is_fake_adapter` is the honesty flag: a T0 `memory` reply must never
            # be averaged in with real provider latency and cost.
            model=payload.get("model"),
            adapter=payload.get("adapter"),
            is_fake_adapter=payload.get("is_fake_adapter"),
        )
        return GatewayCompletion(
            text=text,
            usage=UsageDelta(
                llm_tokens_in=int(usage.get("input_tokens", 0))
                + int(usage.get("cache_read_tokens", 0))
                + int(usage.get("cache_creation_tokens", 0)),
                llm_tokens_out=int(usage.get("output_tokens", 0)),
            ),
        )
