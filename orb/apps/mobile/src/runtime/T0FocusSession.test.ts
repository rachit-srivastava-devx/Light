import { describe, expect, it, vi } from 'vitest';

import { screenModelFromEnvelope } from '../AppModel';
import { utf8Encode } from '../voice/Utf8';
import { createStaticAtomizerPort, type AtomizerPort, type AtomizerPortInput } from './AtomizerPort';
import type { ConversationPort } from './ConversationPort';
import { createT0FocusSession } from './T0FocusSession';

function bytes(text: string): Uint8Array {
  return utf8Encode(text);
}

function staticStep(step_text: string): AtomizerPort {
  return createStaticAtomizerPort({
    step_text,
    est_min: 1,
    done_signal: `${step_text} is visible`,
  });
}

/** Echoes back whatever text it receives, tagged so a test can tell this port fired at all. */
function conversationEcho(): ConversationPort {
  return {
    respond: vi.fn(async ({ text }) => ({
      text: `[echo] ${text}`,
      source: 'model' as const,
      latency_ms: 10,
      spent_paise: 1,
    })),
  };
}

function twoStepAtomizer(): AtomizerPort {
  return {
    async atomize() {
      return {
        output: {
          steps: [
            { step_text: 'Open the tax portal', est_min: 1, done_signal: 'portal is visible' },
            { step_text: 'download the statement', est_min: 2, done_signal: 'statement is downloaded' },
          ],
          steps_total: 2,
        },
        envelope_source: 'model',
        atomizer_source: 'model',
        spent_paise: 7,
        latency_ms: 42,
      };
    },
  };
}

describe('T0FocusSession runtime', () => {
  it('routes transcript through atomizer, step gate, FSM, and response envelope', async () => {
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, staticStep('Open the tax portal'));

    expect(screenModelFromEnvelope(await runtime.start())).toMatchObject({
      stateLabel: 'INTAKE',
      speechText: "I'm here. Tell me the task.",
    });
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('just open the tax portal')))).toMatchObject({
      stateLabel: 'STEP PRESENT',
      stepLabel: '1/1 Open the tax portal',
    });
    expect(screenModelFromEnvelope(await runtime.speakCurrentStep())).toMatchObject({
      stateLabel: 'WORKING',
      speechText: 'Open the tax portal',
    });
    const paused = await runtime.pause();
    expect(screenModelFromEnvelope(paused)).toMatchObject({
      stateLabel: 'INTERRUPTED',
      speechText: 'Paused. Not listening.',
    });
    expect(paused.orb.bed.gain_db).toBe(-60);
  });

  it('speaks an honest planner failure while keeping a tiny local fallback available', async () => {
    const atomizer: AtomizerPort = {
      async atomize() {
        return {
          output: {
            steps: [{ step_text: 'Open one small section', est_min: 1, done_signal: 'section is open' }],
            steps_total: 1,
          },
          envelope_source: 'cache_hit',
          atomizer_source: 'backend_unavailable',
          spent_paise: 0,
          latency_ms: 1200,
          failure_message: "I couldn't reach the task planner. I'll use one tiny fallback: Open one small section.",
        };
      },
    };
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, atomizer);

    await runtime.start();
    const envelope = await runtime.acceptAudio(bytes('just open the tax portal'));

    expect(screenModelFromEnvelope(envelope)).toMatchObject({
      stateLabel: 'STEP PRESENT',
      speechText: "I couldn't reach the task planner. I'll use one tiny fallback: Open one small section.",
    });
    expect(envelope.widgets?.find((widget) => widget.type === 'step_card')).toMatchObject({
      text: 'Open one small section',
    });
  });

  it('advances and ends only on explicit done intents', async () => {
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, twoStepAtomizer());

    await runtime.start();
    await runtime.acceptAudio(bytes('just open the tax portal and download the statement'));
    await runtime.speakCurrentStep();
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('not done yet')))).toMatchObject({
      stateLabel: 'WORKING',
      stepLabel: '1/2 Open the tax portal',
    });
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('done')))).toMatchObject({
      stateLabel: 'STEP PRESENT',
      stepLabel: '2/2 download the statement',
    });
    await runtime.speakCurrentStep();
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('done')))).toMatchObject({
      stateLabel: 'SESSION DONE',
      stepLabel: '2/2 download the statement',
    });
  });

  it('answers working-state questions and chitchat with distinct, useful speech', async () => {
    const conversation: ConversationPort = {
      respond: vi.fn(async ({ text }) => ({
        text: `[warm] I can help with that. You said: ${text}`,
        source: 'model' as const,
        latency_ms: 42,
        spent_paise: 7,
      })),
    };
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, staticStep('Open the tax portal'), 0, conversation);

    await runtime.start();
    await runtime.acceptAudio(bytes('just open the tax portal'));
    await runtime.speakCurrentStep();

    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('what should I do here?'))).speechText).toBe(
      '[warm] I can help with that. You said: what should I do here?',
    );
    expect(conversation.respond).toHaveBeenCalledWith(expect.objectContaining({
      text: 'what should I do here?',
      session_id: 's1',
    }));
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('hey there'))).speechText).toBe(
      "Hi, Rachit. I'm here with you.",
    );
  });

  it('acknowledges overwhelm instead of repeating a generic clarification', async () => {
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
      staticStep('unused'),
    );
    await runtime.start();

    const reply = await runtime.acceptAudio(bytes('I am stuck and overwhelmed'));

    expect(reply.speech?.text).toContain('That sounds like a lot');
    expect(reply.speech?.text).toContain('directly in front of you');
    expect(reply.speech?.text).not.toBe("Let's take the smallest slice of that — I'd start with just the first bit. Want that, or a different part?");
  });

  it('does not turn an unrecognised transcript into a repeated presence response', async () => {
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
      staticStep('Open the tax portal'),
    );
    await runtime.start();
    await runtime.acceptAudio(bytes('just open the tax portal'));
    await runtime.speakCurrentStep();

    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('यह अस्पष्ट है'))).speechText).toBe(
      "I couldn't make that out clearly. Please say it again.",
    );
  });

  it('rejects a corrupted non-ASCII intake transcript before it can reuse a prior task', async () => {
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
      staticStep('Open the bathroom door'),
    );

    await runtime.start();
    const reply = await runtime.acceptAudio(bytes('वॉट पेज'));

    expect(reply.session.state).toBe('INTAKE');
    expect(reply.meta.source).toBe('cache_hit');
    expect(reply.speech?.text).toBe("I couldn't make that out clearly. Please say it again.");
  });

  it('treats a combined greeting and presence question as local chitchat', async () => {
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
      staticStep('unused'),
    );
    await runtime.start();

    const reply = await runtime.acceptAudio(bytes('hello how are you'));

    expect(reply.meta.source).toBe('reuse');
    expect(reply.speech?.text).toMatch(/here|hear|chat/i);
  });

  it('sends uncategorized English speech to the conversational backend', async () => {
    const conversation: ConversationPort = {
      respond: vi.fn(async () => ({
        text: '[warm] I hear you. [emphasis]One step at a time.',
        source: 'model' as const,
        latency_ms: 42,
        spent_paise: 7,
      })),
    };
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
      staticStep('unused'),
      0,
      conversation,
    );

    await runtime.start();
    const reply = await runtime.acceptAudio(bytes('I had a rough morning'));

    expect(reply.speech?.text).toBe('[warm] I hear you. [emphasis]One step at a time.');
    expect(reply.meta.source).toBe('model');
    expect(conversation.respond).toHaveBeenCalledWith(expect.objectContaining({
      text: 'I had a rough morning',
      tenant_id: 't1',
      session_id: 's1',
    }));
  });

  it('answers a greeting during intake locally without calling the atomizer', async () => {
    let atomizerCalls = 0;
    const atomizer: AtomizerPort = {
      async atomize() {
        atomizerCalls += 1;
        return staticStep('This must not run').atomize({
          tenant_id: 't1',
          user_id: 'u1',
          session_id: 's1',
          task: 'hello',
        });
      },
    };
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, atomizer);

    await runtime.start();
    const reply = await runtime.acceptAudio(bytes('hola'));

    expect(reply.meta.source).toBe('reuse');
    expect(reply.speech?.text).toBe("Hi, Rachit. I'm here with you.");
    expect(reply.session.state).toBe('INTAKE');
    expect(atomizerCalls).toBe(0);
  });

  it('varies fixed local presence replies by the injected session seed', async () => {
    const input = { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' } as const;
    const first = createT0FocusSession(input, staticStep('unused'), 0);
    const second = createT0FocusSession(input, staticStep('unused'), 1);

    await first.start();
    await second.start();
    const firstReply = await first.acceptAudio(bytes('are you there'));
    const secondReply = await second.acceptAudio(bytes('are you there'));

    expect(firstReply.speech?.text).not.toBe(secondReply.speech?.text);
    expect(firstReply.meta.source).toBe('reuse');
    expect(secondReply.meta.source).toBe('reuse');
  });

  it('produces bounded varied presence after launch instead of waiting for a task', async () => {
    const input = { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' } as const;
    const runtime = createT0FocusSession(input, staticStep('unused'));
    await runtime.start();

    const first = await runtime.proactivePresence();
    const second = await runtime.proactivePresence();

    expect(first?.session.state).toBe('INTAKE');
    expect(first?.speech?.text).toContain('listening');
    expect(second?.speech?.text).not.toBe(first?.speech?.text);
    expect(first?.meta.source).toBe('reuse');
  });

  it('keeps a real task containing a greeting word on the atomizer path', async () => {
    const tasks: string[] = [];
    const atomizer: AtomizerPort = {
      async atomize(input) {
        tasks.push(input.task);
        return staticStep('Say hello to the team').atomize(input);
      },
    };
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, atomizer);

    await runtime.start();
    const reply = await runtime.acceptAudio(bytes('I need to say hello to the team in slack, just one message'));

    expect(reply.session.state).toBe('STEP_PRESENT');
    expect(tasks).toEqual(['I need to say hello to the team in slack, just one message']);
  });

  it('atomizes a new task request during WORKING instead of replying with chitchat', async () => {
    const tasks: string[] = [];
    const atomizer: AtomizerPort = {
      async atomize(input) {
        tasks.push(input.task);
        const step = input.task.includes('study') ? 'Open the study plan' : 'Open the tax portal';
        return staticStep(step).atomize(input);
      },
    };
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, atomizer);

    await runtime.start();
    await runtime.acceptAudio(bytes('just open the tax portal'));
    await runtime.speakCurrentStep();

    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes("I'm looking to study right now.")))).toMatchObject({
      stateLabel: 'STEP PRESENT',
      speechText: 'Start with: Open the study plan',
      stepLabel: '1/1 Open the study plan',
    });
    expect(tasks).toEqual(['just open the tax portal', "I'm looking to study right now."]);
  });

  it('preserves code-mixed Indic transcripts before handing them to the atomizer port', async () => {
    let observedTask = '';
    const atomizer: AtomizerPort = {
      async atomize(input: AtomizerPortInput) {
        observedTask = input.task;
        return staticStep('मुझे टैक्स भरना है').atomize(input);
      },
    };
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, atomizer);

    await runtime.start();
    expect(screenModelFromEnvelope(await runtime.acceptAudio(utf8Encode('मुझे टैक्स भरना है')))).toMatchObject({
      stateLabel: 'STEP PRESENT',
      stepLabel: '1/1 मुझे टैक्स भरना है',
    });
    expect(observedTask).toBe('मुझे टैक्स भरना है');
  });

  it('does not turn broad raw transcript text into a spoken step on the T0 fallback path', async () => {
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    });

    await runtime.start();
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('Do my taxes and then email Sam')))).toMatchObject({
      stateLabel: 'CLARIFY',
      speechText: "Let's take the smallest slice of that — I'd start with just the first bit. Want that, or a different part?",
    });
  });

  it('asks at most two clarify questions before atomizing a vague intake', async () => {
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, staticStep('Open the tax portal'));

    await runtime.start();
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('taxes')))).toMatchObject({
      stateLabel: 'CLARIFY',
      speechText: "Let's take the smallest slice of that — I'd start with just the first bit. Want that, or a different part?",
    });
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('just one part')))).toMatchObject({
      stateLabel: 'CLARIFY',
      speechText: 'Want to do this at your computer, or on your phone?',
    });
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('portal')))).toMatchObject({
      stateLabel: 'STEP PRESENT',
      stepLabel: '1/1 Open the tax portal',
    });
  });

  it('reuses a prior context-pack task without calling the atomizer', async () => {
    const atomizer: AtomizerPort = {
      async atomize() {
        throw new Error('context-pack reuse should not call the atomizer');
      },
    };
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
      context_pack: {
        profile: { user_id: 'u1' },
        open_loops: [],
        open_session: null,
        recent_tasks: [
          {
            task_text: 'just open the tax portal',
            embedding: [1],
            steps: {
              steps: [{ step_text: 'Open the saved tax portal link', est_min: 1, done_signal: 'portal is open' }],
              steps_total: 1,
            },
          },
        ],
      },
    }, atomizer);

    await runtime.start();
    const envelope = await runtime.acceptAudio(bytes('just open the tax portal'));
    expect(screenModelFromEnvelope(envelope)).toMatchObject({
      stateLabel: 'STEP PRESENT',
      stepLabel: '1/1 Open the saved tax portal link',
    });
    expect(envelope.meta.source).toBe('reuse');
  });

  it('feeds stuck evidence into policy so CHECK_IN is reachable through the FSM', async () => {
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, staticStep('Open the tax portal'));

    await runtime.start();
    await runtime.acceptAudio(bytes('just open the tax portal'));
    await runtime.speakCurrentStep();
    expect(screenModelFromEnvelope(await runtime.acceptAudio(bytes('I am stuck')))).toMatchObject({
      stateLabel: 'CHECK IN',
      stepLabel: '1/1 Open the tax portal',
      speechText: 'Stay with this: Open the tax portal',
    });
  });

  it('uses the current plan rather than the stuck utterance when reatomizing', async () => {
    const tasks: string[] = [];
    const atomizer: AtomizerPort = {
      async atomize(input: AtomizerPortInput) {
        tasks.push(input.task);
        return {
          output: {
            steps: [
              {
                step_text: input.task === 'just open the tax portal' ? 'Open the tax portal' : 'Open a smaller view',
                est_min: 1,
                done_signal: 'visible',
              },
            ],
            steps_total: 1,
          },
          envelope_source: 'model',
          atomizer_source: 'model',
          spent_paise: 1,
          latency_ms: 1,
        };
      },
    };
    const runtime = createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'ignored seed task',
    }, atomizer);

    await runtime.start();
    await runtime.acceptAudio(bytes('just open the tax portal'));
    await runtime.speakCurrentStep();
    await runtime.acceptAudio(bytes('I am stuck'));
    const secondStuck = screenModelFromEnvelope(await runtime.acceptAudio(bytes('I am stuck')));

    expect(tasks).toEqual(['just open the tax portal', 'Open the tax portal']);
    expect(secondStuck).toMatchObject({
      stateLabel: 'STEP PRESENT',
      stepLabel: '1/1 Open a smaller view',
      speechText: 'Try this smaller step: Open a smaller view',
    });
  });

  describe('checkIn — proactive re-engagement from real elapsed time', () => {
    it('is a no-op outside WORKING', async () => {
      const runtime = createT0FocusSession({
        tenant_id: 't1',
        user_id: 'u1',
        session_id: 's1',
        task: 'ignored seed task',
      }, staticStep('Open the tax portal'));

      await runtime.start();
      expect(await runtime.checkIn(1_000)).toBeNull();
      await runtime.acceptAudio(bytes('just open the tax portal'));
      expect(await runtime.checkIn(2_000)).toBeNull();
    });

    it('anchors silently on the first tick, then fires from real elapsed silence — regression: this used to be hardcoded to 0ms elapsed and could never fire', async () => {
      const runtime = createT0FocusSession({
        tenant_id: 't1',
        user_id: 'u1',
        session_id: 's1',
        task: 'ignored seed task',
      }, staticStep('Open the tax portal'));

      await runtime.start();
      await runtime.acceptAudio(bytes('just open the tax portal'));
      await runtime.speakCurrentStep();
      // Raises Execution belief confidence past the guard's 0.5 threshold without tripping the
      // reactive 'stuck' router path (classifySafeIntent explicitly routes "not done yet" to the
      // non-advancing fallback intent, so this stays in WORKING — see classifySafeIntent).
      await runtime.acceptAudio(bytes('not done yet'));
      await runtime.acceptAudio(bytes('still not done yet'));

      const t0 = 1_700_000_000_000;
      const firstTick = await runtime.checkIn(t0);
      expect(firstTick).toBeNull();

      const stillTooSoon = await runtime.checkIn(t0 + 1_000);
      expect(stillTooSoon).toBeNull();

      // Past the 3-minute execution-stall threshold (cognitive/Policy.ts EXECUTION_STALL_MS).
      const afterRealSilence = await runtime.checkIn(t0 + 3 * 60_000 + 1);
      expect(afterRealSilence).not.toBeNull();
      expect(screenModelFromEnvelope(afterRealSilence!)).toMatchObject({ stateLabel: 'CHECK IN' });
      expect(afterRealSilence!.speech?.text).toMatch(/break it down|smaller/i);
    });
  });
});

/**
 * ORB-ACCEPTANCE-CONTRACT Track B (B1-B5) — runtime-level integration proof. `IntentClassifier.test.ts`
 * already proves `classifyIntakeUtterance`/`isLikelyTaskRequest` as pure functions; this file proves
 * they are actually WIRED into `T0FocusSession.acceptAudio`'s production call path (AGENTS.md L8 bar:
 * "a module without a production caller is a contract or scaffold, not a completed feature").
 */
describe('ORB-ACCEPTANCE-CONTRACT Track B — intake open-domain routing (B1-B5)', () => {
  function freshRuntime(atomizer: AtomizerPort, conversation: ConversationPort) {
    return createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
      atomizer,
      0,
      conversation,
    );
  }

  function noAtomizeAllowed(label: string): AtomizerPort {
    return {
      async atomize() {
        throw new Error(`${label}: the atomizer must not be called for this utterance`);
      },
    };
  }

  it('B1: "teach me about photosynthesis" as the FIRST utterance at INTAKE reaches conversation, not clarify/atomize', async () => {
    const conversation = conversationEcho();
    const runtime = freshRuntime(noAtomizeAllowed('B1'), conversation);

    await runtime.start();
    const reply = await runtime.acceptAudio(bytes('teach me about photosynthesis'));

    // Before the fix: T0FocusSession.ts's isConversationalIntake regex
    // (`i am|i'm|i feel|i had|i was|i think|how are|what is|what's|can you|could you|do you`) did
    // not match this sentence, so it fell through to handleIntakeTranscript -> decideClarify ->
    // atomize. This is the exact bug ORB-ACCEPTANCE-CONTRACT B1 names.
    expect(reply.session.state).toBe('INTAKE'); // never advanced into CLARIFY/STEP_PRESENT
    expect(reply.speech?.text).toBe('[echo] teach me about photosynthesis');
    expect(reply.mode).toBe('teach');
    expect(conversation.respond).toHaveBeenCalledWith(
      expect.objectContaining({ text: 'teach me about photosynthesis', session_id: 's1' }),
    );
  });

  /** Mirrors IntentClassifier.test.ts's OPEN_DOMAIN_TABLE — see that file for the per-category trace. */
  const OPEN_DOMAIN_TABLE: ReadonlyArray<{ readonly category: string; readonly utterance: string }> = [
    { category: 'teach request', utterance: 'teach me about photosynthesis' },
    { category: 'venting/emotional disclosure', utterance: "nothing is going right for me lately and it's exhausting" },
    { category: 'opinion request', utterance: 'honestly, your take on remote work would be interesting to hear' },
    { category: 'bare greeting', utterance: 'hey' },
    { category: 'meta question about the orb', utterance: 'wait, are you a real person or a robot?' },
    { category: 'off-topic factual question', utterance: 'curious who actually invented the light bulb' },
    { category: 'ambiguous fragment', utterance: 'just thinking out loud, not sure really' },
    { category: 'Hinglish/code-mixed', utterance: 'yaar mujhe thoda samajh nahi aa raha, kya karu' },
    {
      category: 'very long rambling utterance',
      utterance:
        "So I woke up this morning and I couldn't really decide what I wanted to do with my day, and I " +
        "started thinking about this project I've been putting off, and then I got distracted thinking " +
        'about a conversation I had with a friend last week, and now I am just sitting here not really ' +
        'sure where to even begin or what any of this means for what I should focus on next.',
    },
    { category: 'single word', utterance: 'wow' },
    { category: 'question with no "?"', utterance: 'I was wondering how airplanes actually stay in the air' },
    { category: 'statement about a feeling', utterance: "I'm kind of nervous about tomorrow" },
    {
      // Coordinator-mandated addition. Exercises this classifier's routing only — the separate,
      // real endpointing bug (`voice/SemanticEndpointer.ts`'s CONTINUATION_MARKERS is English-only,
      // so a Hindi speaker trailing off on "और"/"aur" gets cut off before a transcript like this
      // ever reaches acceptAudio) is out of this track's owned files; flagged separately.
      category: 'Hindi (Hinglish-script) utterance with a trailing conjunction',
      utterance: 'yaar aaj ka din bilkul accha nahi tha, aur',
    },
  ] as const;

  it(
    `B2 (runtime wiring, published denominator): >= N-1 of ${OPEN_DOMAIN_TABLE.length} open-domain ` +
      'first utterances reach the conversation port at a fresh INTAKE, never the atomizer',
    async () => {
      const misses: string[] = [];
      for (const { category, utterance } of OPEN_DOMAIN_TABLE) {
        let atomizerCalls = 0;
        const atomizer: AtomizerPort = {
          async atomize(input) {
            atomizerCalls += 1;
            return staticStep('must not run').atomize(input);
          },
        };
        const conversation = conversationEcho();
        const runtime = freshRuntime(atomizer, conversation);
        await runtime.start();
        const reply = await runtime.acceptAudio(bytes(utterance));
        const ok = atomizerCalls === 0 && reply.session.state === 'INTAKE' && (reply.mode === 'converse' || reply.mode === 'teach');
        if (!ok) {
          misses.push(`${category} (${JSON.stringify(utterance)}) -> state=${reply.session.state} mode=${String(reply.mode)} atomizerCalls=${atomizerCalls}`);
        }
      }
      const passed = OPEN_DOMAIN_TABLE.length - misses.length;
      const missDescription = misses.length === 0 ? 'none' : misses.join('; ');
      // Published denominator — printed unconditionally, not only on failure.
      console.log(`[B2-runtime] ${passed} of ${OPEN_DOMAIN_TABLE.length} reached conversation without touching the atomizer. Misses: ${missDescription}`);
      expect(passed, missDescription).toBeGreaterThanOrEqual(OPEN_DOMAIN_TABLE.length - 1);
    },
  );

  /**
   * B3 (regression — "the one that matters most"): zero tolerance. Mirrors
   * IntentClassifier.test.ts's TASK_TABLE, including the three coordinator-flagged conjugated-verb
   * cases the Lemmatizer.ts fix targets.
   */
  const TASK_TABLE: ReadonlyArray<{ readonly note: string; readonly utterance: string }> = [
    { note: 'plain imperative', utterance: 'write the quarterly report' },
    { note: 'plain imperative', utterance: 'clean the kitchen' },
    { note: 'plain imperative', utterance: 'fix the login bug' },
    { note: 'plain imperative with object clause', utterance: 'email Priya about the invoice' },
    { note: 'conjugated verb -ed (coordinator-flagged, Lemmatizer.ts fix)', utterance: 'Opened the file already' },
    { note: 'conjugated verb -ing (coordinator-flagged, Lemmatizer.ts fix)', utterance: 'Sending the email now' },
    { note: 'conjugated verb -ing (coordinator-flagged, Lemmatizer.ts fix)', utterance: 'paying the electricity bill' },
    { note: 'task-prefix phrasing', utterance: 'I need to send an email' },
    { note: 'plain imperative', utterance: 'book a dentist appointment for next week' },
    { note: 'plain imperative, compound', utterance: 'download the tax form and submit it' },
  ] as const;

  it(
    `B3 (runtime wiring, regression, published denominator): all ${TASK_TABLE.length} task ` +
      'utterances still reach the atomizer/clarify path at a fresh INTAKE, never the conversation port',
    async () => {
      const misses: string[] = [];
      for (const { note, utterance } of TASK_TABLE) {
        const conversation: ConversationPort = {
          respond: vi.fn(async () => ({
            text: 'MUST NOT BE SPOKEN — conversation port fired for a task utterance',
            source: 'model' as const,
            latency_ms: 0,
            spent_paise: 0,
          })),
        };
        const runtime = freshRuntime(staticStep('a validated step'), conversation);
        await runtime.start();
        const reply = await runtime.acceptAudio(bytes(utterance));
        const respondCalls = (conversation.respond as ReturnType<typeof vi.fn>).mock.calls.length;
        const ok = (reply.session.state === 'STEP_PRESENT' || reply.session.state === 'CLARIFY') && respondCalls === 0;
        if (!ok) misses.push(`${note} (${JSON.stringify(utterance)}) -> state=${reply.session.state} conversationCalls=${respondCalls}`);
      }
      const missDescription = misses.length === 0 ? 'none' : misses.join('; ');
      console.log(`[B3-runtime] ${TASK_TABLE.length - misses.length} of ${TASK_TABLE.length} still reached atomizer/clarify. Misses: ${missDescription}`);
      expect(misses, missDescription).toEqual([]);
    },
  );

  it('B4: mode is explicit on every envelope and matches the pipeline that produced the turn (focus path)', async () => {
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
      staticStep('Open the tax portal'),
    );
    expect((await runtime.start()).mode).toBe('focus');
    expect((await runtime.acceptAudio(bytes('just open the tax portal'))).mode).toBe('focus');
    expect((await runtime.speakCurrentStep()).mode).toBe('focus');
  });

  it('B4: a mid-session teach-shaped question is tagged mode "teach", distinct from plain "converse"', async () => {
    const conversation = conversationEcho();
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
      staticStep('Open the tax portal'),
      0,
      conversation,
    );
    await runtime.start();
    await runtime.acceptAudio(bytes('just open the tax portal'));
    await runtime.speakCurrentStep();

    const teachReply = await runtime.acceptAudio(bytes('can you explain how compound interest works'));
    expect(teachReply.mode).toBe('teach');

    const chitchatReply = await runtime.acceptAudio(bytes('haha okay that makes sense, by the way this is fun'));
    expect(chitchatReply.mode).toBe('converse');
  });

  describe('B5 — unenumerated inputs, each explicitly handled, never a silently-wrong route', () => {
    it('empty string transcript: a bounded repair prompt, not a clarify question about nothing', async () => {
      const runtime = createT0FocusSession(
        { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
        staticStep('unused'),
      );
      await runtime.start();
      const reply = await runtime.acceptAudio(bytes(''));
      expect(reply.speech?.text).toBe("I couldn't make that out clearly. Please say it again.");
      expect(reply.session.state).toBe('INTAKE');
      expect(reply.mode).toBe('focus');
    });

    it('whitespace-only transcript: handled identically to empty', async () => {
      const runtime = createT0FocusSession(
        { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
        staticStep('unused'),
      );
      await runtime.start();
      const reply = await runtime.acceptAudio(bytes('   \n\t  '));
      expect(reply.speech?.text).toBe("I couldn't make that out clearly. Please say it again.");
    });

    it('null/undefined audio bytes degrade to the same explicit repair path instead of throwing', async () => {
      const runtime = createT0FocusSession(
        { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
        staticStep('unused'),
      );
      await runtime.start();
      // A native-bridge/JSON boundary can hand this null/undefined at runtime despite the
      // `Uint8Array` parameter type — the cast simulates exactly that; the assertion is that this
      // does not throw and instead reaches the same explicit repair path as an empty transcript.
      await expect(runtime.acceptAudio(null as unknown as Uint8Array)).resolves.toMatchObject({
        speech: { text: "I couldn't make that out clearly. Please say it again." },
      });
      await expect(runtime.acceptAudio(undefined as unknown as Uint8Array)).resolves.toMatchObject({
        speech: { text: "I couldn't make that out clearly. Please say it again." },
      });
    });

    it('a >10,000-character utterance is bounded before forwarding, not crashed on and not sent unbounded', async () => {
      const longText = 'teach me about photosynthesis in incredible detail '.repeat(200);
      expect(longText.length).toBeGreaterThan(10_000);
      const conversation = conversationEcho();
      const runtime = createT0FocusSession(
        { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
        staticStep('unused'),
        0,
        conversation,
      );
      await runtime.start();
      const reply = await runtime.acceptAudio(bytes(longText));
      expect(reply.mode).toBe('teach'); // the teach marker is at the very start, well inside the bound
      const [sentInput] = (conversation.respond as ReturnType<typeof vi.fn>).mock.calls[0] as [{ text: string }];
      expect(sentInput.text.length).toBeLessThanOrEqual(4_000);
    });

    it('duplicate identical consecutive utterances at INTAKE: same route both times, no state drift', async () => {
      const conversation = conversationEcho();
      const runtime = createT0FocusSession(
        { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
        staticStep('unused'),
        0,
        conversation,
      );
      await runtime.start();
      const first = await runtime.acceptAudio(bytes('teach me about photosynthesis'));
      const second = await runtime.acceptAudio(bytes('teach me about photosynthesis'));
      expect(first.mode).toBe('teach');
      expect(second.mode).toBe('teach');
      expect(first.session.state).toBe('INTAKE');
      expect(second.session.state).toBe('INTAKE'); // never drifted into CLARIFY from the repeat
      expect(conversation.respond).toHaveBeenCalledTimes(2);
    });

    it('a unicode/emoji-only transcript is handled explicitly, never atomized as literal task text', async () => {
      const atomizer = noAtomizeAllowed('B5-emoji');
      const runtime = createT0FocusSession(
        { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
        atomizer,
      );
      await runtime.start();
      const reply = await runtime.acceptAudio(bytes('🎉🎉🎉'));
      expect(reply.speech?.text).toBe("I couldn't make that out clearly. Please say it again.");
      expect(reply.session.state).toBe('INTAKE');
    });

    it('a leading/trailing-punctuation-only transcript is handled explicitly, never atomized as literal task text', async () => {
      const atomizer = noAtomizeAllowed('B5-punctuation');
      const runtime = createT0FocusSession(
        { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
        atomizer,
      );
      await runtime.start();
      const reply = await runtime.acceptAudio(bytes('...'));
      expect(reply.speech?.text).toBe("I couldn't make that out clearly. Please say it again.");
    });

    it('leading/trailing punctuation wrapped AROUND real content is not swallowed as "punctuation-only"', async () => {
      const conversation = conversationEcho();
      const runtime = createT0FocusSession(
        { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' },
        staticStep('unused'),
        0,
        conversation,
      );
      await runtime.start();
      const reply = await runtime.acceptAudio(bytes('"teach me about photosynthesis"'));
      expect(reply.mode).toBe('teach');
    });
  });
});
