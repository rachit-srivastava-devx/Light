/**
 * Boot-time provider-SDK preflight for the gateway sidecar.
 *
 * WHY THIS EXISTS. On 2026-08-28 a live sidecar (pid 86089, booted 02:18:53) answered four
 * consecutive `POST /v1/complete` calls with HTTP 502 —
 *   "[gemini] @google/generative-ai is not installed — add it to dependencies to use this adapter"
 * — and then, with no restart and no code change, served every later call from that same process
 * against the real provider. Each adapter loads its provider SDK lazily on its first request, so
 * whoever sends that first request is the one who discovers the SDK cannot be loaded, and the only
 * trace left behind is a 502 with a fixed string in it.
 *
 * Two properties were missing. This module supplies both:
 *
 *  1. FAIL AT BOOT, NOT ON SOMEONE'S REQUEST. The load is attempted before `server.listen()`, so a
 *     provider SDK that cannot be loaded stops the process instead of turning it into a 502
 *     machine that looks healthy on `/healthz`.
 *  2. MEASURE, DON'T ASSUME. The result names what was actually proven. `outcome: 'skipped'` is
 *     never reported as a pass — a preflight that checked nothing is not a green preflight.
 *
 * The original trigger could not be recovered after the fact: the adapter's `catch` discarded the
 * underlying error and substituted a fixed "not installed" string, while the packages were (and
 * still are) correctly declared and installed — every later reproduction attempt passes. That is
 * exactly the point. This preflight exists so the next occurrence arrives as a boot failure
 * carrying the real error instead of as an unexplained 502.
 *
 * Pure except for its injected seams: the clock, the SDK loaders and the probe call are all
 * parameters, so every branch is testable with no network, no provider key and no real clock.
 */
import type { GatewayAdapterName } from './index.js';
import type { LlmGatewayPort } from '@pe/llm-gateway';

/**
 * `import` — load the provider SDK module and nothing else. Free, no network, no credentials used.
 *            This is the failure that was actually observed, so it is the default.
 * `call`   — additionally push one discarded `complete()` (max_tokens: 1) through the adapter.
 *            Strictly stronger: it exercises the adapter's OWN lazy load at the adapter's own
 *            module-resolution base, plus credentials and provider reachability. Costs one minimal
 *            billed request per boot and couples boot to provider uptime, so it is opt-in.
 * `off`    — skip. Reported as `skipped`, never as a pass.
 */
export type GatewayPreflightMode = 'import' | 'call' | 'off';

const PREFLIGHT_MODES: readonly GatewayPreflightMode[] = ['import', 'call', 'off'];

/** Thrown for an unrecognised ORB_GATEWAY_PREFLIGHT rather than silently falling back to a
 *  default — a typo in an operator's env must not quietly disable a boot gate. */
export class UnsupportedPreflightModeError extends Error {
  readonly code = 'UNSUPPORTED_PREFLIGHT_MODE';
  constructor(readonly value: string) {
    super(
      `unsupported ORB_GATEWAY_PREFLIGHT: ${JSON.stringify(value)} ` +
        `(expected one of ${PREFLIGHT_MODES.join(' | ')})`,
    );
    this.name = 'UnsupportedPreflightModeError';
  }
}

export function gatewayPreflightModeFromEnv(env: NodeJS.ProcessEnv = process.env): GatewayPreflightMode {
  const raw = env['ORB_GATEWAY_PREFLIGHT'];
  // Unset and empty-string both mean "operator expressed no preference" — take the default.
  if (raw === undefined || raw.trim() === '') return 'import';
  const value = raw.trim().toLowerCase();
  if ((PREFLIGHT_MODES as readonly string[]).includes(value)) return value as GatewayPreflightMode;
  throw new UnsupportedPreflightModeError(raw);
}

/**
 * Literal specifiers on purpose: a variable in `import()` is invisible to Vite's and every other
 * bundler's static analysis, and this file runs under vite-node in dev. `memory` has no provider
 * SDK, which is why the value is explicitly `null` rather than a missing key — a new adapter must
 * make a deliberate choice here instead of silently getting no preflight.
 */
export type SdkLoaders = Readonly<Record<GatewayAdapterName, (() => Promise<unknown>) | null>>;

export const DEFAULT_SDK_LOADERS: SdkLoaders = {
  memory: null,
  anthropic: () => import('@anthropic-ai/sdk'),
  gemini: () => import('@google/generative-ai'),
};

export interface GatewayPreflightResult {
  readonly adapter: GatewayAdapterName;
  readonly mode: GatewayPreflightMode;
  /** `skipped` means nothing was measured. It is not a pass and must not be rendered as one. */
  readonly outcome: 'ok' | 'failed' | 'skipped';
  /** Human-readable statement of what was actually proven (or why nothing was). */
  readonly measured: string;
  readonly latency_ms: number;
  readonly error?: { readonly name: string; readonly message: string; readonly code?: string };
}

/** Boot must not hang on a wedged provider or filesystem. Bounded, and the bound is injectable. */
export const DEFAULT_PREFLIGHT_TIMEOUT_MS = 15_000;

class PreflightTimeoutError extends Error {
  readonly code = 'PREFLIGHT_TIMEOUT';
  constructor(what: string, ms: number) {
    super(`${what} did not settle within ${ms}ms`);
    this.name = 'PreflightTimeoutError';
  }
}

async function withTimeout<T>(what: string, ms: number, operation: () => Promise<T>): Promise<T> {
  if (!Number.isFinite(ms) || ms <= 0) return operation();
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      operation(),
      new Promise<never>((_resolve, reject) => {
        timer = setTimeout(() => reject(new PreflightTimeoutError(what, ms)), ms);
      }),
    ]);
  } finally {
    // Without this the pending timer keeps the event loop alive and boot appears to hang after a
    // successful preflight.
    if (timer !== undefined) clearTimeout(timer);
  }
}

function describeError(err: unknown): { name: string; message: string; code?: string } {
  if (err instanceof Error) {
    const code = (err as { code?: unknown }).code;
    const base = { name: err.name, message: err.message };
    return typeof code === 'string' ? { ...base, code } : base;
  }
  // Non-Error throws (a string, a number, null) are rare but real; never lose them to String(undefined).
  return { name: 'NonError', message: typeof err === 'string' ? err : JSON.stringify(err) ?? String(err) };
}

export interface PreflightOptions {
  readonly gateway: LlmGatewayPort;
  readonly adapter: GatewayAdapterName;
  readonly mode: GatewayPreflightMode;
  readonly loaders?: SdkLoaders;
  readonly timeoutMs?: number;
  /** Injected clock — no wall-clock read inside the decision logic. */
  readonly now?: () => number;
  /** Injected probe so `call` mode is testable without a provider. */
  readonly probe?: (gateway: LlmGatewayPort) => Promise<unknown>;
}

/** The discarded boot call. `max_tokens: 1` keeps it the cheapest request the provider will bill. */
function defaultProbe(gateway: LlmGatewayPort): Promise<unknown> {
  return gateway.complete({
    tenant_id: 'gateway-sidecar-preflight',
    feature_id: 'boot-preflight',
    messages: [{ role: 'user', content: 'ok' }],
    max_tokens: 1,
  });
}

/**
 * Attempt the provider SDK load (and, in `call` mode, one discarded completion) and report what
 * was proven. Never throws for a provider failure — the caller decides what a red preflight means
 * for the process. Does throw for a programming error (an adapter missing from `loaders`), because
 * that is a defect in this file's own table, not a runtime condition.
 *
 * O(1). Single-threaded boot path; introduces no shared mutable state.
 */
export async function preflightGateway(opts: PreflightOptions): Promise<GatewayPreflightResult> {
  const { gateway, adapter, mode } = opts;
  const loaders = opts.loaders ?? DEFAULT_SDK_LOADERS;
  const timeoutMs = opts.timeoutMs ?? DEFAULT_PREFLIGHT_TIMEOUT_MS;
  const now = opts.now ?? Date.now;
  const probe = opts.probe ?? defaultProbe;
  const started = now();
  const elapsed = (): number => now() - started;

  if (mode === 'off') {
    return { adapter, mode, outcome: 'skipped', measured: 'nothing (ORB_GATEWAY_PREFLIGHT=off)', latency_ms: elapsed() };
  }

  if (!(adapter in loaders)) {
    // A new adapter was added to GatewayAdapterName without a decision here. Fail loudly at boot
    // rather than silently preflighting nothing.
    throw new Error(`no preflight entry for adapter ${JSON.stringify(adapter)} — add one to DEFAULT_SDK_LOADERS`);
  }

  const load = loaders[adapter];
  if (load === null) {
    return {
      adapter,
      mode,
      outcome: 'skipped',
      measured: `nothing (the ${adapter} adapter has no provider SDK to load)`,
      latency_ms: elapsed(),
    };
  }

  try {
    const mod = await withTimeout(`provider SDK import for ${adapter}`, timeoutMs, load);
    if (mod === null || typeof mod !== 'object') {
      // A resolvable specifier that yields a non-module is a broken install, not a success.
      throw new Error(`provider SDK for ${adapter} loaded but is not a module namespace (got ${typeof mod})`);
    }
  } catch (err) {
    return {
      adapter,
      mode,
      outcome: 'failed',
      measured: `provider SDK import for ${adapter} — FAILED`,
      latency_ms: elapsed(),
      error: describeError(err),
    };
  }

  if (mode === 'import') {
    return {
      adapter,
      mode,
      outcome: 'ok',
      // Say what this does NOT prove, so a green line is not read as "the provider works".
      measured: `provider SDK for ${adapter} is loadable (credentials and provider reachability NOT checked)`,
      latency_ms: elapsed(),
    };
  }

  try {
    await withTimeout(`boot completion probe for ${adapter}`, timeoutMs, () => probe(gateway));
  } catch (err) {
    return {
      adapter,
      mode,
      outcome: 'failed',
      measured: `discarded boot completion via the ${adapter} adapter — FAILED`,
      latency_ms: elapsed(),
      error: describeError(err),
    };
  }

  return {
    adapter,
    mode,
    outcome: 'ok',
    measured: `a real completion through the ${adapter} adapter (SDK load, credentials and provider reachability all proven)`,
    latency_ms: elapsed(),
  };
}
