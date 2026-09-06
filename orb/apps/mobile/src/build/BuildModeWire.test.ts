import { describe, expect, it, vi } from 'vitest';
import { createRelayConversationPort } from '../runtime/ConversationPort';

describe('build mode reaches the wire', () => {
  it("U4-T1 mode:'build' appears in the POST body, not just in local state", async () => {
    // The scar this guards: `mode` was computed everywhere and transmitted nowhere,
    // making teach/converse unreachable from the real app while fully unit-tested.
    const fetchImpl = vi.fn(async () => new Response(
      JSON.stringify({ text: 'ok', source: 'model', latency_ms: 1, spent_paise: 0 }),
      { status: 200, headers: { 'content-type': 'application/json' } }));
    const port = createRelayConversationPort({ baseUrl: 'http://relay.test', fetchImpl: fetchImpl as any });
    await port.respond({ session_id: 's', tenant_id: 't', user_id: 'u', task: 'build me a thing', text: 'build me a thing', mode: 'build' });
    const [, init] = fetchImpl.mock.calls[0] as unknown as [string, { body: string }];
    const body = JSON.parse(init.body);
    expect(body.mode).toBe('build');
  });

  it('U4-T2 an absent mode is omitted, not sent as null (relay would 422)', async () => {
    const fetchImpl = vi.fn(async () => new Response(
      JSON.stringify({ text: 'ok', source: 'model', latency_ms: 1, spent_paise: 0 }), { status: 200 }));
    const port = createRelayConversationPort({ baseUrl: 'http://relay.test', fetchImpl: fetchImpl as any });
    await port.respond({ session_id: 's', tenant_id: 't', user_id: 'u', task: 'hi', text: 'hi' });
    const [, init] = fetchImpl.mock.calls[0] as unknown as [string, { body: string }];
    expect(Object.prototype.hasOwnProperty.call(JSON.parse(init.body), 'mode')).toBe(false);
  });
});
