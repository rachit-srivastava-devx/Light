# ADR 0008 — Android native presence and first greeting bootstrap

**Status:** accepted. **Date:** 2026-08-04.

## Context

The blueprint prefers pure React Native scheduling through `react-native-audio-api`. Android E2E
proved the React-mounted graph works, but strict timing evidence stayed red: JS presence was ready
hundreds of milliseconds after `MainActivity`, and the first spoken step waited on React startup.

The product invariant is stronger than the implementation preference: the user gets a bed quickly,
and the app speaks instead of rendering text.

## Decision

Android starts a tiny native brown-noise `AudioTrack` in `MainApplication.onCreate`, then records
`OrbPresence: activity_ready` in `MainActivity.onCreate`. Android also prewarms native
`TextToSpeech` in the application and requests the first hardcoded T0 greeting before React
Activity startup. React still renders the orb-only surface and owns the session loop; the JS
`react-native-audio-api` graph remains present for later control-plane ducking, color morphing,
and tests.

## Evidence

Strict Android E2E now passes with:

```txt
presence_ready=pass observed_ms=0 budget_ms=120
speech_request=pass observed_ms=0 budget_ms=250
```

The same logcat evidence includes `OrbGreeting: queued`, `OrbSpeech: queued`, and
`focus-orb:speak Open the first small action`. Evidence files are written under
`docs/evidence/android-e2e/`.

## Consequences

- This is a deliberate Android divergence from the original "pure React Native only" preference.
- iOS still needs either the equivalent native bootstrap or a proved React-native path once the
  host CoreSimulator blocker is fixed.
- The native bootstrap must stay small: no provider calls, no routing decisions, and no visible UI.
