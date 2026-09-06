# L8 Principal Review — Operational Readiness — ADHD Focus Orb

**Reviewer:** Sonnet, acting as L8 lead / review board (A17).
**Date:** 2026-08-05.
**Scope:** `company/products/adhd-focus-orb` working tree, follow-up to
`docs/REVIEW-2026-08-04-L8.md` (composition-layer review). That review asked "is it wired
together"; this one asks "is it production-operable." Verified live where noted — verify gate run,
real curl calls against real keys against real Gemini, `native:preflight` re-run — not doc-trust.

**Overall verdict: CONDITIONAL.** The composition-layer NO-GO from 2026-08-04 is fixed — real
callers, real cost metering on the live path, real keys already work end-to-end for the LLM leg.
This review is about the layer above that: **can this survive contact with a real user session
that has a bad network, a dead provider, a crashed sidecar, or a second tenant.** On that bar, most
items below are NOT DONE. None are scope drift or padding — they are the standard L8 production
checklist, and the pattern from 08-04 has recurred: **the blueprint documents the requirement
correctly; the code was never written against it.**

Work items are grouped by priority. Each has: verdict, evidence, and what "done" looks like.

---

## P0 — breaks a live session today

### 1. LLM failover — NOT DONE
**Blueprint:** `04-MODEL-ROUTER-AND-THE-CREW.md` §6 (lines 136-145) — Gemini Flash-Lite primary →
Haiku fallback on `TTFT p99 > 2× baseline over 60s`, cross-provider by design, tested monthly.
**Code:** `gateway-sidecar/src/index.ts:25-44` picks exactly one fixed adapter at boot from
`ORB_LLM_GATEWAY_ADAPTER`. No runtime switching exists. `atomizer.py:145-185` retries once against
the *same* provider on schema failure only (`MAX_REPAIR_ATTEMPTS = 1`) — a content-repair retry,
not a failover. A real `GatewayError` (provider timeout/5xx) is never caught by `atomize()` at all;
it propagates to `app.py:156-159`, which releases the budget hold and returns 402/503 to the
caller. Reproduced live this session (real Gemini call → genuine `GATEWAY_ERROR`/502 → no retry on
a different provider).
**Done means:** router tries Gemini, on timeout/5xx/rate-limit falls back to Haiku within the same
request, budget hold survives the switch, and this path has a test that kills the primary and
asserts the fallback fired.

### 2. Relay-layer failure doctrine gap — NOT DONE
**Blueprint:** `09-FAILURE-DR-AND-DEGRADATION.md` §2/§4 — an LLM outage must degrade to a
templated/deterministic step at the boundary that saw the failure, never surface a raw error mid-session.
**Code:** `backend/relay-py/src/orb_relay/proxy/atomizer.py:131-133` does not catch `GatewayError`
— it's currently only saved because `apps/mobile/src/runtime/AtomizerPort.ts:79-84` happens to
catch any non-402 relay failure client-side. The relay itself is not the failure boundary the
blueprint specifies. Any future caller of this relay that isn't this one RN client (a web client,
an internal tool) gets an unhandled 502.
**Done means:** `atomize()` catches `GatewayError` and returns the deterministic fallback step
itself, at the relay, with the client-side catch becoming defense-in-depth rather than the only line.

### 3. Crash handling / process supervision — NOT DONE
**Code:** no Dockerfile, docker-compose, systemd unit, pm2 config, or Procfile anywhere in the
repo (confirmed empty `find`). Rust (`relay-rs/src/main.rs`) has no `catch_unwind`/panic hook;
`tokio::spawn` per connection isolates a panic to one session but not a process-level crash (OOM,
bind failure) — nothing restarts it. Mobile `VoiceLoopController.ts` has no dead-backend detection,
no reconnect logic, fire-and-forget calls to `relay?.speak()`/`sendAudio()`/`endOfTurn()`. The
README's "never-silent" promise is not wired into the actual voice loop controller.
**Done means:** each sidecar has a restart policy (systemd `Restart=on-failure` or equivalent,
minimum viable for Phase 1 — doesn't need k8s), the mobile client detects a dead relay via
health-check/timeout and degrades to the local static atomizer + a spoken "reconnecting" cue rather
than going silent.

---

## P1 — silent correctness/cost risk, not yet visible in normal testing

### 4. Logging — NOT DONE
No structured logging anywhere: zero `import logging`/`logger.` in any Python file, bare
`console.log`/`console.warn` in TS sidecars, `println!`/`eprintln!` in Rust. No log-level control.
**Zero correlation/session/request ID threading through any log line** — `session_id` is passed as
a data param but never emitted in a log statement, because no log statements structure it.
**Consequence:** a real bug report ("my session broke at 2pm") is currently undebuggable — there is
no way to reconstruct what a session did across the mobile→relay→gateway→provider hop chain.
**Done means:** structured JSON logs (one library per language, doesn't need to be fancy) with a
`session_id`/`correlation_id` generated once at session start and threaded through every hop,
emitted to stdout at minimum (aggregation can come later, but the field must exist now or every
future log line is unmigrated debt).

### 5. Observability / alerting — NOT DONE (not asked about in the review above, but required)
No metrics emission found anywhere (no counters/histograms for request latency, error rate, cost
spend rate, TTFT). `07-CAPACITY-AND-LATENCY-MATH.md` defines the exact latency budgets this system
is supposed to hit (TTFT, p99 targets) but nothing measures them at runtime — the capacity math is
aspirational, not monitored. `08-COST-MODEL.md`'s per-session/per-user cost anchors are similarly
unmeasured in aggregate (per-session budget tracking exists; there's no running total, no alert if
burn rate deviates from the ₹4/session assumption that caching-absence already invalidates, see
below).
**Done means:** at minimum, emit (a) per-request latency and outcome (success/schema-repair/fallback/error)
as a structured log line that can be grepped/aggregated, and (b) a running spend counter with a
threshold alert — doesn't need Grafana on day one, but the numbers must exist somewhere queryable.

### 6. Prompt caching — NOT DONE, cost-model-invalidating
**Blueprint:** `04-MODEL-ROUTER-AND-THE-CREW.md:108,138-140` mandates a cached
`[system + rubric + pinned few-shots]` prefix — Gemini context-cache at −90% cached-input cost,
Anthropic prompt-cache at 10% — for a stated ~−60% input cost reduction.
**Code:** `gateway_client.py:39-96` sends a plain string `system` field, no `cache_control`
blocks anywhere in the gateway sidecar. Usage accounting defensively parses
`cache_read_tokens`/`cache_creation_tokens` (lines 91-92) but nothing ever populates them — dead
plumbing, not a deferral note.
**Consequence:** `08-COST-MODEL.md:20` prices the LLM line item at ~₹0.2/session *assuming* the
cached prefix. Without caching, the ₹4/session and ₹120/user/month cost anchors quoted in this
product's own `CLAUDE.md` are wrong today, not just future risk. This is a real, current gap, not
a nice-to-have.
**Done means:** `cache_control: {type: "ephemeral"}` on the stable system+rubric block in every
gateway call, and the assembly order the blueprint specifies (`[cached] → [volatile, last]`)
enforced structurally, not just by convention.

### 7. Key validation gaps — PARTIAL, worth closing
Sidecars fail fast at boot if a selected provider has no key (good). But: (a) presence-only, no
format/shape check; (b) a misconfigured deploy that never sets `ORB_LLM_GATEWAY_ADAPTER`/
`ORB_STT_PROVIDER` silently boots into a `fake`/`memory` adapter with **zero error** — this is the
most dangerous kind of misconfiguration because it looks like success; (c) `relay-py` and
`relay-rs` do no key-presence validation of their own, trusting the sidecar entirely.
**Done means:** boot fails loud (not defaults-to-fake) when a required env var that selects a real
provider is simply absent, and there's a `--dry-run`/health endpoint that reports which adapters
are actually live vs fake.

### 8. Error taxonomy inconsistency — PARTIAL
Real typed errors exist independently in TS (`BudgetExhaustedError`, `MissingTenantError`) and
Python (`GatewayError`, `ReservationExceededError`, `AtomizerValidationError` + enum), mapped to
real HTTP status codes. But Rust invented its own untyped `ProviderError::Unavailable(String)` with
no status-code mapping — errors there just become an `eprintln!` and a dropped connection. No
shared error-code contract exists across the three languages; each was designed independently.
**Done means:** one small shared error-code enum (even just a doc-level contract: `{code, http_status,
retryable}` triples) that all three languages map into, so a client can branch on `code` regardless
of which service produced it.

---

## P2 — required before this handles real users at scale, not before first manual test

### 9. Rate limiting / backpressure — MISSING, not evaluated in either prior pass
No rate limiting found on any HTTP endpoint (relay, gateway, voice sidecar). A single misbehaving
client (or a retry storm from the missing-failover gap above) can exhaust the process. This matters
doubly because P0-1 means a stuck provider currently has no backoff — a client retry loop against a
dead primary would hammer it.
**Done means:** basic per-tenant rate limit at the relay ingress, even a naive token bucket.

### 10. Secrets handling / rotation — PARTIAL
`.env` is gitignored (good) and generated by `scripts/setup.sh`. No rotation mechanism, no secrets
manager integration, keys are plaintext env vars read directly by each process. Fine for Phase 1
single-operator use; a real gap the moment there's more than one deploy target or a second engineer.
**Done means:** documented rotation procedure at minimum; a secrets manager (even a simple one) before
any multi-environment deploy.

### 11. PII / data retention — NOT REVIEWED, flag for explicit sign-off
The app processes voice (STT transcripts), session content, and belief-model evidence about a
user's ADHD state — this is sensitive health-adjacent data. No retention policy, no data-deletion
path, no explicit PII handling review found in either the blueprint or the code. `13-COGNITIVE-STATE-AND-POLICY.md`
should be checked for whether it specifies this; if it doesn't, that's a blueprint gap, not just a
code gap.
**Done means:** an explicit decision (documented) on what's persisted, for how long, and how a user
deletes their data — before this touches a real user, not after.

### 12. Crisis detector coverage — NOT DONE (flagged in prior review, repeating here for the work list)
English-only (`CrisisDetector.ts` header) despite Indic-language being a stated product
differentiator. No human safety sign-off documented anywhere. This is a safety-critical path, not a
feature gap — treat as P0 if any real user testing is planned before it's fixed.

### 13. iOS build blocker — NOT DONE, host-level, not code
`npm run native:preflight` fails: `CoreSimulator 1051.54.0` vs required `1051.55.0`,
`spawnSync xcrun ETIMEDOUT`. Requires a macOS/Xcode update + restart on the dev machine. Not a code
fix — tracked here so it doesn't get lost, since it blocks half the "plug in keys and it works" claim.

### 14. Voice (STT/TTS) live-path verification — NOT DONE
LLM/atomizer leg verified live this session against real keys. STT/TTS provider sidecar
(`backend/voice-provider-sidecar`) has in-file confidence notes that field formats are unverified
against a live key (Cartesia key empty in `.env`). The actual voice loop — the product's core
interaction — has never been driven end to end with real audio.
**Done means:** one real recorded voice-in → transcript → atomize → TTS-out round trip, captured as
evidence (audio or a transcript log), not just curl against the text leg.

---

## What I missed asking about in the two prior review passes (this is the actual "what you missed" list)

The two review passes today covered: idea fidelity, AI slop, guardrails, cost metering, manual
testing, key readiness, code quality, prompt caching, context-window scoping, logging, key
validation, failover, error taxonomy, crash handling. Not yet covered, and required at L8 before
calling this production-ready:

- **Rate limiting / backpressure** (#9 above) — never asked, never checked until this pass.
- **Observability/alerting beyond raw logs** (#5) — the review asked about logs; metrics and
  alerting are a distinct requirement and were absent from both prior passes.
- **PII / data retention / deletion** (#11) — health-adjacent data with no retention policy is a
  compliance risk, not just an engineering gap, and neither prior pass raised it.
- **Secrets rotation** (#10) — key *presence* was checked; key *lifecycle* was not.
- **Multi-tenant isolation guarantees** — `tenant_id` is threaded through the cost/budget code, but
  neither pass verified that one tenant cannot read/exhaust another tenant's session state or
  budget reservation. Worth a targeted check before any multi-user pilot.
- **Graceful shutdown** — no `SIGTERM` handling found in any sidecar during this pass; a deploy/restart
  today would drop in-flight sessions mid-turn with no draining.
- **Dependency/supply-chain pinning** — not checked in any pass; worth a `npm audit`/`pip-audit`/
  `cargo audit` pass before this goes further, given it's never been done.
- **Load/chaos testing** — the review verified correctness (does it work) and now resilience (does
  it survive a bad provider); it has never been load-tested (concurrent sessions) or chaos-tested
  (kill a sidecar mid-session and observe).
- **Rollback strategy** — there is no deploy story at all yet (#3 covers process supervision; this
  is the layer above it — how a bad release gets undone). Not urgent pre-launch, but should be a
  conscious decision, not an omission, before the first real deploy.

---

## Priority order for the work queue

1. LLM failover (#1) + relay-layer failure doctrine (#2) — these are the same root cause
   (no cross-provider fallback, no relay-level catch) and should be fixed together.
2. Crash handling / process supervision (#3) — cheap to do minimally (systemd or even a shell
   restart loop), high value.
3. Logging with correlation IDs (#4) — unblocks debugging everything else on this list.
4. Prompt caching (#6) — fixes a cost-model claim that's actively wrong today.
5. Crisis detector coverage + safety sign-off (#12) — safety-critical, do before any real user.
6. Observability/alerting (#5), key validation hardening (#7), error taxonomy (#8) — do together,
   same review pass.
7. Rate limiting (#9), secrets rotation (#10), PII/retention decision (#11) — before multi-user
   pilot, not before solo manual testing.
8. iOS build (#13) and live voice-path verification (#14) — blocking full "plug in keys and go"
   claim; #13 needs a host fix, #14 needs a live test session.
9. Multi-tenant isolation check, graceful shutdown, dependency audit, load/chaos test, rollback
   strategy — before scaling past single-operator manual testing.
