import { describe, expect, it, afterEach } from 'vitest';
import type { Server } from 'node:http';
import { createMemoryGateway } from '@pe/llm-gateway/adapters/memory';
import type { LlmGatewayPort } from '@pe/llm-gateway';
import { createGatewayServer } from '../src/index';

/**
 * E1 (Track E) — HTTP-level proof that `/v1/complete` itself, not just the pure predicate, makes
 * the fake/memory adapter detectable. This test runs the REAL `createMemoryGateway({})` from the
 * registry (no mock) through the REAL server construction this sidecar boots in production
 * (`export const server = createGatewayServer(gateway)` in `src/index.ts`), so a regression that
 * stops forwarding `adapter`/`is_fake_adapter` onto the wire — even if the pure predicate in
 * `adapterEvidence.ts` still passes its own unit tests — fails here.
 *
 * `ORB_LLM_GATEWAY_ADAPTER` is not set for this process (vitest runs with `NODE_ENV=test`, and
 * this file constructs its own server instance rather than importing the module-scope
 * `export const server`), so `createMemoryGateway({})` is instantiated directly — the same
 * adapter `gatewayAdapterFromEnv()` resolves to by default when the env var is unset.
 */
describe('gateway-sidecar HTTP contract — fake adapter is detectable on the wire', () => {
  let server: Server | undefined;

  afterEach(async () => {
    if (!server) return;
    await new Promise<void>((resolve, reject) => server?.close((err) => (err ? reject(err) : resolve())));
    server = undefined;
  });

  async function listen(gateway: LlmGatewayPort): Promise<string> {
    server = createGatewayServer(gateway);
    await new Promise<void>((resolve) => server?.listen(0, resolve));
    const address = server?.address();
    if (address === null || typeof address !== 'object') throw new Error('server did not bind');
    return `http://127.0.0.1:${address.port}`;
  }

  it('marks the real (unmocked) memory adapter as adapter=memory, is_fake_adapter=true', async () => {
    const base = await listen(createMemoryGateway({}));
    const response = await fetch(`${base}/v1/complete`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ tenant_id: 't0', messages: [{ role: 'user', content: 'hi' }] }),
    });
    const body = (await response.json()) as {
      adapter: string;
      is_fake_adapter: boolean;
      model: string;
      content: Array<{ text?: string }>;
    };
    expect(response.status).toBe(200);
    expect(body.adapter).toBe('memory');
    expect(body.is_fake_adapter).toBe(true);
    // Belt-and-suspenders on the underlying evidence the flag is derived from, so this test fails
    // loudly (not just "the boolean changed") if the registry's fake constants ever move.
    expect(body.model).toBe('fake-llm-memory');
    expect(body.content[0]?.text).toContain('Memory adapter response.');
  });

  it('does not mark a real-looking non-memory response as fake', async () => {
    const fakeRealPort: LlmGatewayPort = {
      complete: async () => ({
        id: 'r1',
        content: [{ type: 'text' as const, text: 'The sky looks blue because of Rayleigh scattering.' }],
        stop_reason: 'end_turn' as const,
        usage: { input_tokens: 10, output_tokens: 12, cache_read_tokens: 0, cache_creation_tokens: 0 },
        cost: { usd: 0.001, inr: 0.095, model: 'claude-haiku-4-5-20251001', tier: 'small' as const, cached: false },
        model: 'claude-haiku-4-5-20251001',
        latency_ms: 240,
      }),
      stream: async function* () {
        yield { type: 'message_start' as const };
      },
      embed: async () => ({
        embeddings: [],
        model: 'claude-haiku-4-5-20251001',
        usage: { input_tokens: 0 },
        cost: { usd: 0, inr: 0, model: 'claude-haiku-4-5-20251001', tier: 'small' as const, cached: false },
      }),
      countTokens: async () => ({ input_tokens: 0 }),
    };
    // `gatewayAdapterFromEnv()` reads `ORB_LLM_GATEWAY_ADAPTER` at call time inside the handler,
    // so this test must actually set it to a non-memory value — otherwise it would silently prove
    // nothing (the handler would resolve 'memory' regardless of the injected port, and the
    // assertion below would pass for the wrong reason). This is the one env-dependent seam the
    // pure `adapterEvidence.test.ts` unit tests cannot exercise.
    const previous = process.env['ORB_LLM_GATEWAY_ADAPTER'];
    process.env['ORB_LLM_GATEWAY_ADAPTER'] = 'anthropic';
    try {
      const base = await listen(fakeRealPort);
      const response = await fetch(`${base}/v1/complete`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ tenant_id: 't0', messages: [{ role: 'user', content: 'why is the sky blue' }] }),
      });
      const body = (await response.json()) as { adapter: string; is_fake_adapter: boolean; model: string };
      expect(response.status).toBe(200);
      expect(body.adapter).toBe('anthropic');
      expect(body.model).toBe('claude-haiku-4-5-20251001');
      expect(body.is_fake_adapter).toBe(false);
    } finally {
      if (previous === undefined) delete process.env['ORB_LLM_GATEWAY_ADAPTER'];
      else process.env['ORB_LLM_GATEWAY_ADAPTER'] = previous;
    }
  });
});
