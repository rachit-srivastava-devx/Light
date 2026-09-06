# ADR 0004 — Backend rewritten as Python (orchestration) + Rust (realtime relay)

**Status:** accepted. **Date:** 2026-08-04. **Supersedes:** the TypeScript `backend/relay`
scaffold from ADR 0001/the initial scaffold commit (package.json + tsconfig only — no
implementation had been built into it yet, so nothing is lost by the switch).

## Context

The human directed a backend language change mid-build: "backend should be in python and rust."
The mobile client (`apps/mobile`, React Native/TypeScript — presence + session control planes,
already built and merged) is unaffected; this ADR is scoped to `backend/*` only.

## Decision

Split the backend by workload, not a blanket rewrite in one language:

- **`backend/relay-py`** (Python, FastAPI) — the product's orchestration surface: the Atomizer
  pipeline (schema validation, repair-once, fail-closed), `ConversationalResponder`,
  `CheckInPhraser`, `CostMeter`, `EvalHarness` (the eleven CI gates), `Observability`, and the
  `/v1/session/warmup` + `/v1/cache/prime` endpoints. This is business logic and I/O-bound
  orchestration — Python's ecosystem (pydantic for schema-locking, pytest for the eval harness,
  the ML/data tooling the eval golden-sets will eventually need) fits this surface well.
- **`backend/relay-rs`** (Rust, tokio + tokio-tungstenite) — the realtime audio data plane:
  `SttProxy`/`TtsProxy`'s WebSocket piping between the mobile client and the STT/TTS providers.
  This is BUILD-DIGEST.md §3's hot path (turn p50 ≤250ms, conversational p50 ≤600ms/p99 ≤1.2s) —
  exactly the latency-critical, allocation-sensitive workload Rust's async runtime is built for,
  and where a GC pause or an interpreter's overhead is a real budget risk.
- **`backend/gateway-sidecar`** (TypeScript, unchanged language) — a thin HTTP wrapper around the
  registry services this product already installs per C1 (`@pe/llm-gateway`,
  `@pe/cost-control-plane`, both TS-only). Rather than re-implementing routing/prompt-cache/budget
  logic in Python (real work, and a maintenance fork the registry's "one door" principle, C9,
  exists specifically to prevent), the sidecar runs the existing registry packages in-process and
  exposes them over a small internal REST API. `relay-py` calls the sidecar instead of a provider
  SDK — C9 is still satisfied (no product code anywhere imports `@anthropic-ai/sdk` directly), it
  is just satisfied across a process boundary instead of an import boundary. This is a legitimate
  cross-language registry-integration pattern, not a workaround; the alternative (forking the
  gateway's routing logic into Python) would drift from the registry's contract-suite the moment
  either copy changed.

## What was removed

`backend/relay/{package.json,tsconfig.json,src/proxy/contracts.ts}` — scaffolding only, no
implementation. The wire-format contracts it defined move to `backend/relay-py`'s pydantic models
(`AtomizerOutput`, `SttProxyRequest/Response`, `TtsProxyRequest/Response`) as the new canonical
source; `apps/mobile/src/session/StepGate.ts`'s existing local TS copy (already documented as
deliberately duplicated, `docs/adr/LESSONS.md` L5) is now the *second* of the schema's mirrors
rather than the first — the extraction trigger for a shared, language-agnostic schema (e.g. JSON
Schema generated from the pydantic models, checked into both languages) still fires at Tier 5
(`ContextPack.ts`/`ClarifyProtocol.ts`), per L5's existing rule.

## Consequences

- `npm run verify` at the repo root no longer typechecks a `backend/relay` TS project; it now also
  shells out to `pytest` (relay-py) and `cargo test` (relay-rs) when those toolchains are present,
  plus the unchanged mobile lint/typecheck/vitest. See the updated root `package.json`.
- `tooling/boundary-lint.mjs`'s provider-SDK check now applies to `backend/gateway-sidecar/src`
  instead of the deleted monolithic relay proxy — that is the only remaining place in the TS surface where a
  provider SDK import is even possible (indirectly, via the registry packages it wraps).
- Three runtimes now need Company-OS's C7 (memory adapter, runs at T0 on ₹0 infra) and C12 (cost
  metered) restated per-language: `relay-py`'s `CostMeter` wraps `@pe/cost-control-plane` via the
  sidecar, same as `relay-rs`'s audio-byte metering will need to (Rust does not call the sidecar on
  the hot path — it reports usage counters back to `relay-py` out-of-band, since a synchronous HTTP
  round-trip on the audio data plane would reintroduce the latency risk this split exists to avoid).
- CI/local dev now needs Python 3.14+ and a stable Rust toolchain in addition to Node — recorded
  here rather than silently assumed; both were confirmed present in this environment before the
  scaffold was written (`python3 --version`, `cargo --version`).
