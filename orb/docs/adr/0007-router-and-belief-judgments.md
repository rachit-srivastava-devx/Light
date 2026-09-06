# ADR 0007 — Router and belief-model judgment calls

**Status:** accepted. **Date:** 2026-08-04.

## Context

The L8 review flagged two defensible implementation choices that needed explicit ADR coverage:

1. `Router.ts` evaluates `state === INTAKE` before intent rows, even though the digest lists the
   intent rows first.
2. `BeliefModel.ts` uses blueprint-provided half-lives for Attention and Energy, then derives the
   remaining seven belief half-lives by local judgment.

## Decision

`Router.ts` keeps `INTAKE` first. `IntentLabel` is a closed six-member union; reading the digest's
router prose as a literal top-to-bottom chain would make the INTAKE row unreachable whenever intake
speech contains words such as "pause", "next", or "what". During intake, the safe deterministic
route is always atomization/clarification, not session control. This preserves the product contract
that a raw brain-dump becomes a validated step list before the app accepts working-session intents.

`BeliefModel.ts` keeps the current decay table:

- Attention: blueprint anchor, 90 seconds.
- Energy: blueprint anchor, 15 minutes.
- Execution and WorkingMemory: Attention-speed because they shift inside a step.
- EmotionalLoad and NoveltyPull: two Attention half-lives because they move slower than attention
  but still within a session.
- Goal and Context: Energy-speed because they should survive short interruptions.
- Trust: four Energy half-lives because it is a slow relationship prior, not a momentary state.

Every belief uses `reversion_half_life_ms = confidence_half_life_ms * 4`. Confidence should fade
before the value reverts, so stale evidence becomes uncertain before it becomes neutral.

## Consequences

- Router behavior is intentionally not a mechanical transcription of the digest ordering; the code
  comment, tests, and this ADR are the authority for the reachability fix.
- The belief half-lives are accepted T0 calibration values, not measured production constants.
  Future voice-to-voice and belief-replay corpora may tune them, but must preserve deterministic
  replay and document any changed value here or in a successor ADR.
