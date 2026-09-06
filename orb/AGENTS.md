# adhd-focus-orb — Product AGENTS.md

> Inherits the **Company-OS constitution** (`../../../Company-OS/AGENTS.md`, C1–C18 /
> L1–L7) and the five doctrines. This file adds product specifics only and may not contradict the
> constitution. The build spec is **`docs/BUILD-DIGEST.md`** (extracted in full from
> `../../../blueprints/ADHD-Focus-Orb-L8-Deep-Dive/`) — read that, not the 13-file
> blueprint, unless you need a specific derivation the digest points you to.

## What this repo is

Phase 1 of "The Focus Orb": a React Native voice body-doubling companion for ADHD. One screen, one
orb, a never-silent on-device noise bed, cloud STT/TTS for a genuinely warm voice, a deterministic
session state machine, and a cheap-LLM crew boxed on every exit. See `docs/BUILD-DIGEST.md` §1 for
the full plane-by-plane module map.

## Blueprint alignment

Copied from `../../../blueprints/ADHD-Focus-Orb-L8-Deep-Dive/AGENTS.md`; Company-OS still has higher
authority for implementation, but these rules are the product-specific L8 bar:

- The audio bed is invariant #1, not a feature. Any gap without a numeric budget and failure story
  is a defect.
- The control path is deterministic; the LLM only fills language. Never let an LLM decide a state
  transition, authority, route, or tone.
- This is a 20K-user rung, not a 50M one. Prefer thin cloud, on-device compute, and no needless
  server scale framing.
- Pure React Native was the original preference. Android deliberately uses a tiny native
  presence/greeting bootstrap to satisfy strict app-open audio budgets; see ADR 0008. No native
  code may make routing, model, cost, or session-state decisions.
- Empathy is deterministic. Session state selects emotional register; shame-adjacent registers are
  excluded by allowed sets.
- Evals are voice-to-voice. Text-only metrics are not proof for audible quality claims.
- Cross-examination and interview-drill docs are the consolidated question banks; extend them rather
  than answering only locally.

**Recorded divergences** — read before writing code: `docs/adr/0001-parallel-build.md` (why this
build parallelizes across git worktrees instead of one-slice-at-a-time),
`docs/adr/0002-llm-provider.md` (Claude Haiku instead of Gemini — the registry's `llm-gateway` has
no Gemini adapter yet), and `docs/adr/0008-native-android-presence-bootstrap.md` (native Android
audio bootstrap for strict app-open budgets).

**Mistakes already made once** — read `docs/adr/LESSONS.md` before starting a new worktree. Every
Opus review verdict that finds a recurring mistake gets added there in the same review; a second
occurrence of a logged mistake is a review failure, not a fresh finding.

## L8 execution standard — mandatory for every agent

L8 means **integrated, evidence-backed behavior**, not a large diff, many interfaces, or a green
unit-test island. Before editing, identify the one slice being built, its entry point, its owning
plane, its registry decision (`install`, `extract`, or `build-new`), and the blueprint acceptance
evidence it must produce. Do not claim a slice complete while its production caller is absent.

### The L8 bar

1. **Presence is the primary product invariant.** The on-device bed is created once, remains audible
   through network/provider failures, and is controlled only by the explicit pause/session-end
   contract. Audio lifecycle, interruption recovery, ducking, and gap budgets must be wired and
   manually verified; a pure classifier or graph constructor is not implementation proof.
2. **The control path is deterministic.** The production journey must connect transcript → intent →
   router → session FSM/step gate → policy/prosody → response envelope. LLMs may fill only
   schema-locked language/evidence slots; they never select state, route, authority, completion,
   session end, or emotional register.
3. **Steps are indexed facts.** Atomizer output is validated, repaired once, fail-closed, and then
   consumed by index. Never speak the raw task, free-text a step at speak-time, or infer `done` from
   silence, timeout, or an ambiguous utterance.
4. **Every boundary carries identity and spend context.** Propagate `tenant_id`, `user_id`, and
   `session_id` through every client, relay, provider, event, and usage contract. Cost reservation
   and admission happen before paid work, are keyed by tenant/session, and are reported with real
   provenance. `cost_paise: 0`, `cache_hit`, or a fake provider is valid only when explicitly marked
   as T0 test behavior and never presented as production evidence.
5. **Registry composition must be real.** A manifest, README, or package dependency does not prove a
   service is installed correctly. Trace the actual runtime call path. Registry features may not
   advertise LLM or cost-plane composition unless they invoke those contracts.
6. **Evals must measure the claimed surface.** Threshold DTOs, caller-supplied metrics, smoke
   endpoints, and unit tests are scaffolding. Audible claims require audio-in → audio-out evidence;
   deterministic claims require replay/consistency evidence; cost claims require cost replay. Use
   the blueprint gates and report the real output, including red results.
7. **Domain belongs in `domain/`; reusable behavior belongs in the registry.** Do not hide ADHD
   policy, prompts, or evalsets in generic services or hardcode product behavior in the mobile
   runtime. Keep the one-way dependency direction intact.
8. **The app must be runnable at the claimed rung.** Phase 1 is a 20K-user, thin-cloud product,
   not a 50M-user platform. Keep the RN reference app buildable, keep dependencies explicit, and
   verify the actual device/audio path before claiming completion.

### L8 numeric anchors

Use the blueprint numbers; do not invent softer substitutes: 0ms unasked-for silence and 0 gap
events; bed audible within 120ms of app open; ducking of −12dB over 80–150ms; barge-in yield ≤100ms;
voice-to-voice p50 ≤1.1s and p99 ≤2.0s; empathy appropriateness ≥95%; hard session reservation
around ₹4; active-user ceiling ≤₹120/month. Capacity claims are for the 20K rung unless the slice
explicitly derives a different target.

### Anti-slop and completion rules

- Do not add placeholder files, no-op routes, stale path references, or future-tense comments and
  describe them as implemented.
- Do not use metadata or test fixtures to simulate cache hits, cost savings, latency, provider
  quality, or voice-to-voice success.
- Every new module must have a real caller or be clearly labelled as an isolated contract/scaffold;
  every test must name the invariant it proves.
- A green local test lane does not override a missing integration path, manual audio verification,
  tenant isolation, cost admission, or an unbuildable app.
- Before handoff, run the product verify gate, run the relevant smoke/eval/manual checks, update the
  registry/manifest/ADR/docs in the same change, and list known gaps. Untracked work is not shipped
  work.

## Layout

- `apps/mobile/src/` — the RN client, one folder per plane: `presence/`, `voice/`, `session/`,
  `router/`, `cognitive/`, `lld/`, plus shared schema mirrors in `shared/`. The critical
  presence-plane boundary is lint-enforced (`tooling/boundary-lint.mjs`): presence never imports
  cognitive or voice (invariant #1 has zero dependency on the crew or the network).
- `backend/relay-py/src/orb_relay/` — the thin stateless orchestration relay: `proxy/` (Atomizer,
  capped phrasers, schemas; no provider SDK), `cost/`, `eval/`
  (fail-closed local contracts plus the future full voice-to-voice gate owner), `observability/`
  (per-hop histograms, gap watchdog).
- `backend/gateway-sidecar/src/` — the ONLY runtime package allowed to import/wrap provider-facing
  registry gateway code. Python/Rust relays talk to this sidecar instead of provider SDKs (C9).
- `backend/relay-rs/src/` — realtime socket protocol/session data plane.
- `domain/` — ADHD-specific policies, agent prompts, evalsets — the ~30% unique to this product
  (C2 placement test: if this app died tomorrow, `domain/` dies with it; nothing here should be
  something a second app would also want).
- `docs/adr/` — divergences and decisions, one file per ADR, numbered.

## The verify gate (C17)

```
npm run verify   # boundary-lint → typecheck (mobile + relay) → vitest
```

Nothing is claimed done until this passes; paste real output, red included. `.claude/hooks/` runs
this automatically per edit (fast, file-scoped) and blocks session end on a red full gate
(`l8-stop-gate.sh`) when code changed this session.

## Hard invariants for any agent working here

1. **INV1 — the bed never stops** except in `IDLE_PRESENT`/`SESSION_DONE` or the explicit pause
   contract (`docs/BUILD-DIGEST.md` §5). A file that lets the bed gap for any other reason is a
   defect, not a nit.
2. **INV2 — steps are spoken by index, never generated at speak-time.** The step gate references
   the atomizer's validated list; nothing downstream free-texts a step.
3. **INV3/INV4 — completion and session-end are explicit-intent only**, never inferred from silence
   or a timeout.
4. **C9 — one model door.** No file outside `backend/gateway-sidecar/src/` imports or wraps
   provider-facing gateway code; product model calls still go through `@pe/llm-gateway`
   (lint-enforced).
5. **Determinism (`docs/BUILD-DIGEST.md` §4)** — state transitions, the router, belief updates, and
   the policy are pure functions. An LLM may only fill a schema-locked slot or contribute bounded
   evidence (`|w·r| ≤ 0.8 logits`); it never picks a state, a route, or a tone.
6. **Money is `numeric`/string, never float.** No wall-clock/random in pure logic (state machine,
   router, policy, belief updates) — inject clocks/timers so replay is bit-identical.
7. **Manual verification is mandatory for anything audible or on-screen** — a green unit-test suite
   is the floor, not the bar. Drive the app (simulator or the eval harness's voice-to-voice corpus)
   before claiming a slice done; capture evidence in `docs/adr/` or the PR description.
8. **No new runtime dependency without an ADR (C3).**

## Registry reuse (C1 — checked before this repo was scaffolded)

- `@pe/llm-gateway` (`registry/services/llm-gateway`, `candidate`) — installed, `file:` dependency
  in `backend/gateway-sidecar/package.json`. See ADR 0002 for the provider substitution.
- `@pe/cost-control-plane` (`registry/services/cost-control-plane`, `candidate`) — installed the
  same way; wraps the ₹/session hard reservation (INV5).
- `@pe/voice-realtime` (`registry/services/voice-realtime`, `candidate`) — installed for realtime
  voice contracts, T0 fake STT/TTS, usage counters, pause/barge-in semantics.
- `@pe/realtime-voice` (`registry/features/realtime-voice`, `candidate`) — installed as the
  foreground T0 voice-session driver that composes the voice service with `llm-gateway` and
  `cost-control-plane`.
