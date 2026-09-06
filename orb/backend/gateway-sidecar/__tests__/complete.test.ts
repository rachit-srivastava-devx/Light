import { describe, expect, it } from 'vitest';
import { createMemoryGateway } from '@pe/llm-gateway/adapters/memory';
import { MissingTenantError, type LlmGatewayPort, type UsageEvent } from '@pe/llm-gateway';
import { createGatewayFromEnv, createGatewayServer, gatewayAdapterFromEnv } from '../src/index';

/**
 * These test the contract the sidecar depends on, not HTTP plumbing: if the registry's memory
 * adapter stops enforcing C8/C12 the way the sidecar's error mapping assumes, that mapping turns
 * a 400 into a 502 silently. Asserting the adapter's behaviour here is what makes the sidecar's
 * status-code mapping meaningful.
 */
describe('llm-gateway memory adapter — the contract the sidecar maps to HTTP', () => {
  it('rejects a request with no tenant_id (C8) so the sidecar can map it to 400', async () => {
    const gateway = createMemoryGateway({});
    await expect(
      gateway.complete({ tenant_id: '', messages: [{ role: 'user', content: 'hi' }] }),
    ).rejects.toBeInstanceOf(MissingTenantError);
  });

  it('returns a deterministic completion at T0 with zero spend', async () => {
    const gateway = createMemoryGateway({});
    const req = { tenant_id: 't1', messages: [{ role: 'user' as const, content: 'one atomic step' }] };
    const a = await gateway.complete(req);
    const b = await gateway.complete(req);
    expect(a.content).toEqual(b.content);
  });

  it('emits a usage event the cost plane can meter (C12)', async () => {
    const events: UsageEvent[] = [];
    const gateway = createMemoryGateway({ onUsage: (e: UsageEvent) => events.push(e) });
    await gateway.complete({ tenant_id: 't1', messages: [{ role: 'user', content: 'hi' }] });
    expect(events.length).toBe(1);
  });
});

describe('gateway-sidecar adapter selection', () => {
  it('defaults to memory so T0 starts with no provider secret', () => {
    expect(gatewayAdapterFromEnv({})).toBe('memory');
    expect(createGatewayFromEnv({})).toBeDefined();
  });

  it('rejects unknown adapters instead of silently changing the model door', () => {
    expect(() => gatewayAdapterFromEnv({ ORB_LLM_GATEWAY_ADAPTER: 'openai' })).toThrow(
      /unsupported ORB_LLM_GATEWAY_ADAPTER/,
    );
  });

  it('requires an Anthropic key before live-provider mode starts', () => {
    expect(() => createGatewayFromEnv({ ORB_LLM_GATEWAY_ADAPTER: 'anthropic' })).toThrow(
      /ANTHROPIC_API_KEY is required/,
    );
  });

  it('accepts gemini as a selectable adapter', () => {
    expect(gatewayAdapterFromEnv({ ORB_LLM_GATEWAY_ADAPTER: 'gemini' })).toBe('gemini');
  });

  it('requires a Gemini key before live-provider mode starts', () => {
    expect(() => createGatewayFromEnv({ ORB_LLM_GATEWAY_ADAPTER: 'gemini' })).toThrow(
      /GEMINI_API_KEY is required/,
    );
  });
});

describe('gateway-sidecar speech boundary', () => {
  it('returns normalized speech metadata and preserves Fish markup over HTTP', async () => {
    let receivedSystem = '';
    const gateway: LlmGatewayPort = {
      complete: async (request) => {
        receivedSystem = request.system ?? '';
        return {
        id: 'r1',
        content: [{ type: 'text' as const, text: JSON.stringify({
          intent: 'presence',
          spoken_text: '[chuckle] I am here. [emphasis] One step.',
          emotion: 'warm',
          interruptible: true,
          max_duration_ms: 4200,
          follow_up: { kind: 'none', delay_ms: 0 },
        }) }],
        stop_reason: 'end_turn' as const,
        usage: { input_tokens: 1, output_tokens: 1, cache_read_tokens: 0, cache_creation_tokens: 0 },
        cost: { usd: 0, inr: 0, model: 'judge', tier: 'small' as const, cached: false },
        model: 'judge',
        latency_ms: 1,
        };
      },
      stream: async function* () { yield { type: 'message_start' as const }; },
      embed: async () => ({
        embeddings: [],
        model: 'judge',
        usage: { input_tokens: 0 },
        cost: { usd: 0, inr: 0, model: 'judge', tier: 'small' as const, cached: false },
      }),
      countTokens: async () => ({ input_tokens: 0 }),
    };
    const server = createGatewayServer(gateway);
    await new Promise<void>((resolve) => server.listen(0, resolve));
    const address = server.address();
    if (address === null || typeof address === 'string') throw new Error('server did not bind');
    try {
      const response = await fetch(`http://127.0.0.1:${address.port}/v1/complete`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          tenant_id: 't0',
          response_mode: 'speech',
          system: 'Answer as a supportive body double.',
          messages: [{ role: 'user', content: 'hi' }],
        }),
      });
      const body = await response.json() as { content: Array<{ text?: string }>; speech: { plain_text: string; tags: string[] } };
      expect(response.status).toBe(200);
      expect(body.content[0]?.text).toContain('[chuckle] I am here. [emphasis] One step.');
      expect(body.speech.plain_text).toBe('I am here. One step.');
      expect(body.speech.tags).toEqual(['chuckle', 'emphasis']);
      expect(receivedSystem).toContain('Return ONLY one JSON object');
      expect(receivedSystem).toContain('Answer as a supportive body double.');
    } finally {
      await new Promise<void>((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
    }
  });

  /**
   * readJsonBody throws on malformed JSON before handleComplete's own try/catch runs. Route
   * dispatch calls it as `void handleComplete(...)` (fire-and-forget) — without a catch on that
   * promise, this throw becomes an unhandled rejection, which crashes the whole process by
   * default in Node. A malformed request must return 400 and leave the process (and this test's
   * server) alive for the next request, not take the sidecar down.
   */
  it('malformed JSON returns 400 instead of crashing the process', async () => {
    const gateway: LlmGatewayPort = createMemoryGateway({});
    const server = createGatewayServer(gateway);
    await new Promise<void>((resolve) => server.listen(0, resolve));
    const address = server.address();
    if (address === null || typeof address === 'string') throw new Error('server did not bind');
    try {
      const response = await fetch(`http://127.0.0.1:${address.port}/v1/complete`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: '{not valid json',
      });
      expect(response.status).toBe(400);

      const healthy = await fetch(`http://127.0.0.1:${address.port}/healthz`);
      expect(healthy.status).toBe(200);
    } finally {
      await new Promise<void>((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
    }
  });
});
