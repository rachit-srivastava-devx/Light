/**
 * Thin HTTP door onto @pe/llm-gateway (registry, C1/C9) for the Python/Rust backend.
 * docs/adr/0004-backend-language-split.md explains why this exists instead of a Python port of
 * the gateway's routing/budget logic: one implementation, one contract suite, no drift.
 *
 * No framework dependency (C3 — no new runtime dep without an ADR; this repo already limits
 * itself to the two registry packages) — plain `node:http` is enough for an internal sidecar.
 */
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { createMemoryGateway } from '@pe/llm-gateway/adapters/memory';
import { createAnthropicGateway } from '@pe/llm-gateway/adapters/anthropic';
import { createGeminiGateway } from '@pe/llm-gateway/adapters/gemini';
import type { CompletionRequest, LlmGatewayPort } from '@pe/llm-gateway';
import { BudgetExhaustedError, MissingTenantError } from '@pe/llm-gateway';
import { devLog, truncate } from './devlog.js';
import { normalizeCompletionResponse, SPEECH_MARKUP_SYSTEM_PROMPT } from './speechMarkup.js';
import { isFakeAdapterInvocation, type AdapterInvocationEvidence } from './adapterEvidence.js';
import { gatewayPreflightModeFromEnv, preflightGateway } from './preflight.js';

/**
 * T0 default is the memory adapter (₹0 infra, deterministic — matches every other registry
 * consumer's default). `ORB_LLM_GATEWAY_ADAPTER=anthropic|gemini` swaps to a real provider — the
 * same one-line change `llm-gateway`'s own README documents; kept here (not duplicated anywhere
 * else) so this sidecar stays the single place `ANTHROPIC_API_KEY`/`GEMINI_API_KEY` wiring lives,
 * per C9.
 */
export type GatewayAdapterName = 'memory' | 'anthropic' | 'gemini';

export function gatewayAdapterFromEnv(env: NodeJS.ProcessEnv = process.env): GatewayAdapterName {
  const adapter = env['ORB_LLM_GATEWAY_ADAPTER'] ?? 'memory';
  if (adapter === 'memory' || adapter === 'anthropic' || adapter === 'gemini') return adapter;
  throw new Error(`unsupported ORB_LLM_GATEWAY_ADAPTER: ${adapter}`);
}

export function createGatewayFromEnv(env: NodeJS.ProcessEnv = process.env): LlmGatewayPort {
  const adapter = gatewayAdapterFromEnv(env);
  if (adapter === 'memory') return createMemoryGateway({});
  if (adapter === 'gemini') {
    if (!env['GEMINI_API_KEY']) {
      throw new Error('GEMINI_API_KEY is required when ORB_LLM_GATEWAY_ADAPTER=gemini');
    }
    return createGeminiGateway({});
  }
  if (!env['ANTHROPIC_API_KEY']) {
    throw new Error('ANTHROPIC_API_KEY is required when ORB_LLM_GATEWAY_ADAPTER=anthropic');
  }
  return createAnthropicGateway({});
}

const gateway: LlmGatewayPort = createGatewayFromEnv();

// A client that opens a connection and trickles bytes (or never sends the terminating chunk)
// must not hold this handler — and the socket behind it — open forever. That is the same "the
// stop path never actually released it" shape as the SIGTERM/recordVideo bug, here for an HTTP
// request instead of a subprocess. 1MB is generous for this sidecar's completion-request bodies;
// 10s is generous against the app's own p99 voice-to-voice target (<=2.0s for the full round trip).
const MAX_JSON_BODY_BYTES = 1_000_000;
const BODY_READ_TIMEOUT_MS = 10_000;

async function readJsonBody(req: IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = [];
  let total = 0;
  const timeout = setTimeout(() => req.destroy(new Error('request body read timed out')), BODY_READ_TIMEOUT_MS);
  try {
    for await (const chunk of req) {
      const buf = chunk as Buffer;
      total += buf.length;
      if (total > MAX_JSON_BODY_BYTES) {
        req.destroy();
        throw new Error(`request body exceeds ${MAX_JSON_BODY_BYTES} bytes`);
      }
      chunks.push(buf);
    }
  } finally {
    clearTimeout(timeout);
  }
  const raw = Buffer.concat(chunks).toString('utf8');
  return raw.length ? JSON.parse(raw) : {};
}

function sendJson(res: ServerResponse, status: number, body: unknown): void {
  const payload = JSON.stringify(body);
  res.writeHead(status, { 'content-type': 'application/json', 'content-length': Buffer.byteLength(payload) });
  res.end(payload);
}

// Route handlers are invoked as `void handler(...)` (fire-and-forget) below. Anything the handler
// itself doesn't catch — a malformed body, the new size/timeout guard above, any other thrown
// error before its own try/catch — becomes an unhandled promise rejection, which crashes the
// whole process by default in Node. One bad request should return 400, not take the sidecar down.
function runHandler(res: ServerResponse, handling: Promise<void>): void {
  handling.catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    devLog('gateway.unhandled_request_error', { error: message }, 'error');
    if (!res.headersSent) sendJson(res, 400, { error: 'BAD_REQUEST', message });
  });
}

function messageText(content: string | readonly { type: string; text?: string }[]): string {
  if (typeof content === 'string') return content;
  return content
    .filter((b) => b.type === 'text')
    .map((b) => b.text ?? '')
    .join('\n');
}

type GatewayHttpRequest = CompletionRequest & {
  /** Opt-in mode for user-facing LLM speech; atomizer requests remain schema-only by default. */
  readonly response_mode?: 'speech';
};

/**
 * E1 (Track E) — the HTTP response envelope every `/v1/complete` caller actually receives.
 * `adapter` and `is_fake_adapter` make provider identity a first-class, observable fact instead
 * of something only visible by tailing `dev-logs/gateway-sidecar.ndjson`. See
 * `evals/TELEMETRY-CONTRACT.md` for the field contract other tracks/consumers should read.
 */
export interface GatewayHttpResponse {
  readonly adapter: GatewayAdapterName;
  readonly is_fake_adapter: boolean;
}

async function handleComplete(
  req: IncomingMessage,
  res: ServerResponse,
  gateway: LlmGatewayPort,
): Promise<void> {
  const body = (await readJsonBody(req)) as GatewayHttpRequest;
  const { response_mode: responseMode, ...completionBody } = body;
  const completionRequest: CompletionRequest = responseMode === 'speech'
    ? {
        ...completionBody,
        system: [SPEECH_MARKUP_SYSTEM_PROMPT, completionBody.system].filter(Boolean).join('\n\n'),
      }
    : completionBody;
  const startedAt = Date.now();
  const userText = completionRequest.messages.map((m) => messageText(m.content)).join('\n');
  devLog('gateway.request', {
    tenant_id: completionRequest.tenant_id,
    adapter: gatewayAdapterFromEnv(),
    response_mode: responseMode ?? 'structured',
    system: truncate(completionRequest.system ?? ''),
    user_text: truncate(userText),
    max_tokens: completionRequest.max_tokens,
  });
  try {
    const result = await gateway.complete(completionRequest);
    // The gateway's own CompletionResponse already carries the real model, usage, cost, and
    // latency it measured — log those verbatim rather than re-deriving a local latency, which
    // would double-count any queueing this handler adds on top.
    const normalized = normalizeCompletionResponse(result);
    const adapter = gatewayAdapterFromEnv();
    // E1: this is the ONE evidence check every `/v1/complete` response is run through, in the ONE
    // place C9 says provider calls live. `responseText` is the pre-markup text (not
    // `normalized.speech.plain_text`) so a model that happens to echo the fake marker inside a
    // real reply is still caught, and so this does not depend on speechMarkup's normalization
    // ever changing.
    const rawResponseText = result.content
      .map((block) => (block.type === 'text' ? block.text : ''))
      .join('\n');
    const evidence: AdapterInvocationEvidence = { adapter, model: result.model, responseText: rawResponseText };
    const isFakeAdapter = isFakeAdapterInvocation(evidence);
    devLog('gateway.response', {
      tenant_id: completionRequest.tenant_id,
      adapter,
      is_fake_adapter: isFakeAdapter,
      model: result.model,
      tier: result.cost.tier,
      response_text: truncate(normalized.speech.plain_text),
      speech_tags: normalized.speech.tags,
      speech_intent: normalized.speech.intent,
      usage: result.usage,
      cost_usd: result.cost.usd,
      cost_inr: result.cost.inr,
      cached: result.cost.cached,
      latency_ms: result.latency_ms,
      handler_latency_ms: Date.now() - startedAt,
    });
    const responseBody: typeof normalized & GatewayHttpResponse = {
      ...normalized,
      adapter,
      is_fake_adapter: isFakeAdapter,
    };
    sendJson(res, 200, responseBody);
  } catch (err: unknown) {
    const latencyMs = Date.now() - startedAt;
    // C8 and C12 map to distinct client-visible outcomes: a missing tenant is the caller's bug
    // (400), an exhausted budget is a real, expected in-path rejection the caller must handle
    // without retrying blindly (402). Collapsing both into 500 would hide a spend guard.
    if (err instanceof MissingTenantError) {
      devLog(
        'gateway.error',
        { tenant_id: completionRequest.tenant_id, status: 400, code: err.code, latency_ms: latencyMs },
        'error',
      );
      return sendJson(res, 400, { error: err.code, message: err.message });
    }
    if (err instanceof BudgetExhaustedError) {
      devLog(
        'gateway.error',
        { tenant_id: completionRequest.tenant_id, status: 402, code: err.code, latency_ms: latencyMs },
        'warn',
      );
      return sendJson(res, 402, { error: err.code, message: err.message });
    }
    devLog(
      'gateway.error',
      {
        tenant_id: completionRequest.tenant_id,
        status: 502,
        code: 'GATEWAY_ERROR',
        message: err instanceof Error ? err.message : String(err),
        latency_ms: latencyMs,
      },
      'error',
    );
    sendJson(res, 502, {
      error: 'GATEWAY_ERROR',
      message: err instanceof Error ? err.message : String(err),
    });
  }
}

export function createGatewayServer(gateway: LlmGatewayPort): ReturnType<typeof createServer> {
  return createServer((req, res) => {
  if (req.method === 'GET' && req.url === '/healthz') return sendJson(res, 200, { status: 'ok' });
  if (req.method === 'POST' && req.url === '/v1/complete') return runHandler(res, handleComplete(req, res, gateway));
  sendJson(res, 404, { error: 'NOT_FOUND' });
  });
}

export const server = createGatewayServer(gateway);

const PORT = Number(process.env['GATEWAY_SIDECAR_PORT'] ?? 8082);
if (process.env['NODE_ENV'] !== 'test') {
  // Load the provider SDK BEFORE listening. Each adapter loads its SDK lazily on its first real
  // request, which once turned an unloadable SDK into four consecutive 502s served by a process
  // that answered /healthz with 200 the whole time — see preflight.ts for the incident. A red
  // preflight refuses to serve instead, with the real error in front of whoever started it.
  const preflight = await preflightGateway({
    gateway,
    adapter: gatewayAdapterFromEnv(),
    mode: gatewayPreflightModeFromEnv(),
  });
  devLog('gateway.preflight', { ...preflight }, preflight.outcome === 'failed' ? 'error' : 'info');
  if (preflight.outcome === 'failed') {
    const { name, message, code } = preflight.error ?? { name: 'Error', message: 'unknown' };
    // eslint-disable-next-line no-console
    console.error(
      `gateway-sidecar preflight FAILED (adapter=${preflight.adapter}, ${preflight.measured}): ` +
        `${name}: ${message}${code === undefined ? '' : ` [${code}]`}`,
    );
    // exitCode rather than process.exit(): nothing is listening yet, so the loop drains on its own
    // and no buffered stderr is truncated on the way out.
    process.exitCode = 1;
  } else {
    // eslint-disable-next-line no-console
    console.log(
      `gateway-sidecar preflight ${preflight.outcome} in ${preflight.latency_ms}ms — proved: ${preflight.measured}`,
    );
    server.listen(PORT, () => {
      // eslint-disable-next-line no-console
      console.log(`gateway-sidecar listening on :${PORT}`);
    });
  }
}
