import { describe, expect, it } from 'vitest';

import { createRelayAtomizerPort } from './AtomizerPort';

const input = {
  tenant_id: 't1',
  user_id: 'u1',
  session_id: 's1',
  task: 'file my taxes',
};

describe('AtomizerPort', () => {
  it('calls the relay atomizer contract and maps repaired model output to envelope source model', async () => {
    const calls: unknown[] = [];
    const fetchImpl: typeof fetch = async (_url, init) => {
      calls.push(JSON.parse(String(init?.body)));
      return new Response(
        JSON.stringify({
          source: 'model_repaired',
          spent_paise: 9,
          output: {
            steps: [{ step_text: 'Open the tax portal', est_min: 1, done_signal: 'portal is visible' }],
            steps_total: 1,
          },
        }),
        { status: 200 },
      );
    };

    const result = await createRelayAtomizerPort({ baseUrl: 'http://relay.test', fetchImpl }).atomize(input);

    expect(calls).toEqual([input]);
    expect(result.output.steps[0]?.step_text).toBe('Open the tax portal');
    expect(result.envelope_source).toBe('model');
    expect(result.atomizer_source).toBe('model_repaired');
    expect(result.spent_paise).toBe(9);
  });

  it('falls back only when the relay is unreachable', async () => {
    const fetchImpl: typeof fetch = async () => {
      throw new TypeError('network down');
    };

    const result = await createRelayAtomizerPort({ baseUrl: 'http://relay.test', fetchImpl }).atomize(input);

    expect(result.output.steps[0]?.step_text).toBe(
      'Open the thing you need for this and look at it for one minute.',
    );
    expect(result.envelope_source).toBe('cache_hit');
    expect(result.failure_message).toContain("couldn't connect to the task planner");
  });

  it('speaks a backend failure when the relay returns a server error', async () => {
    const fetchImpl: typeof fetch = async () => new Response('provider unavailable', { status: 502 });

    const result = await createRelayAtomizerPort({ baseUrl: 'http://relay.test', fetchImpl }).atomize(input);

    expect(result.failure_message).toContain("couldn't reach the task planner");
    expect(result.output.steps[0]?.step_text).toBe(
      'Open the thing you need for this and look at it for one minute.',
    );
  });

  it('does not hide a budget refusal behind a local fallback', async () => {
    const fetchImpl: typeof fetch = async () => new Response('budget exhausted', { status: 402 });

    await expect(
      createRelayAtomizerPort({ baseUrl: 'http://relay.test', fetchImpl }).atomize(input),
    ).rejects.toThrow('Atomizer budget exhausted');
  });
});
