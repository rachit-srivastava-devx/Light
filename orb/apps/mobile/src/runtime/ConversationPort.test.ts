import { describe, expect, it, vi } from 'vitest';

import { createRelayConversationPort } from './ConversationPort';

const input = {
  tenant_id: 't1',
  user_id: 'u1',
  session_id: 's1',
  task: 'ignored',
  text: 'I had a rough morning',
} as const;

describe('ConversationPort', () => {
  it('sends the final transcript to relay-py and preserves Fish markup', async () => {
    const fetchImpl = vi.fn(async () => new Response(JSON.stringify({
      text: '[warm] I hear you. [emphasis]One step.',
      source: 'model',
      latency_ms: 42,
      spent_paise: 7,
    }), { status: 200, headers: { 'content-type': 'application/json' } }));
    const port = createRelayConversationPort({ baseUrl: 'http://relay.test', fetchImpl });

    await expect(port.respond(input)).resolves.toMatchObject({
      text: '[warm] I hear you. [emphasis]One step.',
      source: 'model',
      latency_ms: 42,
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      'http://relay.test/v1/respond',
      expect.objectContaining({ body: expect.stringContaining('I had a rough morning') }),
    );
  });

  // Regression guard for a real defect (2026-08-28): the routing layer computed the response mode
  // and passed it ONLY to the local envelope, so `mode` never reached the request body. The relay
  // then applied its compatibility default (ResponseMode.FOCUS) on every turn, which made teach and
  // converse modes UNREACHABLE from the app even though their prompts, beat-chunking and 480-token
  // ceiling were fully built and unit-tested. 562 tests passed while the feature was dead, because
  // every one of them tested a component rather than the wire. These two assert the wire.
  // Captures the request body through a TYPED fetch parameter rather than indexing into
  // `fetchImpl.mock.calls`: a bare `vi.fn(async () => ...)` declares no parameters, so its call
  // tuple types as `[]` and `calls[0][1]` fails `tsc --noEmit` even though `vitest run` passes.
  // (Learned the hard way — the gate caught it, the test runner did not.)
  function captureBody() {
    const seen: { body?: Record<string, unknown> } = {};
    const fetchImpl = vi.fn(async (_url: string, init?: RequestInit) => {
      seen.body = JSON.parse(String(init?.body ?? '{}')) as Record<string, unknown>;
      return new Response(JSON.stringify({
        text: 'ok', source: 'model', latency_ms: 1, spent_paise: 0,
      }), { status: 200, headers: { 'content-type': 'application/json' } });
    });
    return { seen, fetchImpl: fetchImpl as unknown as typeof fetch };
  }

  it('transmits the response mode on the wire so teach/converse are reachable', async () => {
    const { seen, fetchImpl } = captureBody();
    const port = createRelayConversationPort({ baseUrl: 'http://relay.test', fetchImpl });

    await port.respond({ ...input, text: 'teach me about photosynthesis', mode: 'teach' });

    // Exact value, not just presence: the backend enum is "converse" | "focus" | "teach"
    // (proxy/schemas.py ResponseMode) and an unknown value is a typed 422, not a coercion.
    expect(seen.body?.mode).toBe('teach');
  });

  it('omits mode entirely when the caller has none, rather than sending null', async () => {
    const { seen, fetchImpl } = captureBody();
    const port = createRelayConversationPort({ baseUrl: 'http://relay.test', fetchImpl });

    await port.respond(input);

    // Omission is what triggers the relay's documented default; an explicit null/undefined would
    // be a validation error there.
    expect(seen.body && 'mode' in seen.body).toBe(false);
  });

  it('speaks a truthful local failure when relay-py is unavailable', async () => {
    const fetchImpl = vi.fn(async () => { throw new Error('connection refused'); });
    const port = createRelayConversationPort({ baseUrl: 'http://relay.test', fetchImpl });

    await expect(port.respond(input)).resolves.toMatchObject({
      source: 'cache_hit',
      failure_message: 'conversation_backend_unavailable',
      text: expect.stringContaining("couldn't reach"),
    });
  });
});
