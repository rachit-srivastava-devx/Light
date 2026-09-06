import { describe, expect, it, vi } from 'vitest';

import {
  createFocusOrbActions,
  LOCAL_MVP_GREETING,
  requestSessionWarmup,
  type SessionWarmup,
  type WarmupFetcher,
} from './AppController';
import type { ContextPack } from './lld/ContextPack';
import { IDLE_ENVELOPE } from './AppModel';
import type { T0FocusSessionRuntime } from './runtime/T0FocusSession';
import type { SpeechPort } from './voice/SpeechPort';

const identity = { tenant_id: 't0', user_id: 'local-user', session_id: 'local-session' } as const;

function fakeRuntime(): T0FocusSessionRuntime & {
  readonly acceptAudio: ReturnType<typeof vi.fn>;
  readonly setContextPack: ReturnType<typeof vi.fn>;
} {
  return {
    identity: () => identity,
    setContextPack: vi.fn(),
    start: vi.fn(async () => IDLE_ENVELOPE),
    acceptAudio: vi.fn(async () => IDLE_ENVELOPE),
    speakCurrentStep: vi.fn(async () => IDLE_ENVELOPE),
    pause: vi.fn(async () => IDLE_ENVELOPE),
    checkIn: vi.fn(async () => null),
    proactivePresence: vi.fn(async () => null),
  };
}

describe('FocusOrb launch/real-turn contract', () => {
  it('does not call the atomizer path on startup and speaks the greeting once', async () => {
    const runtime = fakeRuntime();
    const spoken: string[] = [];
    const speaker: SpeechPort = {
      speak: vi.fn((text) => {
        spoken.push(text);
      }),
    };
    const actions = createFocusOrbActions(runtime, vi.fn(), { speaker });

    await actions.start();
    await actions.start();

    expect(runtime.acceptAudio).not.toHaveBeenCalled();
    expect(spoken).toEqual([LOCAL_MVP_GREETING]);
    expect(speaker.speak).toHaveBeenCalledTimes(1);
  });

  it('runs warmup before runtime launch and feeds the fetched context into the runtime', async () => {
    const runtime = fakeRuntime();
    const contextPack: ContextPack = {
      profile: { user_id: identity.user_id },
      recent_tasks: [],
      open_loops: ['finish the draft'],
      open_session: null,
    };
    const order: string[] = [];
    const warmup: SessionWarmup = async (receivedIdentity) => {
      order.push(`warmup:${receivedIdentity.tenant_id}:${receivedIdentity.user_id}:${receivedIdentity.session_id}`);
      return contextPack;
    };
    const start = runtime.start as ReturnType<typeof vi.fn>;
    start.mockImplementation(async () => {
      order.push('runtime.start');
      return IDLE_ENVELOPE;
    });

    await createFocusOrbActions(runtime, vi.fn(), { warmup }).start();

    expect(order).toEqual(['warmup:t0:local-user:local-session', 'runtime.start']);
    expect(runtime.setContextPack).toHaveBeenCalledWith(contextPack);
  });

  it('speaks a backend failure and still opens the listening loop when warmup fails', async () => {
    const runtime = fakeRuntime();
    const spoken: string[] = [];
    const speaker: SpeechPort = {
      speak: vi.fn((text) => {
        spoken.push(text);
      }),
    };

    await createFocusOrbActions(runtime, vi.fn(), {
      speaker,
      warmup: async () => {
        throw new Error('relay unavailable');
      },
    }).start();

    expect(runtime.start).toHaveBeenCalledTimes(1);
    expect(spoken).toContain('I’m having trouble with my voice connection, but I’m still here. Please try again.');
  });

  it('POSTs the unchanged T0 identity to /v1/session/warmup and maps the context pack', async () => {
    const requests: Array<{ url: string; body: string }> = [];
    const fetcher: WarmupFetcher = vi.fn(async (url, init) => {
      requests.push({ url, body: init.body });
      return {
        ok: true,
        status: 200,
        json: async () => ({
          profile: { tenant_id: identity.tenant_id, user_id: identity.user_id },
          recent_tasks: [],
          open_loops: ['reply to the email'],
          embeddings: [],
          open_session: { tenant_id: identity.tenant_id, session_id: identity.session_id },
          phrase_manifest: { 'presence.here.v1': "I'm here." },
        }),
      };
    });

    await expect(requestSessionWarmup('http://relay.test/', identity, fetcher)).resolves.toEqual({
      profile: { user_id: 'local-user' },
      recent_tasks: [],
      open_loops: ['reply to the email'],
      open_session: { session_id: 'local-session' },
    });
    expect(requests).toEqual([
      {
        url: 'http://relay.test/v1/session/warmup',
        body: JSON.stringify(identity),
      },
    ]);
  });
});
