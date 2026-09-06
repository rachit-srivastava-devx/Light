import { applyEvidence, initialBeliefs } from '../cognitive/BeliefModel';
import { detectCrisis } from '../cognitive/CrisisDetector';
import { decideEvidence } from '../cognitive/EvidenceTiers';
import { decide, type PolicyInput } from '../cognitive/Policy';
import type { BeliefVector, Intervention, RegisterSet, InterventionRecord } from '../cognitive/contracts';
import {
  RESPONSE_ENVELOPE_VERSION,
  type ResponseEnvelope,
  type SpeechAudio,
} from '../lld/ResponseEnvelope';
import {
  EMPTY_SLOTS,
  MAX_CLARIFY_QUESTIONS,
  decideClarify,
  extractSlots,
  type SlotState,
} from '../lld/ClarifyProtocol';
import { retrieve, type ContextPack } from '../lld/ContextPack';
import { selectProsody } from '../lld/ProsodyDirector';
import {
  classifyByRule,
  classifyIntakeUtterance,
  classifySocialUtterance,
  isLikelyTaskRequest,
  isTeachRequestUtterance,
} from '../router/IntentClassifier';
import { route } from '../router/Router';
import type { IntentLabel, ModedResponseEnvelope, OrbMode, RouterAction } from '../router/contracts';
import { dispatch } from '../session/StateMachine';
import type { EpochMs, SessionSnapshot, SessionState } from '../session/contracts';
import { StepGate } from '../session/StepGate';
import { utf8Decode } from '../voice/Utf8';
import {
  createStaticAtomizerPort,
  type AtomizerPort,
  type AtomizerPortResult,
} from './AtomizerPort';
import { createStaticConversationPort, type ConversationPort } from './ConversationPort';
import chatResponses from '../shared/chat-responses.v1.json';

export interface T0FocusSessionInput {
  readonly tenant_id: string;
  readonly user_id: string;
  readonly session_id: string;
  readonly task: string;
  readonly context_pack?: ContextPack;
}

export interface T0FocusSessionRuntime {
  readonly identity: () => Pick<T0FocusSessionInput, 'tenant_id' | 'user_id' | 'session_id'>;
  readonly setContextPack?: (contextPack: ContextPack) => void;
  readonly start: () => Promise<ModedResponseEnvelope>;
  readonly acceptAudio: (bytes: Uint8Array) => Promise<ModedResponseEnvelope>;
  readonly speakCurrentStep: () => Promise<ModedResponseEnvelope>;
  readonly pause: () => Promise<ModedResponseEnvelope>;
  /**
   * Proactive re-engagement tick (§13 CHECK_IN — belief-driven, not a fixed nag). The caller (an
   * impure boundary — App.tsx already owns `Date.now()` for the presence bed) polls this on a real
   * wall-clock interval; `now` is the only clock input, so replay stays deterministic (AGENTS.md
   * invariant 6). Returns `null` on every tick that isn't WORKING or where the policy observes —
   * silence is the correct default (§2 DO-NO-HARM), so most ticks return null.
   */
  readonly checkIn: (now: EpochMs) => Promise<ModedResponseEnvelope | null>;
  /** One bounded, human presence line for launch/intake or a quiet working session. */
  readonly proactivePresence: () => Promise<ModedResponseEnvelope | null>;
}

const FALLBACK_INTENT: IntentLabel = 'chitchat';
/** Exported so callers can tell a LOCAL rejection apart from a real model reply. Without that
 *  distinction the two are indistinguishable from outside this module — both arrive as an ordinary
 *  spoken envelope — which is why a local reject has been invisible in the dev logs. */
export const UNRECOGNIZED_SPEECH_TEXT = "I couldn't make that out clearly. Please say it again.";
/**
 * B5 (ORB-ACCEPTANCE-CONTRACT) — bound on a single utterance forwarded to classification/atomizer/
 * conversation, so a pathologically long transcript (the contract's 10,000-char case) is a bounded,
 * documented, deterministic truncation rather than an unbounded string reaching every downstream
 * regex and (eventually) a paid model call. Kept well under the relay's own
 * `_MAX_CONVERSATION_PROMPT_CHARS` (8,000 — `backend/relay-py/src/orb_relay/app.py`) so there is
 * still budget left for conversation history once this turn's text is added.
 */
const MAX_INTAKE_UTTERANCE_CHARS = 4_000;

/**
 * Launch greetings. Each has its own pre-recorded `phrase_id` (see `cachedPhrase`) so varying the
 * greeting never costs a live TTS call. Which index plays is chosen by the caller (App.tsx, the
 * impure boundary that already owns `Date.now()`) and injected via `greetingIndex` — this module
 * stays free of wall-clock/random per the determinism invariant (AGENTS.md).
 */
const GREETINGS: readonly { readonly text: string; readonly phrase_id: string }[] = [
  { text: "I'm here. Tell me the task.", phrase_id: 'presence.here.v1' },
  { text: "Hey. What are we tackling?", phrase_id: 'presence.here.v2' },
  { text: "Ready when you are — what's the task?", phrase_id: 'presence.here.v3' },
  { text: "Let's go. Tell me what you're working on.", phrase_id: 'presence.here.v4' },
];

const CHAT_RESPONSES = chatResponses.responses as Readonly<{
  readonly greeting: readonly string[];
  readonly presence: readonly string[];
}>;

/**
 * Crisis response copy — EXACT WORDING, do not alter. A human safety reviewer must sign off on
 * this text before it reaches a real user; see the CrisisDetector task report for that flag.
 */
const CRISIS_RESPONSE_TEXT =
  "I'm not able to help with this, but you don't have to go through it alone. Tele-MANAS is " +
  "India's free, 24/7 mental health helpline — call or message 14416, any time.";

export function createT0FocusSession(
  input: T0FocusSessionInput,
  atomizer: AtomizerPort = createStaticAtomizerPort(),
  greetingIndex = 0,
  conversation: ConversationPort = createStaticConversationPort(),
): T0FocusSessionRuntime {
  const greeting = GREETINGS[((greetingIndex % GREETINGS.length) + GREETINGS.length) % GREETINGS.length] as {
    readonly text: string;
    readonly phrase_id: string;
  };
  let seq = 0;
  let state: SessionState = 'IDLE_PRESENT';
  let gate: StepGate | null = null;
  let stuckCount = 0;
  let interruptionKind: 'none' | 'user_pause' | 'os_interrupt' = 'none';
  let beliefs = initialBeliefs();
  let contextPack = input.context_pack;
  let activeTask = input.task;
  let policyNow = 0;
  // Real wall-clock anchors for checkIn()'s session_elapsed_ms/execution_stalled_ms — distinct
  // from policyNow above (a logical per-call counter used by the turn-driven path). Both are set
  // lazily from checkIn()'s first `now` rather than at construction, since nothing here may call
  // Date.now() itself (AGENTS.md determinism invariant); stepAnchorMs resets on every new step.
  let sessionAnchorMs: EpochMs | null = null;
  let stepAnchorMs: EpochMs | null = null;
  let clarifySlots: SlotState = EMPTY_SLOTS;
  let clarifyQuestionsAsked = 0;
  let intakeTask = '';
  let socialReplyIndex = greetingIndex;
  let proactivePresenceIndex = greetingIndex;
  const interventionHistory: InterventionRecord[] = [];


  async function atomizeTask(task: string): Promise<AtomizerPortResult> {
    const pack = contextPack ?? {
      profile: { user_id: input.user_id },
      recent_tasks: [],
      open_loops: [],
      open_session: null,
    };
    const retrieval = retrieve(pack, task, simpleEmbedding(task));
    if (retrieval.kind === 'reuse') {
      return {
        output: retrieval.task.steps,
        envelope_source: 'reuse',
        atomizer_source: 'context_pack_reuse',
        spent_paise: 0,
        latency_ms: 0,
      };
    }
    return atomizer.atomize({ ...input, task });
  }

  async function firstStepFromTask(task: string): Promise<ModedResponseEnvelope> {
    activeTask = task;
    const result = await atomizeTask(task);
    gate = new StepGate(result.output);
    const step = gate.advance();
    state = dispatch(state, 'atomize_ready', { atomizer_output_valid: true });
    return makeEnvelope(
      input,
      seq,
      state,
      gate,
      result.failure_message ?? `Start with: ${step.step_text}`,
      result.envelope_source,
      result.spent_paise,
      result.latency_ms,
      interruptionKind,
      'focus',
    );
  }

  function advanceFromDone(): ModedResponseEnvelope {
    if (gate === null) return makeEnvelope(input, seq, state, gate, "I need the task first.", 'reuse', 0, 0, interruptionKind, 'focus');
    state = dispatch(state, 'intent_done', { intent: 'done' });
    state = dispatch(state, 'step_advance', {
      step_index: gate.currentIndex,
      steps_total: gate.steps_total,
    });
    if (state === 'STEP_PRESENT') {
      const step = gate.advance();
      return makeEnvelope(input, seq, state, gate, `Nice. Next: ${step.step_text}`, 'reuse', 0, 0, interruptionKind, 'focus');
    }
    return makeEnvelope(input, seq, state, gate, 'Done. We can end here.', 'reuse', 0, 0, interruptionKind, 'focus');
  }

  function advanceFromNext(): ModedResponseEnvelope {
    if (gate === null) return makeEnvelope(input, seq, state, gate, "I need the task first.", 'reuse', 0, 0, interruptionKind, 'focus');
    if (gate.isLastStep) {
      return makeEnvelope(input, seq, state, gate, 'This is the last step. Say done when it is finished.', 'reuse', 0, 0, interruptionKind, 'focus');
    }
    state = dispatch(state, 'intent_next', {
      step_index: gate.currentIndex,
      steps_total: gate.steps_total,
    });
    const step = gate.advance();
    return makeEnvelope(input, seq, state, gate, `Next: ${step.step_text}`, 'reuse', 0, 0, interruptionKind, 'focus');
  }

  async function handleWorkingIntent(intent: IntentLabel, transcript: string): Promise<ModedResponseEnvelope> {
    // `classifySafeIntent` deliberately falls back to chitchat for unknown/negated task speech;
    // only exact social phrases may skip evidence observation and use the local chat pack.
    const socialKind = classifySocialUtterance(transcript);
    if (intent === 'chitchat' && socialKind !== null) return socialEnvelope(socialKind);
    observeTranscript(transcript);
    const nextStuckCount = intent === 'stuck' ? stuckCount + 1 : 0;
    const action = route(state, intent, nextStuckCount);
    stuckCount = nextStuckCount;
    if (action === 'advance_step') return intent === 'done' ? advanceFromDone() : advanceFromNext();
    if (action === 'pause_session') return pauseEnvelope();
    if (action === 'reanchor_templated') {
      applyPolicy();
      return reanchorEnvelope();
    }
    if (action === 'crew_reatomize') return reatomizeCurrentPlan();
    return fastVoiceEnvelope(action, intent, transcript);
  }

  function socialEnvelope(kind: 'greeting' | 'presence'): ModedResponseEnvelope {
    const responses = CHAT_RESPONSES[kind];
    const index = ((socialReplyIndex++ % responses.length) + responses.length) % responses.length;
    return makeEnvelope(input, seq, state, gate, responses[index] as string, 'reuse', 0, 0, interruptionKind, 'converse');
  }

  async function startNewTask(task: string): Promise<ModedResponseEnvelope> {
    state = dispatch(state, 'new_task');
    gate = null;
    stuckCount = 0;
    interruptionKind = 'none';
    intakeTask = '';
    clarifySlots = EMPTY_SLOTS;
    clarifyQuestionsAsked = 0;
    return firstStepFromTask(task);
  }

  /**
   * @param mode B4 — explicit, never inferred by the caller. Every call site below picks it from
   *   the same classification that decided to route here in the first place (never re-derived ad
   *   hoc), defaulting to `'converse'` for the pre-existing WORKING-state call sites that don't
   *   pass one explicitly.
   */
  async function respondToConversation(transcript: string, mode: OrbMode = 'converse'): Promise<ModedResponseEnvelope> {
    observeTranscript(transcript);
    const result = await conversation.respond({
      ...input,
      text: transcript,
      active_task: activeTask,
      current_step: gate?.currentStep?.step_text,
      session_state: state,
      // Send the mode the router already decided. Previously this was passed only to
      // makeEnvelope() below, so the relay never saw it and always applied its FOCUS default —
      // which made teach/converse structurally unreachable from the app.
      mode,
    });
    return makeEnvelope(
      input,
      seq,
      state,
      gate,
      result.text,
      result.source,
      result.spent_paise,
      result.latency_ms,
      interruptionKind,
      mode,
    );
  }

  function pauseEnvelope(): ModedResponseEnvelope {
    interruptionKind = 'user_pause';
    state = dispatch(state, 'user_pause');
    return makeEnvelope(input, seq, state, gate, 'Paused. Not listening.', 'reuse', 0, 0, interruptionKind, 'focus');
  }

  async function reatomizeCurrentPlan(): Promise<ModedResponseEnvelope> {
    const task = gate?.currentStep?.step_text ?? input.task;
    const result = await atomizeTask(task);
    const replacement = new StepGate(result.output);
    const step = replacement.advance();
    gate = replacement;
    state = dispatch(state, 'reatomize_ready', { atomizer_output_valid: true });
    return makeEnvelope(
      input,
      seq,
      state,
      gate,
      `Try this smaller step: ${step.step_text}`,
      result.envelope_source,
      result.spent_paise,
      result.latency_ms,
      interruptionKind,
      'focus',
    );
  }

  function reanchorEnvelope(): ModedResponseEnvelope {
    const step = gate?.currentStep;
    return makeEnvelope(
      input,
      seq,
      state,
      gate,
      step ? `Stay with this: ${step.step_text}` : "Let's name the first small action.",
      'reuse',
      0,
      0,
      interruptionKind,
      'focus',
    );
  }

  function fastVoiceEnvelope(action: RouterAction, intent: IntentLabel, transcript: string): ModedResponseEnvelope {
    let text = "I'm here. Tell me the task.";
    if (action === 'fast_voice' && transcript.trim().length > 0) {
      text =
        intent === 'question'
            ? "Tell me what you're looking at, and I'll give you one small next step."
            : intent === 'chitchat'
              ? "I'm with you. We can chat, or pick one small thing to do."
              : "I heard you. Tell me what would help: a smaller step, the next step, or a pause.";
    }
    return makeEnvelope(input, seq, state, gate, text, 'reuse', 0, 0, interruptionKind, 'converse');
  }

  return {
    identity: () => ({
      tenant_id: input.tenant_id,
      user_id: input.user_id,
      session_id: input.session_id,
    }),
    setContextPack(nextContextPack: ContextPack) {
      contextPack = nextContextPack;
    },
    async start() {
      seq += 1;
      state = dispatch(state, 'tap');
      return makeEnvelope(input, seq, state, gate, greeting.text, 'reuse', 0, 0, interruptionKind, 'focus');
    },
    async acceptAudio(bytes: Uint8Array) {
      // B5 — a native-bridge/JSON boundary can hand this a null/undefined value at runtime even
      // though the type says `Uint8Array`; degrade to an empty transcript (handled just below)
      // rather than let `transcriptFromBytes` throw on `bytes.length`.
      const rawTranscript = bytes ? transcriptFromBytes(bytes) : '';
      // Crisis check short-circuits everything else on this turn — no atomizer, no policy, no
      // gateway/LLM call, entirely offline and deterministic. Must run before any other
      // classification/routing (AGENTS.md determinism invariant; task safety requirement). Runs on
      // the full, untruncated transcript: safety is never subject to the length bound below.
      if (detectCrisis(rawTranscript)) {
        seq += 1;
        // Distinctly-tagged, transcript-free log: auditable that the path fired, without
        // persisting the user's actual words.
        console.warn('focus-orb:crisis-detected', {
          session_id: input.session_id,
          turn_id: `${input.session_id}-${seq}`,
        });
        return makeEnvelope(
          input,
          seq,
          state,
          gate,
          CRISIS_RESPONSE_TEXT,
          'cache_hit',
          0,
          0,
          interruptionKind,
          'converse',
        );
      }
      // B5 — empty · whitespace-only · unicode/emoji-only · leading/trailing-punctuation-only all
      // collapse to "no recognizable linguistic content" (see `isContentless`): none contain a
      // single letter or digit in any script. Handled explicitly and identically — a bounded repair
      // prompt — rather than falling through into intake routing, where (before this guard existed)
      // an emoji-only transcript was treated as literal task text and atomized verbatim.
      if (isContentless(rawTranscript)) {
        return makeEnvelope(input, seq, state, gate, UNRECOGNIZED_SPEECH_TEXT, 'cache_hit', 0, 0, interruptionKind, 'focus');
      }
      // B5 — bound a pathologically long utterance (the contract's 10,000-char case) before
      // classification or forwarding; see `MAX_INTAKE_UTTERANCE_CHARS`.
      const transcript =
        rawTranscript.length > MAX_INTAKE_UTTERANCE_CHARS
          ? rawTranscript.slice(0, MAX_INTAKE_UTTERANCE_CHARS)
          : rawTranscript;
      // A short Devanagari rendering of common English words is a known iOS/Fish ASR failure mode
      // (for example, "what page" -> "वॉट पेज"). Reject it before intake/task routing: otherwise
      // the corrupted text is appended to the pending task and context retrieval can reuse an
      // unrelated prior plan. Genuine Hindi tasks remain valid input.
      if ((state === 'INTAKE' || state === 'CLARIFY') && isLikelyPhoneticEnglishTranscript(transcript)) {
        return makeEnvelope(input, seq, state, gate, UNRECOGNIZED_SPEECH_TEXT, 'cache_hit', 0, 0, interruptionKind, 'focus');
      }
      seq += 1;
      const socialKind = classifySocialUtterance(transcript);
      if (socialKind !== null) {
        if (state === 'CHECK_IN') state = dispatch(state, 'reply');
        return socialEnvelope(socialKind);
      }
      // Overwhelm is a conversational need, not another generic clarification. Handle it before
      // INTAKE/task routing so "I am stuck and overwhelmed" cannot repeat the same smallest-part
      // question that was already heard in the live human-simulation run.
      if (isOverwhelmedUtterance(transcript)) {
        observeTranscript(transcript);
        return makeEnvelope(
          input,
          seq,
          state,
          gate,
          "That sounds like a lot. Let's slow down together. What's the one thing directly in front of you?",
          'reuse',
          0,
          0,
          interruptionKind,
          'converse',
        );
      }
      if (state === 'INTAKE' || state === 'CLARIFY') {
        // ORB-ACCEPTANCE-CONTRACT B1/B2/B3 — a FIRST utterance at INTAKE is not always a task
        // ("teach me about photosynthesis" and a dozen other open-domain openers were previously
        // swallowed by the atomizer/clarify path). classifyIntakeUtterance always checks
        // isLikelyTaskRequest first and only ever redirects utterances that already failed it, so a
        // real task can never be rerouted here — see IntentClassifier.ts's doc comment for the
        // regression argument in full.
        if (state === 'INTAKE') {
          const intakeRoute = classifyIntakeUtterance(transcript);
          if (intakeRoute !== 'task') return respondToConversation(transcript, intakeRoute);
        }
        state = dispatch(state, 'transcript');
        const intake = handleIntakeTranscript(transcript);
        if (intake.kind === 'ask') return clarifyEnvelope(intake.slot);
        return firstStepFromTask(intake.task);
      }
      if (state === 'CHECK_IN') {
        state = dispatch(state, 'reply');
      }
      if (isLikelyTaskRequest(transcript)) return startNewTask(transcript);
      const ruleClassification = classifyByRule(transcript);
      // In WORKING, non-ASCII output is still treated as an unrecognised final. There is no safe
      // way to apply a corrupted command to the current task.
      if (containsNonAscii(transcript)) {
        return makeEnvelope(input, seq, state, gate, UNRECOGNIZED_SPEECH_TEXT, 'cache_hit', 0, 0, interruptionKind, 'focus');
      }
      const conversationalIntent = ruleClassification.matched &&
        (ruleClassification.label === 'question' || ruleClassification.label === 'chitchat');
      // Tags which conversational backend this mid-session turn is (B4); does not change routing.
      const midSessionMode: OrbMode = isTeachRequestUtterance(transcript) ? 'teach' : 'converse';
      // Natural questions must reach the conversational backend even during WORKING. The task
      // router owns explicit controls; it must not turn a request for help into a canned re-anchor.
      if (conversationalIntent || transcript.includes('?')) return respondToConversation(transcript, midSessionMode);
      if (
        !ruleClassification.matched &&
        !containsNonAscii(transcript) &&
        !/\bnot\s+(done|finished|complete|completed)\b/i.test(transcript)
      ) {
        return respondToConversation(transcript, midSessionMode);
      }
      return handleWorkingIntent(classifySafeIntent(transcript), transcript);
    },
    async speakCurrentStep() {
      const step = gate?.currentStep;
      const speechText = step?.step_text ?? "I'm here. Tell me the task.";
      seq += 1;
      if (step) {
        state = dispatch(state, 'spoken_complete', {
          step_index: gate?.currentIndex,
          steps_total: gate?.steps_total,
        });
        stepAnchorMs = null;
        applyPolicy();
      }
      return makeEnvelope(input, seq, state, gate, speechText, 'reuse', 0, 0, interruptionKind, 'focus');
    },
    async checkIn(now: EpochMs) {
      if (state !== 'WORKING') return null;
      if (sessionAnchorMs === null) sessionAnchorMs = now;
      // First tick after a step became WORKING: anchor and wait for the next tick rather than
      // reporting a spurious 0ms-elapsed stall.
      if (stepAnchorMs === null) {
        stepAnchorMs = now;
        return null;
      }
      const policyInput: PolicyInput = {
        beliefs,
        registers: registersFromBeliefs(beliefs),
        session: snapshot(input, state, gate),
        history: interventionHistory,
        now,
        session_elapsed_ms: now - sessionAnchorMs,
        execution_stalled_ms: now - stepAnchorMs,
        step_just_completed: false,
        severity: 'normal',
      };
      const decision = decide(policyInput);
      if (decision.intervention === 'Observe') return null;
      interventionHistory.push({ intervention: decision.intervention, at_ts: now });
      state = dispatch(state, 'policy_intervention', { policy_passed_veto_and_budget: true });
      seq += 1;
      const text = checkInText(decision.intervention);
      return makeEnvelope(input, seq, state, gate, text, 'reuse', 0, 0, interruptionKind, 'focus');
    },
    async proactivePresence() {
      if (state === 'IDLE_PRESENT' || state === 'SESSION_DONE' || state === 'INTERRUPTED') return null;
      const step = gate?.currentStep?.step_text;
      const intakeLines = [
        "I'm set up and listening. You can give me a task, ask me something, or just talk.",
        "No need to phrase it perfectly. Tell me the messy version and we'll find the first move.",
        "We can start tiny. What are you looking at right now?",
      ] as const;
      const workingLines = [
        'Still beside you. What is the next physical action you can see?',
        'One minute on this step, then we reassess. Say the stuck part out loud if you need me.',
        step ? `Quick check: stay with "${step}" for one more small move.` : 'Quick check: what is the next visible move?',
      ] as const;
      const lines = state === 'WORKING' || state === 'CHECK_IN' ? workingLines : intakeLines;
      const text = lines[((proactivePresenceIndex++) % lines.length + lines.length) % lines.length] as string;
      seq += 1;
      return makeEnvelope(input, seq, state, gate, text, 'reuse', 0, 0, interruptionKind, 'focus');
    },
    async pause() {
      seq += 1;
      return pauseEnvelope();
    },
  };

  function applyPolicy(): void {
    policyNow += 1_000;
    const policyInput = defaultPolicyInput(
      snapshot(input, state, gate),
      interventionHistory,
      beliefs,
      registersFromBeliefs(beliefs),
      policyNow,
    );
    const decision = decide(policyInput);
    if (decision.intervention !== 'Observe') {
      interventionHistory.push({ intervention: decision.intervention, at_ts: policyInput.now });
      state = dispatch(state, 'policy_intervention', { policy_passed_veto_and_budget: true });
    }
  }

  function observeTranscript(transcript: string): void {
    const decision = decideEvidence({ utterance: transcript, embedding: [1], examples: [] });
    if (decision.kind !== 'evidence') return;
    policyNow += 1_000;
    beliefs = applyEvidence(beliefs, decision.evidence, policyNow);
  }

  function handleIntakeTranscript(transcript: string):
    | { readonly kind: 'ask'; readonly slot: keyof SlotState }
    | { readonly kind: 'proceed'; readonly task: string } {
    intakeTask = [intakeTask, transcript].filter((part) => part.trim().length > 0).join(' ');
    clarifySlots = extractSlots(transcript, clarifySlots);
    // A genuine Indic task is valid intake even though it cannot satisfy the English clarify-slot
    // parser. The phonetic-English guard in acceptAudio has already rejected known corrupted ASR.
    if (containsNonAscii(transcript)) return proceedFromIntakeTask(transcript);
    const decision = decideClarify(clarifySlots, clarifyQuestionsAsked);
    if (decision.kind === 'ask') {
      clarifyQuestionsAsked += 1;
      state = dispatch(state, 'needs_clarification', {
        slots_filled: filledRequiredClarifySlots(clarifySlots),
        slots_required: 2,
      });
      return decision;
    }
    if (state === 'CLARIFY') {
      state = dispatch(state, 'clarify_answered', {
        slots_filled: filledRequiredClarifySlots(clarifySlots),
        slots_required: 2,
        questions_asked: clarifyQuestionsAsked,
        question_cap: MAX_CLARIFY_QUESTIONS,
      });
    }
    return proceedFromIntakeTask(transcript);
  }

  function proceedFromIntakeTask(fallback: string): { readonly kind: 'proceed'; readonly task: string } {
    const task = intakeTask || fallback;
    intakeTask = '';
    clarifySlots = EMPTY_SLOTS;
    clarifyQuestionsAsked = 0;
    return { kind: 'proceed', task };
  }

  /**
   * CLARIFY's spoken question. Both slots used to hand the thinking straight back:
   *
   *   scope -> "What is the smallest part to start?"
   *   place -> "Where should we open it?"
   *
   * The owner heard the first one from a live voice session and reported it as the orb's whole
   * personality: *"it simply says what is the smallest part to start"*. Worth being precise about
   * why nothing caught it — this is a HARDCODED string in the FSM, not model output, so it never
   * passes through `/v1/respond` and `conversation_guard.shifts_mental_load` could never see it. A
   * guard on the model is not a guard on the product.
   *
   * Both now PROPOSE and invite a correction, which still fills the slot from the user's reply
   * while carrying the load (the owner's rule: suggest the smallest step, don't make them produce
   * it). `place` offers two concrete options rather than an open "where", per the choice-overload
   * finding that two named options beat an open question.
   */
  function clarifyEnvelope(slot: keyof SlotState): ModedResponseEnvelope {
    const text =
      slot === 'scope'
        ? "Let's take the smallest slice of that — I'd start with just the first bit. Want that, or a different part?"
        : 'Want to do this at your computer, or on your phone?';
    return makeEnvelope(input, seq, state, gate, text, 'cache_hit', 0, 0, interruptionKind, 'focus');
  }
}

function simpleEmbedding(text: string): readonly number[] {
  return [text.length > 0 ? 1 : 0];
}

function containsNonAscii(text: string): boolean {
  return /[^\u0000-\u007f]/.test(text);
}

const PHONETIC_ENGLISH_DEVANAGARI = new Set([
  'वॉट', 'व्हाट', 'पेज', 'हेलो', 'हैलो', 'हाउ', 'आर', 'यू', 'आई', 'एम', 'मी', 'माय',
  'प्लीज', 'ओपन', 'डोर', 'बाथरूम', 'कैन', 'टेल', 'द', 'नेक्स्ट', 'स्टेप', 'व्हेयर',
  'डू', 'डिड', 'क्लोज', 'फाइल', 'मेल', 'बिल', 'पे', 'पेमेंट', 'स्टडी', 'वर्क',
]);

const HINDI_FUNCTION_WORDS = new Set([
  'मुझे', 'मैं', 'मेरे', 'मेरा', 'है', 'हूँ', 'हूं', 'करना', 'करो', 'करनी', 'भरना', 'भरनी',
  'चाहिए', 'क्या', 'कैसे', 'कहाँ', 'कहां', 'और', 'से', 'को', 'का', 'की', 'में', 'यह', 'वह',
  'एक', 'दो', 'नहीं', 'हो', 'होगा', 'पर', 'तक',
]);

function isLikelyPhoneticEnglishTranscript(text: string): boolean {
  if (!containsNonAscii(text)) return false;
  const tokens = text
    .trim()
    .split(/\s+/)
    .map((token) => token.replace(/[^\u0900-\u097f]/gi, ''))
    .filter((token) => token.length > 0);
  if (tokens.length < 2 || tokens.some((token) => HINDI_FUNCTION_WORDS.has(token))) return false;
  const knownCount = tokens.filter((token) => PHONETIC_ENGLISH_DEVANAGARI.has(token)).length;
  return knownCount >= 2 && knownCount / tokens.length >= 0.66;
}

function isOverwhelmedUtterance(text: string): boolean {
  return /\b(overwhelmed|overloaded|spiraling|spiral|panicking|panic attack|too much)\b/i.test(text);
}

/**
 * B5 (ORB-ACCEPTANCE-CONTRACT) — true when a transcript has no letter or digit in any script:
 * empty, whitespace-only, unicode/emoji-only, and leading/trailing-punctuation-only all satisfy
 * this. Deliberately narrower than `containsNonAscii`, which is also true for a genuine non-Latin
 * task ("मुझे टैक्स भरना है") that must still reach the atomizer — this only catches transcripts
 * with no recognizable content at all, in any language.
 */
function isContentless(text: string): boolean {
  return !/[\p{L}\p{N}]/u.test(text);
}

function filledRequiredClarifySlots(slots: SlotState): number {
  return Number(slots.scope !== null) + Number(slots.first_context !== null);
}

function transcriptFromBytes(bytes: Uint8Array): string {
  return utf8Decode(bytes).trim();
}

function classifySafeIntent(transcript: string): IntentLabel {
  const normalized = transcript.toLowerCase();
  if (/\bnot\s+(done|finished|complete|completed)\b/.test(normalized)) return FALLBACK_INTENT;
  const result = classifyByRule(normalized);
  return result.matched ? result.label : FALLBACK_INTENT;
}

function snapshot(input: T0FocusSessionInput, state: SessionState, gate: StepGate | null): SessionSnapshot {
  const stepIndex = gate?.currentIndex ?? 0;
  const stepsTotal = gate?.steps_total ?? 0;
  if (state === 'IDLE_PRESENT' || state === 'SESSION_DONE' || state === 'INTERRUPTED') {
    return {
      session_id: input.session_id,
      state,
      step_index: stepIndex,
      steps_total: stepsTotal,
      interrupted_from: null,
      bed_active: state !== 'SESSION_DONE',
    };
  }
  return {
    session_id: input.session_id,
    state,
    step_index: stepIndex,
    steps_total: stepsTotal,
    interrupted_from: null,
    bed_active: true,
  };
}

function makeEnvelope(
  input: T0FocusSessionInput,
  seq: number,
  state: SessionState,
  gate: StepGate | null,
  speechText: string,
  source: ResponseEnvelope['meta']['source'],
  costPaise = 0,
  latencyMs = 0,
  interruptionKind: 'none' | 'user_pause' | 'os_interrupt' = 'none',
  // B4 — required, no default: every call site names its mode explicitly rather than one being
  // silently inferred or forgotten (a missing argument is a compile error, not a fallback value).
  mode: OrbMode,
): ModedResponseEnvelope {
  const step = gate?.currentStep;
  const prosody = selectProsody(state, null);
  return {
    v: RESPONSE_ENVELOPE_VERSION,
    // B4's response mode (converse/focus/teach) — distinct from `speech.audio.mode` below
    // (`SpeechAudio`'s unrelated 'cached'/'novel' discriminant; same word, different field, no
    // collision — just don't confuse the two while reading this object literal).
    mode,
    session_id: input.session_id,
    turn_id: `${input.session_id}-${seq}`,
    seq,
    speech: {
      text: speechText,
      audio: cachedPhrase(speechText) ?? { mode: 'novel' },
      voice: { provider: 'fish', voice_id: 'orb.warm.v1' },
    },
    orb: {
      emotion: prosody.emotion,
      intensity: state === 'WORKING' ? 0.6 : 0.45,
      animation: state === 'INTERRUPTED' ? 'hold' : 'breathe',
      bed: {
        color: 'brown',
        gain_db: state === 'INTERRUPTED' && interruptionKind === 'user_pause' ? -60 : -18,
      },
    },
    widgets: step
      ? [{ type: 'step_card', index: gate.currentIndex, total: gate.steps_total, text: step.step_text }]
      : [],
    session: { state, step_index: gate?.currentIndex ?? 0, steps_total: gate?.steps_total ?? 0 },
    meta: {
      model_version: 't0-deterministic',
      prompt_version: 't0.focus-session.v2',
      source,
      latency_ms: Math.max(0, Math.round(latencyMs)),
      cost_paise: costPaise,
      trace_id: `${input.tenant_id}:${input.session_id}:${seq}`,
    },
  };
}

/**
 * Warm, human copy for a proactive check-in — deliberately distinct from the reactive re-prompt
 * texts elsewhere in this file (those answer something the user just said; this speaks first,
 * unasked, so it must read like someone checking in on you, not a nag or a status query).
 */
function checkInText(intervention: Intervention): string {
  switch (intervention) {
    case 'Presence':
      return "Still with you. No rush — I'm here whenever you're ready to talk it through.";
    case 'Capture':
      return "Before it slips — want to just say what's on your mind? I'll hold onto it.";
    case 'Celebrate':
      return "Hey, that was real progress. Nice work.";
    case 'Clarify':
      return "I noticed this step's been sitting a while — want to break it down smaller together?";
    case 'Suggest':
      return "Quick thought, only if it helps: sometimes the smallest next move is just opening the thing.";
    case 'Redirect':
      return "I hear you — let's come back to this one step. What's true right in front of you?";
    case 'Pause':
      return "Let's pause for a second. You don't have to push through this right now.";
    case 'Escalate':
      return "This one's been tough for a while. Want to stop here and just be done for now?";
    case 'Observe':
      return "I'm here. Tell me the task.";
    default: {
      const _exhaustive: never = intervention;
      return _exhaustive;
    }
  }
}

function cachedPhrase(text: string): SpeechAudio | null {
  const greeting = GREETINGS.find((g) => g.text === text);
  if (greeting) return { mode: 'cached', phrase_id: greeting.phrase_id };
  if (text === 'Paused. Not listening.') return { mode: 'cached', phrase_id: 'pause.v1' };
  return null;
}

function defaultPolicyInput(
  session: SessionSnapshot,
  history: readonly InterventionRecord[],
  beliefs: BeliefVector,
  registers: RegisterSet,
  now: number,
): PolicyInput {
  return {
    beliefs,
    registers,
    session,
    history,
    now,
    session_elapsed_ms: 0,
    execution_stalled_ms: 0,
    step_just_completed: false,
    severity: 'normal',
  };
}

function defaultRegisters(): RegisterSet {
  return {
    goal: {
      NoGoal: 0,
      Formation: 0,
      Renegotiation: 0,
      Commitment: 1,
      Maintenance: 0,
      Completion: 0,
      Decay: 0,
    },
    attention: {
      Initiating: 0,
      Focused: 0.5,
      Exploring: 0,
      MindWandering: 0,
      Hyperfocus: 0,
      ExternalInterruption: 0,
      Recovering: 0,
    },
    barrier: {
      Blocked: 0,
      Uncertain: 0,
      Overwhelmed: 0,
      UnderStimulated: 0,
      Avoiding: 0,
      Waiting: 0,
      Fatigued: 0,
      Dysregulated: 0,
    },
  };
}

function registersFromBeliefs(beliefs: BeliefVector): RegisterSet {
  const base = defaultRegisters();
  const executionBlocked = beliefs.Execution.confidence >= 0.3 && beliefs.Execution.value < 0.35;
  const workingMemoryLow = beliefs.WorkingMemory.confidence >= 0.3 && beliefs.WorkingMemory.value < 0.35;
  const emotionallyLoaded = beliefs.EmotionalLoad.confidence >= 0.3 && beliefs.EmotionalLoad.value > 0.65;
  const distracted = beliefs.Attention.confidence >= 0.3 && beliefs.Attention.value < 0.35;

  return {
    goal: base.goal,
    attention: {
      ...base.attention,
      Focused: distracted ? 0.2 : base.attention.Focused,
      MindWandering: distracted ? 0.8 : base.attention.MindWandering,
    },
    barrier: {
      ...base.barrier,
      Blocked: executionBlocked ? 0.85 : base.barrier.Blocked,
      Uncertain: workingMemoryLow ? 0.75 : base.barrier.Uncertain,
      Overwhelmed: emotionallyLoaded ? 0.8 : base.barrier.Overwhelmed,
    },
  };
}
