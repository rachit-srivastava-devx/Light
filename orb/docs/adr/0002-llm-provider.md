# ADR 0002 — Claude Haiku instead of Gemini 2.5 Flash-Lite for Phase 1

**Status:** superseded in part — 2026-08-05. A real Gemini adapter now exists in
`registry/services/llm-gateway/src/llm-gateway/adapters/gemini/` (per this ADR's own §Consequences
trigger), because the user has a Gemini API key and wants to use it. `ORB_LLM_GATEWAY_ADAPTER=gemini`
+ `GEMINI_API_KEY` in `backend/gateway-sidecar` selects it; Claude Haiku via `anthropic` remains
available and is still the default choice absent an explicit adapter selection. The cost-model
re-derivation this ADR calls for is now less urgent — Gemini 2.5 Flash-Lite's blueprint-sourced price
is the one actually in use once selected, no re-derivation needed for that path. Live-verified only
against a real (invalid) key so far — see the registry package's CHANGELOG for exact scope.

**Installability amendment (2026-08-28):** `backend/gateway-sidecar` declares both provider SDKs
directly at the versions already selected by `@pe/llm-gateway`. The sidecar imports both registry
adapters and is the runtime owner of provider selection, while npm does not install optional
dependencies from the linked `file:` registry package reliably. Keeping the SDKs explicit at this
boundary makes clean-checkout typechecking and real provider selection deterministic without
moving provider calls outside the C9 gateway.

**Original status:** accepted. **Date:** 2026-08-04.

## Context

The blueprint (`docs/06`, `docs/08`) specifies Gemini 2.5 Flash-Lite as the primary cheap model for
the atomizer/router crew, with Claude Haiku 4.5 as failover. `registry/services/llm-gateway` — the
service this repo installs per C1 (never re-implement a capability) and C9 (one model door) — ships
adapters for `memory` (T0) and `anthropic` (T1/T2) only; no Gemini adapter exists in the registry
yet, and building one is out of scope for this product (it belongs in the shared service, C2).

## Decision

Phase 1 uses **Claude Haiku** as the primary cheap model for the atomizer, intent classifier
(ambiguous cases), conversational responder, and check-in phraser — i.e. everywhere the blueprint
says "Gemini 2.5 Flash-Lite." Claude Sonnet remains the mid-tier failover the gateway already
supports.

## Consequences

- Cost figures in `docs/08-COST-MODEL.md` don't transfer directly (Haiku 4.5 is $1/$5 per M tokens
  vs Flash-Lite's $0.10/$0.40 per M) — re-derive the LLM line of the ₹/user/month budget against
  Haiku pricing before treating the ≤₹120/user/month ceiling as verified. The LLM line is ~7% of
  total cost per the blueprint, so this is a real but bounded delta, not a ceiling-breaker — flag if
  re-derivation puts total cost within 10% of ₹120.
- The schema-locking, repair-once, fail-closed pipeline and the boxing rules are provider-agnostic
  and unaffected — this is exactly the substitutability C9 is designed to buy.
- If/when a Gemini adapter is added to `registry/services/llm-gateway` (a second consuming app would
  trigger this per C2), this product should re-evaluate switching back, gated by the cost-model
  re-derivation above.
