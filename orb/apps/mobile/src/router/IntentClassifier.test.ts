import { describe, expect, it } from 'vitest';
import {
  AmbiguousIntentNotWiredError,
  classifyByRule,
  classifyIntakeUtterance,
  classifyIntent,
  classifySocialUtterance,
  isLikelyTaskRequest,
  isTeachRequestUtterance,
  notYetWiredAmbiguousResolver,
} from './IntentClassifier';

describe('classifyByRule — the deterministic rule-first path', () => {
  it('classifies an unambiguous "done" utterance', () => {
    expect(classifyByRule('okay I finished that one')).toEqual({
      matched: true,
      label: 'done',
    });
  });

  it('classifies an unambiguous "next" utterance', () => {
    expect(classifyByRule('next please')).toEqual({ matched: true, label: 'next' });
  });

  it('classifies an unambiguous "pause" utterance', () => {
    expect(classifyByRule('can we pause for a sec')).toEqual({ matched: true, label: 'pause' });
  });

  it('classifies an unambiguous "stuck" utterance', () => {
    expect(classifyByRule("I'm stuck on this")).toEqual({ matched: true, label: 'stuck' });
  });

  it('classifies an unambiguous "question" utterance', () => {
    expect(classifyByRule('what do I do here')).toEqual({ matched: true, label: 'question' });
  });

  it('classifies an unambiguous "chitchat" utterance', () => {
    expect(classifyByRule('hey there, lol')).toEqual({ matched: true, label: 'chitchat' });
  });

  it('recognizes common greetings locally, including hola', () => {
    expect(classifyByRule('Hello!')).toEqual({ matched: true, label: 'chitchat' });
    expect(classifyByRule('hola')).toEqual({ matched: true, label: 'chitchat' });
    expect(classifySocialUtterance('How are you?')).toBe('presence');
  });

  it('is case-insensitive', () => {
    expect(classifyByRule('DONE')).toEqual({ matched: true, label: 'done' });
  });

  it('reports ambiguity (no candidate) for an utterance matching nothing', () => {
    const result = classifyByRule('the quick brown fox');
    expect(result.matched).toBe(false);
    if (!result.matched) expect(result.candidates).toEqual([]);
  });

  it('does not treat negated completion as explicit done intent', () => {
    const result = classifyByRule("I'm not done yet");
    expect(result.matched).toBe(false);
    if (!result.matched) expect(result.candidates).toEqual([]);
  });

  it('does not match route keywords inside unrelated words', () => {
    expect(classifyByRule('I am unstuck now')).toEqual({ matched: false, candidates: [] });
    expect(classifyByRule('however this works')).toEqual({ matched: false, candidates: [] });
  });

  it('uses rule order to resolve common mixed utterances deterministically', () => {
    expect(classifyByRule('done, next one')).toEqual({ matched: true, label: 'done' });
    expect(classifyByRule("I'm stuck, what do I do")).toEqual({ matched: true, label: 'stuck' });
  });

  it('does not treat question-shaped control phrases as explicit pause commands', () => {
    // B5 fix: a question-form-prefix utterance ("can I ...?") must still classify as `question`
    // rather than being short-circuited to an unmatched/empty result. Before the fix,
    // classifyByRule returned {matched:false, candidates:[]} here unconditionally — not just
    // "does not become pause" but "becomes nothing at all", which made classifyIntent() throw
    // AmbiguousIntentNotWiredError on ordinary question-shaped speech (see the dedicated test
    // below for the exact repro that motivated the fix).
    expect(classifyByRule('can I stop?')).toEqual({ matched: true, label: 'question' });
  });

  it('B5: a question-form-prefix utterance during an active session routes to question, not an empty/ambiguous match', () => {
    // Real defect repro: "Could you help me with my taxes?" is a question, but any utterance
    // starting with a question-form prefix ("can I", "could I", "should I", "do I", "am I allowed
    // to") and containing "?" used to short-circuit classifyByRule to {matched:false,
    // candidates:[]} BEFORE the 'question' rule (keyword "?") ever got a chance to fire — even
    // though the utterance is unambiguously a question. Feeding empty candidates to
    // classifyIntent's default resolver throws AmbiguousIntentNotWiredError, which violates the
    // "deterministic path must not throw on valid inputs" invariant (BUILD-DIGEST §4) for
    // something that should just route question -> fast_voice (BUILD-DIGEST §2).
    expect(classifyByRule('Could I get help with my taxes?')).toEqual({
      matched: true,
      label: 'question',
    });
    expect(classifyIntent('Could I get help with my taxes?')).toBe('question');
    expect(classifyByRule('Should I pause for now?')).toEqual({ matched: true, label: 'question' });
    expect(() => classifyIntent('Do I need to stop?')).not.toThrow();
    expect(classifyIntent('Do I need to stop?')).toBe('question');
  });

  it('recognizes a new task request instead of treating it as social speech', () => {
    expect(isLikelyTaskRequest("I'm looking to study right now.")).toBe(true);
    expect(isLikelyTaskRequest('I need to send an email')).toBe(true);
  });

  it('does not steal explicit controls or questions as new tasks', () => {
    expect(isLikelyTaskRequest("I'm stuck on this")).toBe(false);
    expect(isLikelyTaskRequest('what should I do?')).toBe(false);
    expect(isLikelyTaskRequest('hey there')).toBe(false);
  });

  it('keeps task sentences containing greeting words on the task path', () => {
    expect(isLikelyTaskRequest('I need to say hello to the team')).toBe(true);
    expect(classifySocialUtterance('hello can you help me with taxes')).toBeNull();
  });
});

describe('classifyIntent — rule-first, ambiguous path clearly marked not-yet-wired', () => {
  it('returns the rule-first label directly when unambiguous, without touching the resolver', () => {
    let resolverCalled = false;
    const result = classifyIntent('done', () => {
      resolverCalled = true;
      return 'done';
    });
    expect(result).toBe('done');
    expect(resolverCalled).toBe(false);
  });

  it('throws AmbiguousIntentNotWiredError by default on an ambiguous utterance (not silently wrong)', () => {
    expect(() => classifyIntent('the quick brown fox')).toThrow(AmbiguousIntentNotWiredError);
  });

  it('the default resolver is exported and independently throws the same marker', () => {
    expect(() => notYetWiredAmbiguousResolver('anything', ['question', 'stuck'])).toThrow(
      AmbiguousIntentNotWiredError,
    );
  });

  it('a supplied resolver IS invoked (and its result trusted) when no rule fires', () => {
    const result = classifyIntent('the quick brown fox', () => 'chitchat');
    expect(result).toBe('chitchat');
  });

  it('does not invoke the resolver after a first-rule match', () => {
    const result = classifyIntent("I'm stuck, what do I do", () => 'question');
    expect(result).toBe('stuck');
  });
});

describe('Lemmatizer.ts fix — conjugated verbs (coordinator-flagged, real shipped bugs)', () => {
  it('isLikelyTaskRequest recognizes a conjugated first-word task verb a bare Set lookup misses', () => {
    // TASK_VERBS.has('opened') was false even though .has('open') is true — three real utterances
    // that previously failed to be recognized as tasks, named in the mid-flight audit verbatim.
    expect(isLikelyTaskRequest('Opened the file already')).toBe(true);
    expect(isLikelyTaskRequest('Sending the email now')).toBe(true);
    expect(isLikelyTaskRequest('paying the electricity bill')).toBe(true);
  });

  it('containsKeyword (via classifyByRule) also lemma-matches a single-word rule keyword', () => {
    // "stop" is the `pause` rule's keyword; a bare whole-word regex cannot match "stopping".
    expect(classifyByRule('stopping for a sec')).toEqual({ matched: true, label: 'pause' });
  });

  it('is suffix-only: does not prefix-strip, so negating prefixes stay correctly excluded', () => {
    // Regression guard for the lemmatizer itself: "unstuck" must not become "stuck", and "taxes"
    // (not a verb) must not spuriously lemma into a TASK_VERBS member.
    expect(classifyByRule('I am unstuck now')).toEqual({ matched: false, candidates: [] });
    expect(isLikelyTaskRequest('taxes')).toBe(false);
  });
});

describe('classifyIntakeUtterance — ORB-ACCEPTANCE-CONTRACT Track B (B1-B3)', () => {
  it('B1: "teach me about photosynthesis" as a first utterance classifies as teach, never task', () => {
    // Before this track's fix: T0FocusSession.ts's isConversationalIntake regex
    // (`i am|i'm|i feel|i had|i was|i think|how are|what is|what's|can you|could you|do you`)
    // did not match this sentence, so it fell through to handleIntakeTranscript -> decideClarify
    // -> atomize, exactly the bug ORB-ACCEPTANCE-CONTRACT B1 names.
    expect(classifyIntakeUtterance('teach me about photosynthesis')).toBe('teach');
    expect(isTeachRequestUtterance('teach me about photosynthesis')).toBe(true);
  });

  /**
   * B2 — >=12 diverse open-domain FIRST utterances; the contract requires >=11 of 12 (generalized
   * here as >=(N-1) of N) to route to conversation. Every category the contract names is present.
   */
  const OPEN_DOMAIN_TABLE: ReadonlyArray<{ readonly category: string; readonly utterance: string }> = [
    { category: 'teach request', utterance: 'teach me about photosynthesis' },
    {
      category: 'venting/emotional disclosure',
      utterance: "nothing is going right for me lately and it's exhausting",
    },
    {
      category: 'opinion request',
      utterance: 'honestly, your take on remote work would be interesting to hear',
    },
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
      // Coordinator-mandated addition (mid-flight audit). Tests only the classifier's own routing —
      // NOT the separate, real, and more serious bug flagged out-of-scope: `voice/SemanticEndpointer.ts`'s
      // `CONTINUATION_MARKERS` is English-only, so a Hindi speaker trailing off on "और"/"aur" would be
      // endpointed (cut off) before this classifier ever runs. That module is not in this track's owned
      // files; see the track report.
      category: 'Hindi (Hinglish-script) utterance with a trailing conjunction',
      utterance: 'yaar aaj ka din bilkul accha nahi tha, aur',
    },
  ] as const;

  it.each(OPEN_DOMAIN_TABLE)('B2 row: "$utterance" ($category) routes to conversation', ({ utterance }) => {
    // Each row is also its own named test so a regression names the exact failing row; the
    // aggregate test right below is the actual >=(N-1)-of-N gate the contract specifies.
    const route = classifyIntakeUtterance(utterance);
    expect(['converse', 'teach']).toContain(route);
  });

  it(`B2 (aggregate, published denominator): >= N-1 of ${OPEN_DOMAIN_TABLE.length} open-domain first utterances route to conversation`, () => {
    const results = OPEN_DOMAIN_TABLE.map((row) => ({ ...row, route: classifyIntakeUtterance(row.utterance) }));
    const misses = results.filter((r) => r.route === 'task');
    const passed = results.length - misses.length;
    const missDescription =
      misses.length === 0 ? 'none' : misses.map((m) => `${m.category} (${JSON.stringify(m.utterance)})`).join('; ');
    // Published denominator — printed unconditionally, not only on failure.
    console.log(`[B2] open-domain routing: ${passed} of ${results.length} routed to conversation. Misses: ${missDescription}`);
    expect(passed, `misses: ${missDescription}`).toBeGreaterThanOrEqual(results.length - 1);
  });

  /**
   * B3 (regression — "the one that matters most"): task utterances must still reach the
   * atomizer/clarify path. Zero tolerance — unlike B2, this is not an "allow one miss" gate.
   * Includes the three coordinator-flagged conjugated-verb cases the lemmatizer fix targets.
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

  it.each(TASK_TABLE)('B3 row: "$utterance" ($note) still classifies as task, never conversation', ({ utterance }) => {
    expect(classifyIntakeUtterance(utterance)).toBe('task');
  });

  it(`B3 (aggregate, published denominator): all ${TASK_TABLE.length} task utterances still classify as task — zero tolerance`, () => {
    const misses = TASK_TABLE.filter(({ utterance }) => classifyIntakeUtterance(utterance) !== 'task');
    const missDescription =
      misses.length === 0 ? 'none' : misses.map((m) => `${m.note} (${JSON.stringify(m.utterance)})`).join('; ');
    console.log(`[B3] task-regression: ${TASK_TABLE.length - misses.length} of ${TASK_TABLE.length} still atomize. Misses: ${missDescription}`);
    expect(misses.map((m) => m.utterance), missDescription).toEqual([]);
  });
});
