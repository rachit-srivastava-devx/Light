import { describe, expect, it } from 'vitest';
import type { LlmGatewayPort } from '@pe/llm-gateway';
import {
  DEFAULT_SDK_LOADERS,
  UnsupportedPreflightModeError,
  gatewayPreflightModeFromEnv,
  preflightGateway,
  type SdkLoaders,
} from '../src/preflight.js';

/** Minimal stand-in: only `complete` is reachable from the preflight path. */
function fakeGateway(complete: () => Promise<unknown> = async () => ({})): LlmGatewayPort {
  return { complete } as unknown as LlmGatewayPort;
}

/** Deterministic clock — the preflight must never read the wall clock itself. */
function fakeClock(...ticks: number[]): () => number {
  const queue = [...ticks];
  return () => queue.shift() ?? (ticks[ticks.length - 1] ?? 0);
}

const loaders = (overrides: Partial<SdkLoaders>): SdkLoaders => ({
  memory: null,
  anthropic: async () => ({ ok: true }),
  gemini: async () => ({ ok: true }),
  ...overrides,
});

describe('gatewayPreflightModeFromEnv', () => {
  it('defaults to import when unset or blank', () => {
    expect(gatewayPreflightModeFromEnv({})).toBe('import');
    expect(gatewayPreflightModeFromEnv({ ORB_GATEWAY_PREFLIGHT: '' })).toBe('import');
    expect(gatewayPreflightModeFromEnv({ ORB_GATEWAY_PREFLIGHT: '   ' })).toBe('import');
  });

  it('accepts the three modes, case- and whitespace-insensitively', () => {
    expect(gatewayPreflightModeFromEnv({ ORB_GATEWAY_PREFLIGHT: 'off' })).toBe('off');
    expect(gatewayPreflightModeFromEnv({ ORB_GATEWAY_PREFLIGHT: ' CALL ' })).toBe('call');
    expect(gatewayPreflightModeFromEnv({ ORB_GATEWAY_PREFLIGHT: 'Import' })).toBe('import');
  });

  it('throws on a typo instead of silently disabling the gate', () => {
    expect(() => gatewayPreflightModeFromEnv({ ORB_GATEWAY_PREFLIGHT: 'imprt' })).toThrow(
      UnsupportedPreflightModeError,
    );
    // The rejected value is echoed back, so the operator can see their own typo.
    expect(() => gatewayPreflightModeFromEnv({ ORB_GATEWAY_PREFLIGHT: 'yes' })).toThrow(/"yes"/);
  });
});

describe('preflightGateway — a skip is never a pass', () => {
  it('mode=off reports skipped and says nothing was measured', async () => {
    const r = await preflightGateway({
      gateway: fakeGateway(), adapter: 'gemini', mode: 'off', loaders: loaders({}), now: fakeClock(0, 0),
    });
    expect(r.outcome).toBe('skipped');
    expect(r.measured).toMatch(/nothing/);
  });

  it('the memory adapter reports skipped — it has no provider SDK', async () => {
    const r = await preflightGateway({
      gateway: fakeGateway(), adapter: 'memory', mode: 'import', loaders: loaders({}), now: fakeClock(0, 0),
    });
    expect(r.outcome).toBe('skipped');
    expect(r.measured).toMatch(/no provider SDK/);
  });

  it('never invokes a provider probe while skipping', async () => {
    let probed = 0;
    await preflightGateway({
      gateway: fakeGateway(), adapter: 'memory', mode: 'call', loaders: loaders({}),
      now: fakeClock(0, 0), probe: async () => { probed += 1; },
    });
    expect(probed).toBe(0);
  });
});

describe('preflightGateway — import mode', () => {
  it('passes when the SDK loads, and states what it did NOT prove', async () => {
    const r = await preflightGateway({
      gateway: fakeGateway(), adapter: 'gemini', mode: 'import', loaders: loaders({}), now: fakeClock(100, 142),
    });
    expect(r.outcome).toBe('ok');
    expect(r.latency_ms).toBe(42);
    expect(r.measured).toMatch(/NOT checked/);
  });

  it('reports the real underlying error rather than a fixed string', async () => {
    const cause = Object.assign(new Error("Cannot find package '@google/generative-ai'"), {
      code: 'ERR_MODULE_NOT_FOUND',
    });
    const r = await preflightGateway({
      gateway: fakeGateway(), adapter: 'gemini', mode: 'import',
      loaders: loaders({ gemini: async () => { throw cause; } }), now: fakeClock(0, 5),
    });
    expect(r.outcome).toBe('failed');
    expect(r.error).toEqual({
      name: 'Error', message: "Cannot find package '@google/generative-ai'", code: 'ERR_MODULE_NOT_FOUND',
    });
  });

  it('treats a resolvable-but-not-a-module result as a failure, not a pass', async () => {
    for (const bad of [null, undefined, 'a string', 42]) {
      const r = await preflightGateway({
        gateway: fakeGateway(), adapter: 'gemini', mode: 'import',
        loaders: loaders({ gemini: async () => bad as unknown }), now: fakeClock(0, 1),
      });
      expect(r.outcome, `value ${String(bad)}`).toBe('failed');
    }
  });

  it('survives a non-Error throw', async () => {
    const r = await preflightGateway({
      gateway: fakeGateway(), adapter: 'gemini', mode: 'import',
      loaders: loaders({ gemini: async () => { throw 'string thrown'; } }), now: fakeClock(0, 1),
    });
    expect(r.outcome).toBe('failed');
    expect(r.error?.message).toBe('string thrown');
  });

  it('is bounded: a hung import fails the preflight instead of hanging boot', async () => {
    const r = await preflightGateway({
      gateway: fakeGateway(), adapter: 'gemini', mode: 'import',
      loaders: loaders({ gemini: () => new Promise(() => {}) }), timeoutMs: 5, now: fakeClock(0, 6),
    });
    expect(r.outcome).toBe('failed');
    expect(r.error?.code).toBe('PREFLIGHT_TIMEOUT');
  });

  it('throws for an adapter with no entry in the loader table', async () => {
    await expect(
      preflightGateway({
        gateway: fakeGateway(), adapter: 'newprovider' as never, mode: 'import',
        loaders: loaders({}), now: fakeClock(0, 0),
      }),
    ).rejects.toThrow(/no preflight entry/);
  });
});

describe('preflightGateway — call mode', () => {
  it('pushes exactly one discarded completion and reports the stronger claim', async () => {
    let calls = 0;
    const r = await preflightGateway({
      gateway: fakeGateway(), adapter: 'gemini', mode: 'call', loaders: loaders({}),
      now: fakeClock(0, 900), probe: async () => { calls += 1; return { content: [] }; },
    });
    expect(calls).toBe(1);
    expect(r.outcome).toBe('ok');
    expect(r.measured).toMatch(/credentials and provider reachability all proven/);
  });

  it('fails on a provider/credential error from the discarded call', async () => {
    const r = await preflightGateway({
      gateway: fakeGateway(), adapter: 'gemini', mode: 'call', loaders: loaders({}), now: fakeClock(0, 3),
      probe: async () => { throw Object.assign(new Error('GEMINI_API_KEY is not set'), { code: 'PROVIDER_ERROR' }); },
    });
    expect(r.outcome).toBe('failed');
    expect(r.measured).toMatch(/discarded boot completion/);
    expect(r.error?.message).toBe('GEMINI_API_KEY is not set');
  });

  it('does not reach the probe when the SDK import already failed', async () => {
    let probed = 0;
    const r = await preflightGateway({
      gateway: fakeGateway(), adapter: 'gemini', mode: 'call',
      loaders: loaders({ gemini: async () => { throw new Error('boom'); } }),
      now: fakeClock(0, 1), probe: async () => { probed += 1; },
    });
    expect(r.outcome).toBe('failed');
    expect(probed).toBe(0);
  });
});

describe('DEFAULT_SDK_LOADERS — the real table, actually invoked', () => {
  // Not a mock: this proves the literal specifiers in preflight.ts resolve from this package for
  // real. An adoption with no successful invocation is a claim, not a check. No network involved.
  it.each(['gemini', 'anthropic'] as const)('%s resolves and yields a module namespace', async (adapter) => {
    const load = DEFAULT_SDK_LOADERS[adapter];
    expect(load).not.toBeNull();
    const mod = await load!();
    expect(typeof mod).toBe('object');
  });

  it('memory is explicitly null, not a missing key', () => {
    expect('memory' in DEFAULT_SDK_LOADERS).toBe(true);
    expect(DEFAULT_SDK_LOADERS.memory).toBeNull();
  });
});
