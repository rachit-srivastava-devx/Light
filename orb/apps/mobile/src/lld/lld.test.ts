import { describe, expect, it } from 'vitest';

import {
  EMPTY_SLOTS,
  MAX_CLARIFY_QUESTIONS,
  decideClarify,
  extractSlots,
} from './ClarifyProtocol';
import {
  COSINE_SEED_THRESHOLD,
  type ContextPack,
  type RecentTask,
  cosineInt8,
  lexicalScore,
  retrieve,
} from './ContextPack';
import { pinVoiceForSession, selectVoice } from './VoiceRouter';

describe('ClarifyProtocol', () => {
  it('asks for scope first on a bare brain-dump', () => {
    const d = decideClarify(EMPTY_SLOTS, 0);
    expect(d).toEqual({ kind: 'ask', slot: 'scope' });
  });

  it('never asks for something the utterance already said', () => {
    const slots = extractSlots('just open the tax portal in 10 minutes');
    expect(slots.scope).not.toBeNull();
    expect(slots.first_context).not.toBeNull();
    expect(slots.time_box).not.toBeNull();
    expect(decideClarify(slots, 0)).toEqual({ kind: 'proceed', reason: 'slots_filled' });
  });

  it('proceeds at the question cap rather than interrogating', () => {
    // Proceeding on partial info beats a third question to someone who came here to start.
    const d = decideClarify(EMPTY_SLOTS, MAX_CLARIFY_QUESTIONS);
    expect(d).toEqual({ kind: 'proceed', reason: 'question_cap_reached' });
  });

  it('does not overwrite a slot already filled by an earlier turn', () => {
    const first = extractSlots('just one thing');
    const second = extractSlots('actually a few things', first);
    expect(second.scope).toBe(first.scope);
  });

  it('asks at most two questions across an intake', () => {
    let asked = 0;
    let slots = EMPTY_SLOTS;
    for (let turn = 0; turn < 5; turn++) {
      const d = decideClarify(slots, asked);
      if (d.kind === 'proceed') break;
      asked += 1;
      slots = extractSlots('mm', slots); // an answer that fills nothing
    }
    expect(asked).toBeLessThanOrEqual(MAX_CLARIFY_QUESTIONS);
  });
});

function task(text: string, embedding: number[]): RecentTask {
  return {
    task_text: text,
    steps: { steps: [{ step_text: 'x', est_min: 1, done_signal: 'y' }], steps_total: 1 },
    embedding,
  };
}

describe('ContextPack retrieval', () => {
  const pack = (tasks: RecentTask[]): ContextPack => ({
    profile: { user_id: 'u1' },
    recent_tasks: tasks,
    open_loops: [],
    open_session: null,
  });

  it('reuses a near-identical previous task without a model call', () => {
    const p = pack([task('file my taxes', [1, 0, 0])]);
    const r = retrieve(p, 'file my taxes', [1, 0, 0]);
    expect(r.kind).toBe('reuse');
  });

  it('seeds from a semantically similar task when lexical overlap is weak', () => {
    const p = pack([task('submit the annual return', [1, 0, 0])]);
    const r = retrieve(p, 'do my tax paperwork', [1, 0, 0]);
    expect(r.kind).toBe('seed');
    if (r.kind === 'seed') expect(r.score).toBeGreaterThanOrEqual(COSINE_SEED_THRESHOLD);
  });

  it('falls back to a cold atomize when nothing matches', () => {
    const p = pack([task('walk the dog', [0, 1, 0])]);
    expect(retrieve(p, 'refactor the parser', [1, 0, 0]).kind).toBe('cold');
  });

  it('is cold on an empty pack rather than throwing', () => {
    expect(retrieve(pack([]), 'anything', [1, 0, 0]).kind).toBe('cold');
  });

  it('scores lexical overlap symmetrically', () => {
    expect(lexicalScore('file my taxes', 'my taxes file')).toBe(1);
    expect(lexicalScore('a b', 'b a')).toBe(lexicalScore('b a', 'a b'));
  });

  it('returns 0, not NaN, for a corrupt or zero embedding', () => {
    // A NaN score compares false against every threshold, silently disabling retrieval.
    expect(cosineInt8([0, 0, 0], [1, 2, 3])).toBe(0);
    expect(cosineInt8([], [])).toBe(0);
    expect(cosineInt8([1, 2], [1, 2, 3])).toBe(0);
  });

  it('does not match a corrupt-embedding task by cosine', () => {
    const p = pack([task('unrelated', [0, 0, 0])]);
    expect(retrieve(p, 'something else', [1, 0, 0]).kind).toBe('cold');
  });
});

describe('VoiceRouter', () => {
  it('routes India/Hinglish sessions to the India voice provider', () => {
    expect(selectVoice('IN', 'en-IN')).toMatchObject({
      provider: 'sarvam',
      voice_id: 'orb.warm.hinglish.v1',
    });
  });

  it('uses the global warm voice elsewhere', () => {
    expect(selectVoice('US', 'en-US')).toMatchObject({
      provider: 'fish',
      voice_id: 'orb.warm.v1',
    });
  });

  it('pins an existing session voice rather than re-routing mid-session', () => {
    const first = selectVoice('US', 'en-US');
    expect(pinVoiceForSession(first, 'IN', 'en-IN')).toBe(first);
  });
});
