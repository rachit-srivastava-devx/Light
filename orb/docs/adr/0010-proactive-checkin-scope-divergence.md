# ADR 0010 — Proactive check-in: a human `/goal` overrides the blueprint's Phase-1 deferral

## Status

Accepted, divergent-from-blueprint (per `CLAUDE.md`: "A human `/goal` wins for its task — note the
divergence in an ADR").

## Context

`governance/blueprints/ADHD-Focus-Orb-L8-Deep-Dive/README.md` §9 ("Known gaps") explicitly defers
proactive engagement:

> "Proactive pings / notifications. Phase 1 is foreground, session-scoped — the orb never
> re-engages you when the app is closed... Scheduling, calendars, time-of-day awareness. No
> schedule model, no 'ping me at 3pm.' Deferred."

Consistent with that deferral, the codebase's `CHECK_IN` state — while fully specified in the FSM
(`WORKING --policy_intervention[policy_admits]--> CHECK_IN`) and in the belief/policy engine
(`cognitive/Policy.ts`) — was never actually wall-clock-driven. `T0FocusSession.ts`'s
`defaultPolicyInput()` hardcoded both `session_elapsed_ms` and `execution_stalled_ms` to `0`,
which permanently disables the two guards that key off real elapsed time
(`guard.execution_stalled` at 3 minutes, `guard.hyperfocus_boundary` at 45 minutes). `applyPolicy()`
was only ever invoked from inside `speakCurrentStep()` — i.e., only as a side effect of the app's
own speech — so `CHECK_IN` was turn-driven, not proactive, despite the design docs' language.

The user's session `/goal` (2026-08-06) explicitly rejects this scope: the product is described as
a "body double" companion that must "engage with the user" using "schedulers and events" without
being "explicitly asked to work on something" — i.e., real proactive re-engagement is the point of
the product for this task, not an optional Phase-2 nice-to-have.

## Decision

For the **foreground, session-scoped** case only (the app already in the running state, same
process — no background execution, no push notifications, no OS wake), wire `CHECK_IN` to real
elapsed time:

- `T0FocusSessionRuntime` gained `checkIn(now: EpochMs): Promise<ResponseEnvelope | null>` —
  called on a real interval from `App.tsx` (the one impure boundary in the app that already owns
  `Date.now()` for the presence bed). It builds a real `PolicyInput` (not the `0`-stubbed one) and
  runs the *existing*, already-reviewed `decide()` policy — no new intervention logic, no new
  thresholds, just real time flowing into contracts that were already designed for it.
- `T0FocusSession.ts` itself stays fully deterministic: it takes `now` as a parameter and never
  calls `Date.now()`/`Math.random()` internally, per the repo's determinism invariant.

This does **not** implement the blueprint's still-deferred items: no background/closed-app
engagement, no push notifications, no calendar/time-of-day awareness, no persistence across app
restarts. Those remain Phase-2, matching the blueprint's own scoping — only the
already-foreground-scoped mechanism gets turned on.

## Consequences

- `CHECK_IN` can now genuinely fire from silence during a live session — the product behavior the
  user asked for — using policy logic that was already fully specified and tested, just never
  driven by anything real.
- The blueprint's Phase-1/Phase-2 boundary around "scheduling" is *narrowed*, not violated: this
  ADR only activates a mechanism the blueprint's own FSM and policy engine already fully specify
  for the foreground case; it does not add the background/notification surface the blueprint
  explicitly deferred.
- Follow-up (still open, still Phase-2 per blueprint): background re-engagement via local
  notifications when the app isn't foregrounded. Not attempted here — out of scope for what a
  foreground `AppState`-bound timer can do.
