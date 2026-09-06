# Orb Body-Double Conversational OS

Status: design proposal  
Audience: mobile, cognitive, voice, and gateway implementations  
Goal: make the orb feel like a reliable person working beside the user, without becoming noisy, needy, or falsely human.

## Product stance

The orb is not a task chatbot that waits for commands. It is a lightweight body double:

- It notices the current moment.
- It remembers the one thing the user is doing.
- It speaks first when presence is useful.
- It stays quiet when the user is in flow.
- It offers one small next move when the user is stuck.
- It celebrates real progress, not compliance.
- It repairs misunderstandings immediately and honestly.

The system must not claim to be a human, infer medical dopamine levels, or use stimulation as a medical treatment. “Energy” and “novelty” below mean bounded voice, timing, wording, and orb presentation controls.

## 1. State model: orthogonal dimensions

A single state machine is not expressive enough. A user can be focused, blocked, tired, and trusting the orb at the same time. Keep dimensions independent and let policy combine them.

### Goal state

`none` → `forming` → `committed` → `maintaining` → `completed`  
Any active goal can enter `decaying` when there is prolonged silence, abandonment language, or a new competing goal.

### Attention state

- `available`: ready for a turn
- `listening`: audio is arriving and the user owns the turn
- `focused`: sustained work signals; default is silence
- `exploring`: user is thinking, searching, or changing direction
- `wandering`: attention is drifting or inputs are unrelated
- `hyperfocus`: high momentum; interruptions require strong evidence
- `interrupted`: an external event or barge-in occurred
- `recovering`: returning to the last known action

### Barrier state

- `none`
- `uncertain`: goal or next action is unclear
- `blocked`: user reports a dependency or cannot proceed
- `overwhelmed`: too much scope, emotion, or working-memory load
- `avoiding`: repeated deflection without progress
- `waiting`: user is waiting on an external system
- `fatigued`: low energy or repeated misses
- `dysregulated`: emotional load is high; reduce demands

### Execution state

- `not_started`
- `started`
- `moving`
- `stalled`
- `waiting_on_user`
- `waiting_on_system`
- `done`

### Relationship/presence state

- `new`: establish identity and consent
- `present`: quietly available
- `co_working`: both have a known next action
- `supporting`: user needs a small assist
- `repairing`: STT, TTS, or intent failed
- `celebrating`: genuine progress just happened
- `cooling_down`: reduce novelty after an energetic burst

### Conversation ownership

- `orb_turn`: orb may speak
- `user_turn`: orb listens and does not compete
- `shared_silence`: no speech; the orb remains visually present
- `deferred`: a useful message is scheduled but waiting for a safe opening

## 2. Belief model

Store beliefs as bounded values with confidence and timestamps, not as permanent labels.

```text
goal                 = "finish the first paragraph"
goal_confidence      = 0.91
attention.focused    = 0.82
attention.wandering  = 0.12
barrier.uncertain    = 0.21
barrier.overwhelmed  = 0.08
execution.progress   = 0.35
execution.stall_ms   = 42_000
working_memory       = 0.38        # lower means more support is needed
energy               = 0.64
novelty_pull         = 0.76
trust                = 0.71
stt_confidence       = 0.88
last_user_turn_ms    = 2_000
last_orb_turn_ms     = 18_000
```

Every belief update must record:

```text
observation, value, confidence, observed_at, source, expiry
```

Use short half-lives for inferred attention and energy. Keep the user’s explicit statements longer. Never let a weak signal override an explicit “stop”, “be quiet”, “I’m fine”, or “I’m overwhelmed”.

## 3. Observation inputs

### High-confidence inputs

- Explicit speech: “I’m stuck”, “don’t talk”, “stay with me”.
- Successful STT with transcript confidence.
- User barge-in, pause, stop, or mute.
- Task completion or done signal.
- Relay/provider failure.

### Medium-confidence inputs

- Time since last meaningful user turn.
- Repeated failed attempts at the same step.
- Repeated task changes.
- User accepts, rejects, or ignores an orb suggestion.
- Audio level and VAD turn duration.

### Future optional inputs

- Screen activity, keyboard/mouse activity, or calendar context only with explicit permission.
- No screen content should be captured by default.

The orb must degrade gracefully to speech-only signals. Silence is not proof of disengagement.

## 4. Policy: decide presence before content

The policy first chooses whether to speak, then chooses what to say, then chooses how to say it.

```text
if user_requested_silence:
    shared_silence
else if provider_or_stt_failure:
    repairing
else if attention.hyperfocus and no urgent event:
    shared_silence
else if user_turn:
    listen
else if explicit_blocked or barrier.blocked > 0.70:
    ask_one_small_question
else if working_memory < 0.35:
    reduce_to_one_action
else if execution.stall_ms > adaptive_stall_threshold:
    offer_choice_or_demonstrate
else if progress_event:
    celebrate briefly
else if safe_scheduled_event_is_due:
    speak_deferred_message
else:
    shared_silence
```

The default action is not “generate a response”. The default action is quiet co-presence.

### Intervention vocabulary

- `observe`: no speech; update beliefs
- `backchannel`: short acknowledgement, only when the user appears to want it
- `capture`: reflect the goal or last useful fact
- `clarify`: ask exactly one question
- `shrink`: turn a broad task into one visible action
- `suggest`: offer one option, never a list of five
- `narrate`: state what the orb is doing while waiting on a backend
- `celebrate`: mark a real completion or recovery
- `repair`: explain what failed and what the user can do next
- `pause`: reduce stimulation and stop proactive speech

## 5. Conversational behavior by moment

### App launch

Speak first after audio readiness, not before the audio route exists:

```text
[warm] Good morning, Rachit. [short pause] I’m here with you.
I’m loading your recent focus context now. You can say what you want to work on, or we can just start small.
```

The backend should preload user context concurrently. The greeting must not claim data is loaded until the preload result is known.

### User says hello

Use a short varied response, then leave space:

```text
[chuckle] Hey, Rachit. I’m here. [short pause] What are we easing into?
```

Do not immediately convert a greeting into a task interrogation.

### User states a task

Reflect it in one sentence and propose the first action:

```text
Got it — we’re getting the project started. [emphasis] First tiny move: open the project folder.
Tell me when it’s open, and I’ll stay with you.
```

### User is working

Silence is usually correct. If the user explicitly asks for company, use sparse backchannels:

```text
Mm-hm. I’m with you.
```

Never emit a timer-driven “you still there?” while attention is likely focused.

### User is stalled

Use a gradual ladder:

1. No speech during the first adaptive grace period.
2. Offer presence: “Still here. No rush.”
3. Ask one question: “What is the part that feels stuck?”
4. Shrink: “Let’s only open the file. Nothing else yet.”
5. Offer two choices: “Want to open it together, or tell me what you see?”

Each step needs a cooldown and should cancel when the user resumes meaningful activity.

### User is overwhelmed or dysregulated

Lower rate, volume, novelty, and question complexity. Do not use celebration, forced optimism, streak pressure, or shame-adjacent language. Prefer:

```text
Okay. We can make this smaller. [long pause]
Would it help if I hold the next step while you take a breath?
```

### User completes a step

Reward the concrete event, then return control:

```text
[chuckle] Nice — that part is done. [emphasis] One real step forward.
[short pause] Ready for the next tiny one, or do you want a minute?
```

### STT or backend failure

Repair conversationally instead of silently repeating:

```text
I didn’t catch that clearly. [short pause] I’m still connected.
Could you say it once more, or tap the orb and try again?
```

If the provider is unavailable:

```text
Oh — I’m having trouble reaching the focus service. [short pause]
I’ll keep this session here and retry once. You can also continue with the last step while I reconnect.
```

## 6. Fish emotion markup and LLM contract

The LLM may suggest expressive markup, but it must not control safety, state transitions, or provider parameters directly. The gateway validates and normalizes it.

### Allowed markup for the first version

```text
[warm] [chuckle] [emphasis] [short pause] [long pause] [curious] [gentle] [celebratory]
```

Example:

```text
[chuckle] When you’re creating something new, there’s this [emphasis] beautiful mix of wonder and fear.
[long pause] It can feel overwhelming and magical at the same time.
```

### Structured response contract

```json
{
  "intent": "presence|capture|clarify|shrink|suggest|celebrate|repair|pause",
  "spoken_text": "[warm] I’m here. [short pause] Let’s make this smaller.",
  "plain_text": "I’m here. Let’s make this smaller.",
  "emotion": "warm",
  "interruptible": true,
  "max_duration_ms": 4200,
  "follow_up": {
    "kind": "wait_for_user|schedule_check_in|none",
    "delay_ms": 0
  }
}
```

Validation rules:

- Strip unknown tags and preserve the plain text.
- Enforce a maximum spoken duration and one question maximum.
- State-derived prosody wins over an LLM suggestion when they conflict.
- `[long pause]` is forbidden in urgent failure, consent, or safety messages.
- Never place user secrets or raw backend errors into speech.

## 7. Scheduling and deferred presence

Use a cancellable event queue, not independent timers scattered through the app.

```text
PresenceEvent {
  id,
  kind,
  due_at,
  expires_at,
  priority,
  requires: [not_listening, not_muted, goal_active],
  cancel_on: [user_speaks, task_done, pause, error],
  payload
}
```

Recommended initial timing, subject to belief-based adjustment:

| Event | Initial delay | Condition | Action |
|---|---:|---|---|
| launch greeting | audio ready + 250ms | first launch | speak first |
| context ready | preload complete | greeting did not claim completion | brief update |
| gentle presence | 60–90s | user asked for company, no speech | one backchannel |
| stall assist | 2–3 min | execution stalled, not focused | offer one question |
| recovery assist | 30s after failure | provider retryable | speak repair/retry |
| completion follow-up | 5–10s | step done, user not speaking | ask whether to continue |
| abandoned-goal rescue | 10–20 min | goal decaying and app foregrounded | offer resume/archive |

Rules:

- Every scheduled event has an expiry and a cancellation path.
- At most one proactive speech event may be pending per goal.
- Never interrupt active listening or likely hyperfocus.
- Reset or extend cooldown after any user response.
- Background execution and notifications require separate consent; foreground behavior must remain correct without them.

## 8. Energy and novelty regulation

Energy is a bounded control loop, not “more excitement forever”.

```text
energy = base_for_state
       + novelty_bonus_during_engage_window
       + reward_bonus_after_real_progress
       - overload_penalty
       - repeated_ignore_penalty
```

Controls:

- Voice speed: small changes only; keep intelligibility above novelty.
- Voice loudness: normalize loudness; never use sudden volume jumps.
- Prosody: use `[chuckle]`, `[emphasis]`, and pauses sparingly.
- Wording: vary acknowledgement phrases, not the user’s actual goal.
- Orb: reuse the existing state visuals and bounded volume input.
- Cooling: after a reward burst, return to calm within one interaction.

If the user ignores two proactive messages, suppress proactive speech for the next cooldown window. If the user says “too much”, “quiet”, or “stop”, immediately enter `pause` and require explicit reactivation.

## 9. Memory that creates presence

Keep a small session memory, not a surveillance transcript:

```text
current_goal
current_step
last_confirmed_progress
user_preference: quiet|gentle_backchannels|energetic
known_blocker
last_repair
next_scheduled_event
```

At every response, prefer continuity:

```text
You were opening the project folder. We’re still on that one step.
```

Do not restate the entire history. The body double should feel attentive, not verbose.

## 10. Metrics and evaluation

Track behavior quality, not only token and latency metrics:

- `time_to_first_greeting_ms`
- `stt_transcript_confidence`
- `repair_rate_after_low_confidence`
- `proactive_speech_per_active_hour`
- `ignored_proactive_event_rate`
- `user_barge_in_rate`
- `unwanted_interruption_rate`
- `step_completion_after_intervention_rate`
- `goal_resume_rate_after_decay`
- `perceived_presence_score` from a short opt-in user rating

Guardrails:

- No proactive speech while `user_turn`, `muted`, `dysregulated`, or `hyperfocus` unless explicitly requested.
- No more than one question per turn.
- No repeated wording more than twice in a row.
- No automatic escalation based only on silence.
- Every backend error has a spoken repair path and a structured log.

## 11. Delivery plan

### P0 — conversational core

1. Add the structured response contract and markup validator at the gateway boundary.
2. Add the cancellable `PresenceEvent` queue with foreground lifecycle handling.
3. Move proactive behavior behind the orthogonal belief/policy interface.
4. Add tests for launch, hello, listening, focused silence, stall, failure repair, completion, mute, and barge-in.

### P1 — presence quality

1. Add session memory and user preference for quiet versus energetic companionship.
2. Add low-confidence STT repair and transcript visibility in diagnostics.
3. Add response variant selection with a no-repeat window.
4. Add Fish markup rendering tests and an intelligibility audio fixture.

### P2 — optional sensors

1. Add screen/keyboard/activity signals only behind explicit consent.
2. Add adaptive stall thresholds from user feedback.
3. Add offline scheduling and notification behavior as a separate product decision.

## 12. Acceptance journeys

The feature is not done until these journeys work end to end:

1. Launch → audio ready → personalized greeting → context preload → user can answer naturally.
2. “Hi” → varied human acknowledgement → no forced task intake.
3. User states a task → orb reflects it → one small step → quiet co-working.
4. User works silently for several minutes → no annoying interruption → optional gentle presence when requested.
5. User says “I’m stuck” → one question → smaller action → progress celebration.
6. STT confidence drops → clear repair → no repeated hallucinated response.
7. Backend fails → spoken failure message → retry and local continuity.
8. User says “stop” → all scheduled speech cancels immediately.
9. User completes a step → expressive but intelligible reward → returns control.

