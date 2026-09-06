# Focus Orb — Study-With-Me Reference Template

Status: design input
Date: 2026-08-08
Reference: [1.5-HOUR STUDY WITH ME — Pomodoro 42-6-42](https://youtu.be/sdhh7AYzsTY)

## 1. What the video is actually doing

The focus effect comes from a simple, repeatable production template:

```text
0:00–0:58   human welcome and setup
0:58–42:57  silent work block (~42 minutes)
42:57–48:50 guided / visible break (~6 minutes)
48:50–end   second silent work block (~42 minutes)
```

The durable ingredients are:

- a human-scale commitment: one bounded work session, not an abstract “be productive” goal;
- continuous focus music;
- a stable co-presence view of someone working;
- a large, always-visible countdown and progress bar;
- no interruptions during the work block;
- a clear break transition;
- a second block that starts automatically after the break;
- occasional camera/layout changes that refresh attention without changing the task.

The video is therefore a **co-presence timebox**, not a conversational assistant loop. Focus Orb should copy the behavioral contract, not the YouTube UI literally.

## 2. Focus Orb version

Add an explicit `study_with_me` session mode. It is opt-in and separate from the default `focus` mode.

| Phase | Default duration | Orb behavior | User control |
|---|---:|---|---|
| `setup` | 30–60 s | Greet, confirm task, choose 42–6–42 or custom block, start audio bed | User can correct task or duration |
| `focus_1` | 42 min | Silent co-presence, timer ring, subtle breathing/presence animation, no unsolicited questions | “Pause”, “I’m stuck”, “Stop”, barge-in |
| `break` | 6 min | Lower-intensity bed, short spoken reset at start/end, timer ring, optional stretch/water/breathe prompt | Skip, extend once, resume |
| `focus_2` | 42 min | Same silent co-presence contract; resume the exact task | Same controls |
| `wrap` | 30–90 s | Ask what was completed, save checkpoint, offer next block or stop | “Done”, “not done”, “continue”, “replan” |

For ADHD safety, the user must be able to say “check in every five minutes” before the session starts. Without that request, the 42-minute block remains quiet; speaking every five minutes would destroy the reference experience.

## 3. Spoken interaction contract

### Setup

The orb should say something close to:

> “I’m here. What are you working on? I’ll stay quiet for 42 minutes, then guide a six-minute break. Say ‘pause’ or ‘I’m stuck’ anytime.”

Then it must:

1. capture or confirm one task;
2. ask at most one blocking clarification;
3. state the chosen timebox;
4. start only after an explicit “start”, “go”, or equivalent;
5. save the session checkpoint before the first focus block.

### During focus

The default output is silence plus presence. The orb may speak only for:

- an explicit user request;
- a safety event;
- a connection recovery phrase;
- a timer boundary that the user explicitly enabled;
- a local failure that requires user action.

If the user says “I’m stuck”, pause the block without losing elapsed time, ask one question, and return to silence after the user says “continue”. If the user says “what next?”, temporarily switch to the existing one-step ADHD task gate.

### Break

At the focus boundary:

1. fade or duck the focus bed;
2. say one short transition phrase;
3. start the six-minute break timer;
4. offer one optional reset action, not a list;
5. announce the final 30 seconds only if the user enabled spoken check-ins;
6. resume the next focus block after explicit or configured auto-resume.

### Wrap

Ask for evidence, not optimism:

> “The session is complete. Did you finish the task, make partial progress, or get blocked?”

Map the answer into `done`, `partial`, or `blocked`. Never infer completion from elapsed time.

## 4. Orb-only visual translation

The reference video uses a person, countdown, music label, and progress bar. Focus Orb should translate those into the orb surface:

```text
focus_1 / focus_2:
  dark background
  orb slowly breathes
  outer ring = remaining time
  inner glow = audio/presence state
  one small color transition at block boundary

break:
  orb becomes softer and slower
  ring changes to break color
  optional gentle pulse for the final minute
```

No text, buttons, or dashboard are required for the core experience. The exact timer remains available through the accessibility/transcript surface, but the spoken channel and orb animation must be sufficient to use the session hands-free.

## 5. Audio design

The audio bed is part of the product contract, not decoration:

- prewarm the bed before `focus_1` starts;
- keep it continuous through normal focus playback;
- duck it during speech and restore it after speech;
- use a local fallback bed if the network or provider fails;
- make the bed opt-in and easy to pause;
- do not use generated speech as a focus filler;
- preserve the existing audio-bed invariant and native playback budgets.

The reference's music changes track identity over time, but Focus Orb should not make track changes a conversational event. Crossfade or rotate locally without speaking.

## 6. State machine and events

```text
IDLE
  → SETUP
  → FOCUS_1_RUNNING
  → BREAK_RUNNING
  → FOCUS_2_RUNNING
  → WRAP
  → IDLE
```

Required events:

```text
study_session.requested
study_session.configured
study_session.started
study_block.started
study_block.paused
study_block.resumed
study_block.interrupted_by_user
study_break.started
study_break.skipped
study_session.wrapped
study_session.checkpointed
```

The deterministic session state owns elapsed time, phase, pause/resume, and completion. The LLM may phrase a transition or answer a user interruption, but it cannot advance the phase or mark the task complete.

## 7. Product modes

Do not collapse these into one ambiguous mode:

| Mode | Best for | Speaking cadence |
|---|---|---|
| `focus_5m` | Default ADHD task execution | One task, five-minute check-in |
| `study_with_me_42_6_42` | Deep work / studying | Setup, break, wrap only |
| `technical_pairing` | Debugging and design discussion | Conversational, interruptible |
| `quiet_presence` | User wants company without coaching | Setup and explicit requests only |

The user can switch modes by voice: “study with me for 42 minutes”, “check in every five minutes”, “talk me through this”, or “stay quiet”.

## 8. Acceptance tests

Hard gates:

- no unsolicited spoken response during a quiet focus block;
- user barge-in pauses or interrupts playback within the existing cancellation budget;
- pause/resume preserves elapsed time and the active phase;
- reconnect does not restart the countdown or duplicate the greeting;
- the six-minute break starts only after the first block is actually complete;
- the second block resumes the same task checkpoint;
- elapsed time never marks the task complete;
- a user request for five-minute check-ins changes cadence only for that session;
- the entire session remains usable through orb, voice, and audio without requiring text;
- focus music continues or falls back locally after provider/network failure.

Experience metrics:

- session start completion rate;
- percentage of focus blocks completed without an unplanned interruption;
- user-requested pause rate;
- break return rate;
- session wrap response rate;
- task completion evidence rate;
- perceived co-presence and focus score after the session;
- audio-bed dropout rate and recovery time;
- false proactive speech rate during focus: target 0.

## 9. Implementation order

1. Add `study_with_me_42_6_42` to the deterministic state machine and checkpoint schema.
2. Reuse `FocusTimeBox`, the orb timer ring, audio-bed ducking, and `BargeIn`; do not create a second timer or playback path.
3. Add setup/break/wrap spoken contracts with one-question clarification limits.
4. Add a 90-minute simulator with pause, reconnect, user-stuck, and early-stop traces.
5. Run a human comparison: existing 5-minute mode versus 42–6–42 mode, measuring focus and interruption preference.

The reference is the template: **short setup → long silent co-presence → guided break → long silent co-presence → evidence-based wrap**. The Orb's differentiator is that the same template remains conversational when the user asks for help.
