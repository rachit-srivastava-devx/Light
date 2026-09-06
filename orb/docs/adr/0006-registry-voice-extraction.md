# ADR 0006 — Registry-first realtime voice extraction

Date: 2026-08-04

## Decision

Promote `voice-realtime` and `realtime-voice` from planned registry entries to T0 candidate modules
before wiring more app behavior. ADHD Focus Orb now consumes the feature through `@pe/realtime-voice`
rather than treating realtime voice as an app-only future extraction candidate.

## Verification ledger

| Lane | Owner | Scope | Verify |
|---|---|---|---|
| registry service | GPT-5 lead | `registry/services/voice-realtime` | verify-equivalent passed: typecheck + 6 tests |
| registry feature | GPT-5 lead | `registry/features/realtime-voice` | verify-equivalent passed: typecheck + 1 test |
| app wiring | GPT-5 lead | `apps/mobile/src/runtime/T0FocusSession.ts`, `apps/mobile/src/runtime/VoiceLoopController.ts`, `apps/mobile/src/App.tsx`, `apps/mobile/src/presence/PresenceBoot.ts` | `npm run verify` passed: boundary lint, typecheck, 248 JS tests, 44 Python tests, 18 Rust tests |
| manual smoke | GPT-5 lead | live T0 relay + sidecar | `npm run smoke:t0` passed: sidecar `/healthz`, relay `/healthz`, `/v1/atomize`, `/v1/session/warmup`, `/v1/cache/prime`, and `/v1/eval/metrics` |
| review | Claude Sonnet | registry extraction review | `VERDICT: GO-AHEAD`; prior blockers on broken docs paths and zero-cost metering were fixed before final verdict |
| review | Codex subagents | extraction and feature contract risks | completed; risks folded into contracts before verify |

## Boundaries

Reusable, domain-agnostic realtime voice contracts live in `registry/`. ADHD-specific tone, prosody,
belief policy, response widgets, and visual/audio presence behavior stay in this product.
