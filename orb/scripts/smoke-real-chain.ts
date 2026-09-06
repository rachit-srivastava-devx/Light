#!/usr/bin/env -S npx vite-node
/**
 * E2 (Track E, anti-mirage) — the real-chain smoke run.
 *
 * The gap this closes: today ZERO tests exercise mobile -> relay `/v1/respond` -> gateway-sidecar
 * -> a REAL model, or the TTS path -> REAL Fish audio. Every conversation test uses a
 * `ScriptedGateway` or a mocked port (see `backend/relay-py/tests/test_app_routes.py`), so a
 * silent regression to the fake/`memory` adapter or a broken Fish key would still show a green
 * `npm run verify`. This script is opt-in (needs real, funded keys + the backend chain already
 * running) and asserts on PROVIDER IDENTITY and BYTE COUNTS — never on secret values.
 *
 * Preconditions (this script does not start or stop the chain itself):
 *   1. `. scripts/load-env.sh` sourced into the shell that launches this script (or the caller's
 *      env already has GEMINI_API_KEY/ANTHROPIC_API_KEY/FISH_API_KEY etc. set) — dot-sourced, per
 *      that file's own header comment, never executed.
 *   2. The real backend chain is running: `ORB_START_METRO=0 bash scripts/dev.sh` (or the full
 *      `npm run dev`) in another terminal/process, until "[dev] backend chain is ready" prints.
 *   3. `.env`'s `ORB_LLM_GATEWAY_ADAPTER` is `anthropic` or `gemini` (NOT `memory`) and
 *      `ORB_TTS_PROVIDER=fish` — this script asserts on exactly that, it does not set it.
 *
 * Run:
 *   . scripts/load-env.sh && npx vite-node scripts/smoke-real-chain.ts
 *
 * Exit code 0 only if every assertion below passes. Never logs an API key value — only which
 * provider answered and how many bytes came back.
 */
import { readFileSync, existsSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import {
  isFakeAdapterInvocation,
  FAKE_ADAPTER_MODEL_MARKER,
  FAKE_ADAPTER_TEXT_MARKER,
  type AdapterInvocationEvidence,
} from '../backend/gateway-sidecar/src/adapterEvidence.js';

const RELAY_HTTP_URL = process.env['ORB_RELAY_HTTP_URL'] ?? 'http://127.0.0.1:8765';
const VOICE_SIDECAR_URL = `http://127.0.0.1:${process.env['VOICE_PROVIDER_SIDECAR_PORT'] ?? '8083'}`;
// `.pathname` on a file:// URL percent-encodes spaces (this repo's checkout path contains one,
// "Principal Engineering") — `fileURLToPath` is the correct conversion, same reasoning
// `backend/gateway-sidecar/src/devlog.ts` already documents for its own LOG_DIR default.
const DEV_LOG_DIR =
  process.env['ORB_DEV_LOG_DIR'] ?? fileURLToPath(new URL('../dev-logs', import.meta.url));

/** Well below what any real spoken sentence of a few seconds at 24kHz/16-bit produces (see
 * `backend/voice-provider-sidecar/src/tts/fish.ts`'s `RELAY_AUDIO_SAMPLE_RATE_HZ = 24_000`), well
 * above what an empty/near-silent/error response could produce. */
const MIN_TTS_AUDIO_BYTES = 8_000;
const MIN_REPLY_CHARS = 25;

/** Deterministic client-local fallback text (`backend/relay-py/src/orb_relay/proxy/phrasers.py`,
 * `conversational_response`'s empty-input branch) — the reply must never silently equal this. */
const KNOWN_LOCAL_FALLBACK_TEXT = "I'm here. Let's keep the next move small.";

interface Check {
  readonly name: string;
  readonly pass: boolean;
  readonly detail: string;
}

const checks: Check[] = [];
function record(name: string, pass: boolean, detail: string): void {
  checks.push({ name, pass, detail });
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name} — ${detail}`);
}

async function httpJson<T>(url: string, init?: RequestInit): Promise<{ status: number; body: T }> {
  const res = await fetch(url, init);
  const body = (await res.json()) as T;
  return { status: res.status, body };
}

/** Tail `dev-logs/gateway-sidecar.ndjson` for the last `gateway.response` event matching
 * `tenantId`. Real telemetry gateway-sidecar already writes on every completion
 * (`backend/gateway-sidecar/src/index.ts`'s `devLog('gateway.response', { tenant_id, adapter,
 * is_fake_adapter, model, ... })`) — this is how this script proves adapter identity for a call
 * that went through the FULL relay chain (mobile-shaped `/v1/respond`), not a direct
 * `/v1/complete` call that would prove less.
 */
function findGatewayResponseEvent(tenantId: string): Record<string, unknown> | undefined {
  const path = `${DEV_LOG_DIR}/gateway-sidecar.ndjson`;
  if (!existsSync(path)) return undefined;
  const lines = readFileSync(path, 'utf8').split('\n').filter(Boolean);
  for (let i = lines.length - 1; i >= 0; i -= 1) {
    try {
      const record = JSON.parse(lines[i]!) as Record<string, unknown>;
      if (record['event'] === 'gateway.response' && record['tenant_id'] === tenantId) return record;
    } catch {
      // malformed line — skip, do not crash the smoke run over a log-writer race
    }
  }
  return undefined;
}

async function main(): Promise<number> {
  const runId = randomUUID().slice(0, 8);
  const tenantId = `smoke-e2-${runId}`;
  const sessionId = `smoke-e2-session-${runId}`;
  const userId = `smoke-e2-user-${runId}`;
  // Arbitrary, unpredictable-from-a-script-writer's-perspective open-domain topic: nothing in
  // this repo's fallback/canned-response paths could coincidentally answer this correctly, so a
  // substantive, on-topic reply is real evidence the model actually reasoned about the prompt.
  const prompt = 'In two sentences, how does a bicycle stay upright while moving but fall over when stationary?';

  console.log(`[smoke-real-chain] tenant=${tenantId} relay=${RELAY_HTTP_URL} voice-sidecar=${VOICE_SIDECAR_URL}`);
  console.log(`[smoke-real-chain] prompt: ${prompt}`);

  // ── Leg 1: relay /v1/respond -> gateway-sidecar -> real model ────────────────────────────────
  let respondBody: { text?: string; source?: string; latency_ms?: number } | undefined;
  let respondLatencyMs = -1;
  try {
    const startedAt = performance.now();
    const { status, body } = await httpJson<{ text: string; source: string; latency_ms: number }>(
      `${RELAY_HTTP_URL}/v1/respond`,
      {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ session_id: sessionId, tenant_id: tenantId, user_id: userId, text: prompt }),
      },
    );
    respondLatencyMs = performance.now() - startedAt;
    respondBody = body;
    record('relay./v1/respond reachable', status === 200, `HTTP ${status}, ${respondLatencyMs.toFixed(0)}ms`);
  } catch (err) {
    record(
      'relay./v1/respond reachable',
      false,
      `request failed: ${err instanceof Error ? err.message : String(err)} — is scripts/dev.sh running?`,
    );
  }

  const replyText = respondBody?.text ?? '';
  record(
    'reply is non-empty and substantive',
    replyText.trim().length >= MIN_REPLY_CHARS && replyText.trim() !== KNOWN_LOCAL_FALLBACK_TEXT,
    `${replyText.length} chars: ${JSON.stringify(replyText.slice(0, 200))}`,
  );

  // ── Provider-identity evidence for leg 1, via dev-log correlation (E1) ────────────────────────
  // Give the best-effort, async devLog append a moment to land on disk before reading it.
  await new Promise((resolve) => setTimeout(resolve, 150));
  const gatewayEvent = findGatewayResponseEvent(tenantId);
  if (gatewayEvent === undefined) {
    record(
      'gateway-sidecar dev-log event found for this run',
      false,
      `no gateway.response event with tenant_id=${tenantId} in dev-logs/gateway-sidecar.ndjson ` +
        `(is ORB_DEV_LOGGING=1? default is on)`,
    );
  } else {
    const adapter = String(gatewayEvent['adapter'] ?? '');
    const model = String(gatewayEvent['model'] ?? '');
    const responseText = String(gatewayEvent['response_text'] ?? '');
    const evidence: AdapterInvocationEvidence = {
      adapter: adapter as AdapterInvocationEvidence['adapter'],
      model,
      responseText,
    };
    // Recomputed independently from the raw evidence in the log line, not just trusting the
    // `is_fake_adapter` boolean gateway-sidecar already wrote — belt and suspenders against a
    // future bug in that one write site.
    const recomputedFake = isFakeAdapterInvocation(evidence);
    record(
      'LLM adapter is NOT memory (real model was invoked)',
      adapter !== 'memory' && !recomputedFake,
      `adapter=${adapter} model=${model}` +
        (adapter === 'memory' ? ' — ORB_LLM_GATEWAY_ADAPTER is unset or "memory" in the running chain' : ''),
    );
    record(
      'response does not carry the memory adapter\'s fake markers',
      model !== FAKE_ADAPTER_MODEL_MARKER && !responseText.includes(FAKE_ADAPTER_TEXT_MARKER),
      `model=${r_safe(model)}`,
    );
  }

  // ── Leg 2: real Fish TTS, via voice-provider-sidecar's own HTTP contract ──────────────────────
  // This calls voice-provider-sidecar directly — the ONLY HTTP door to Fish/Sarvam/Cartesia in
  // this repo by that module's own docstring (`backend/voice-provider-sidecar/src/index.ts`).
  // relay-rs (Rust, not owned by this track) speaks the same contract over its own process on the
  // realtime audio hot path; hitting the sidecar's HTTP contract directly proves the same
  // provider-identity/byte-count facts without reimplementing relay-rs's binary WS framing here.
  try {
    const { status, body } = await httpJson<{ status: string; stt: string; tts: string }>(
      `${VOICE_SIDECAR_URL}/healthz`,
    );
    record(
      'voice-provider-sidecar reports tts=fish',
      status === 200 && body.tts === 'fish',
      `HTTP ${status}, stt=${body.stt}, tts=${body.tts}`,
    );
  } catch (err) {
    record(
      'voice-provider-sidecar reports tts=fish',
      false,
      `request failed: ${err instanceof Error ? err.message : String(err)}`,
    );
  }

  try {
    const ttsStartedAt = performance.now();
    const { status, body } = await httpJson<{ audio_chunks: number[][] }>(`${VOICE_SIDECAR_URL}/v1/tts/synthesize`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        text: replyText.trim().length > 0 ? replyText : 'This is a real-chain smoke test of Fish Audio.',
        voice_id: 'orb.warm.v1',
        emotion: 'warm',
      }),
    });
    const ttsLatencyMs = performance.now() - ttsStartedAt;
    const totalBytes = (body.audio_chunks ?? []).reduce((sum, chunk) => sum + chunk.length, 0);
    record(
      'real Fish TTS returned audio bytes over the stated floor',
      status === 200 && totalBytes > MIN_TTS_AUDIO_BYTES,
      `HTTP ${status}, ${totalBytes} bytes across ${(body.audio_chunks ?? []).length} chunks, ` +
        `${ttsLatencyMs.toFixed(0)}ms (floor=${MIN_TTS_AUDIO_BYTES})`,
    );
  } catch (err) {
    record(
      'real Fish TTS returned audio bytes over the stated floor',
      false,
      `request failed: ${err instanceof Error ? err.message : String(err)}`,
    );
  }

  const failed = checks.filter((c) => !c.pass);
  console.log(`\n[smoke-real-chain] ${checks.length - failed.length}/${checks.length} checks passed.`);
  if (failed.length > 0) {
    console.log('[smoke-real-chain] FAILED:');
    for (const check of failed) console.log(`  - ${check.name}: ${check.detail}`);
    return 1;
  }
  return 0;
}

// Defensive JSON-ish quoting without pulling in a formatting dependency; only used for a
// diagnostic string, never for anything parsed back.
function r_safe(value: string): string {
  return JSON.stringify(value);
}

main()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((err) => {
    console.error('[smoke-real-chain] uncaught error:', err);
    process.exitCode = 1;
  });
