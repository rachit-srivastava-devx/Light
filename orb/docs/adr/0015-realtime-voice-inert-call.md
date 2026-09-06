# ADR 0015 — Remove the inert `@pe/realtime-voice` call from the production session

**Status:** recommended; runtime edit deferred to the current file owner. **Date:** 2026-08-28.
**Registry decision:** remove from the production path; retain as an explicit T0 scaffold only.

## Context

The production app constructs `@pe/realtime-voice`, and `T0FocusSession.turn()` awaits
`voice.handle()`. Every caller discards its `frames`, `snapshot`, and `meta`. The default is always
`createMemoryRealtimeVoiceFeature`, so the only output is fake T0 state. The real voice I/O path is
the independently implemented mobile WebSocket protocol plus `backend/relay-rs`.

Consuming this output would make a fake adapter authoritative over metadata while still leaving the
real socket path separate. That would manufacture composition, not remove duplication.

## Decision

Remove the discarded `turn()`/memory-feature invocation from the production `T0FocusSession` path
and state plainly that `relay-rs` owns live voice I/O. Keep the registry feature runnable only in
its own T0 contract tests until a production adapter exists and one caller consumes all authoritative
outputs. Do not delete either working protocol implementation.

`apps/mobile/src/runtime/T0FocusSession.ts` and `apps/mobile/src/App.tsx` were already modified by
other agents when this ADR was written. Per Track L ownership rules, this change does not edit them;
their owner must apply the removal and update focused tests.

## Re-adoption gate

Re-adopt only when the feature uses a production `VoiceRealtimePort`, the mobile caller consumes
`frames`/`snapshot`/`meta`, provider provenance is non-fake, and the relay-rs protocol remains the
single wire authority rather than a third parallel implementation.
