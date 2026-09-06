# Active Presence Quality Failure — 2026-08-07

Status: fixed in working tree; behavioral verification is required before this is called done.

## User-visible failure

The orb greeted once, then behaved like a passive microphone/status light. It did not reliably
show that it was listening, thinking, speaking, or checking in. The user also reported that prior
verification did not improve the felt quality.

## What actually went wrong

1. **The active-presence path was unreachable at launch.** The queue scheduled a `stall_assist`
   event, but it called `T0FocusSession.checkIn()`. That method returns `null` outside `WORKING`.
   The app starts in `INTAKE`, so the first ten seconds produced no speech.
2. **The policy was applied at the wrong layer.** `Policy.decide()` intentionally returns
   `Observe` for `INTAKE` and `CLARIFY`. That is correct for cognitive intervention, but it was
   incorrectly treated as permission for the product to have no conversational presence.
3. **The user-turn guard was stuck on.** `VoiceLoopController.start()` called `relay.startListening()`;
   `App.tsx` interpreted that as `userTurn=true` before any speech frame arrived. The queue's
   `not_user_turn` requirement then rejected every proactive event.
4. **The speaking state was not connected to the orb.** The screen model derived voice state only
   from the last response envelope. A normal conversational response leaves the session in
   `INTAKE`, so the orb continued to look like it was listening while Fish was speaking.
5. **Backchannel support was dead code.** `handleMidUtterancePause()` could return a backchannel
   event, but the native app had no caller that converted that event into playback.
6. **The previous evaluation measured transport, not experience.** Fish returned bytes, relay-rs
   emitted frames, and unit tests passed. None of those checks proved that the user got a timely
   second presence line, that the user-turn guard opened, or that the orb visibly transitioned
   through live speech states. This was a verification-design failure, not evidence of product
   quality.

## Fix applied

- Added a separate bounded `gentle_presence` cadence: first launch/intake presence after 5 seconds,
  then at most one every 45 seconds while the app is active and the user is not speaking.
- Added deterministic, varied presence lines for intake and working states.
- Kept cognitive check-ins separate; they still require the policy to authorize an intervention.
- Set `userTurn` only after an actual audio frame is sent, and clear it at end-of-turn.
- Added live speech status to the screen model so the orb changes while TTS is starting/speaking or
  fails, even when the session envelope remains `INTAKE`.

## Regression rule

An end-to-end check is not green unless it proves this sequence with timestamps:

`launch → greeting → relay speech start → relay speech complete → active presence scheduled →
active presence spoken → first mic frame → user-turn cancellation → transcript → response speech`

Transport byte counts and isolated tests remain necessary, but they are not sufficient for a human-
quality claim.
