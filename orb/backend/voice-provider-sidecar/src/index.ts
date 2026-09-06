/**
 * voice-provider-sidecar — the real STT/TTS provider door for the voice pipeline.
 *
 * This is the server side of the contract `backend/relay-rs/src/provider.rs`'s
 * `HttpContractProvider` speaks and tests (`start_contract_server` in that file's test module is
 * the reference fixture; see `contract.ts` for the copied contract doc). Mirrors
 * `backend/gateway-sidecar`'s role for the LLM (C9): this directory is the ONLY place STT/TTS
 * provider HTTP calls happen in this repo.
 *
 *   POST /v1/stt/push        { session_id, audio_bytes: number[] } -> { text: string|null, is_final: bool }
 *   POST /v1/stt/end         { session_id }                        -> { text: string, is_final: true }
 *   POST /v1/tts/synthesize  { text, voice_id, emotion }            -> chunked-transfer body, one
 *                                                                       HTTP chunk per audio chunk,
 *                                                                       each `{ "chunk": number[] }`
 *                                                                       (A7/A9: streamed so the
 *                                                                       relay can forward the first
 *                                                                       chunk before the last one
 *                                                                       has been written — see
 *                                                                       `writeTtsChunks` below and
 *                                                                       `HttpContractProvider::
 *                                                                       synthesize_streaming` in
 *                                                                       relay-rs for the reader).
 *
 * Provider selection (env — REQUIRED, no implicit default):
 *   ORB_STT_PROVIDER = fake | fish | sarvam | deepgram   (keys: FISH_API_KEY | SARVAM_API_KEY | DEEPGRAM_API_KEY)
 *   ORB_TTS_PROVIDER = fake | fish | sarvam | cartesia (keys: FISH_API_KEY | SARVAM_API_KEY | CARTESIA_API_KEY)
 * Neither var defaults to `fake` — an unset var fails closed at boot with a typed
 * `MissingProviderConfigError` (see `config.ts`) rather than silently choosing the fake backend.
 * `fake` must be requested explicitly for local dev/testing, and logs a SYNTHETIC warning when
 * selected. A selected non-fake provider with no key also fails fast at boot (throws
 * synchronously) rather than silently falling back — see `config.ts`.
 *
 * Run it:
 *   ORB_STT_PROVIDER=fake ORB_TTS_PROVIDER=fake npx vite-node backend/voice-provider-sidecar/src/index.ts   # :8083
 *   ORB_STT_PROVIDER=fish FISH_API_KEY=... ORB_TTS_PROVIDER=fake npx vite-node backend/voice-provider-sidecar/src/index.ts
 */
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { createSttBackendFromEnv, createTtsBackendFromEnv } from './config.js';
import { chunkAudioBytes } from './contract.js';
import { extractPcmFromWav } from './wav.js';
import type { SttBackend } from './stt/types.js';
import type { TtsBackend } from './tts/types.js';
import type { SttEndRequest, SttPushRequest, TtsSynthesizeRequest } from './contract.js';

// A client that opens a connection and trickles bytes (or never sends the terminating chunk)
// must not hold this handler — and the socket behind it — open forever. That is the same "the
// stop path never actually released it" shape as the SIGTERM/recordVideo bug, here for an HTTP
// request instead of a subprocess. 1MB is generous for an audio-frame push body; 10s is generous
// against the app's own p99 voice-to-voice target (<=2.0s for the full round trip).
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
  res.writeHead(status, {
    'content-type': 'application/json',
    'content-length': Buffer.byteLength(payload),
  });
  res.end(payload);
}

/**
 * Writes one HTTP chunk per audio chunk, each `{"chunk":[byte,...]}`, terminated by the normal
 * zero-length chunk. No `content-length` header is set, so Node sends this as
 * `Transfer-Encoding: chunked` — that's what makes each `res.write()` reach the socket (and the
 * relay parsing it) as its own frame instead of waiting behind a single buffered body.
 *
 * `HttpContractProvider::synthesize_streaming` (relay-rs) is the reader for this exact shape;
 * do not change it without updating that parser and its tests together.
 */
function writeTtsChunks(res: ServerResponse, chunks: number[][]): void {
  res.writeHead(200, { 'content-type': 'application/json', 'transfer-encoding': 'chunked' });
  for (const chunk of chunks) {
    // The trailing newline is inside this HTTP chunk's payload, not a separate frame — harmless
    // to the Rust reader (serde_json tolerates trailing whitespace) and what lets a test replay
    // the de-chunked body as newline-delimited JSON without re-parsing HTTP framing itself.
    res.write(`${JSON.stringify({ chunk })}\n`);
  }
  res.end();
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

// Route handlers are invoked as `void handler(...)` (fire-and-forget) below. Anything the handler
// itself doesn't catch — a malformed body, the new size/timeout guard above, any other thrown
// error before its own try/catch — becomes an unhandled promise rejection, which crashes the
// whole process by default in Node. One bad request should return 400, not take the sidecar down.
function runHandler(res: ServerResponse, handling: Promise<void>): void {
  handling.catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    // eslint-disable-next-line no-console
    console.error(`voice-provider-sidecar: unhandled request error: ${message}`);
    if (!res.headersSent) sendJson(res, 400, { error: 'BAD_REQUEST', message });
  });
}

export function buildServer(stt: SttBackend, tts: TtsBackend) {
  async function handleSttPush(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const body = (await readJsonBody(req)) as Partial<SttPushRequest>;
    if (!isNonEmptyString(body.session_id) || !Array.isArray(body.audio_bytes)) {
      return sendJson(res, 400, { error: 'INVALID_REQUEST', message: 'session_id and audio_bytes are required' });
    }
    try {
      const frame = Uint8Array.from(body.audio_bytes as number[]);
      const result = await stt.pushAudio(body.session_id, frame);
      sendJson(res, 200, result);
    } catch (err: unknown) {
      sendJson(res, 502, { error: 'STT_PROVIDER_ERROR', message: errorMessage(err) });
    }
  }

  async function handleSttEnd(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const body = (await readJsonBody(req)) as Partial<SttEndRequest>;
    if (!isNonEmptyString(body.session_id)) {
      return sendJson(res, 400, { error: 'INVALID_REQUEST', message: 'session_id is required' });
    }
    try {
      const result = await stt.endTurn(body.session_id);
      sendJson(res, 200, result);
    } catch (err: unknown) {
      sendJson(res, 502, { error: 'STT_PROVIDER_ERROR', message: errorMessage(err) });
    }
  }

  async function handleTtsSynthesize(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const body = (await readJsonBody(req)) as Partial<TtsSynthesizeRequest>;
    if (!isNonEmptyString(body.text) || !isNonEmptyString(body.voice_id) || !isNonEmptyString(body.emotion)) {
      return sendJson(res, 400, {
        error: 'INVALID_REQUEST',
        message: 'text, voice_id, and emotion are required',
      });
    }
    try {
      const rawAudio = await tts.synthesize(body.text, body.voice_id, body.emotion);
      // Fish/Sarvam/Cartesia all request or return a WAV container, not headerless raw PCM — the
      // client (RelayAudioPlayer.ts) assumes raw PCM samples, so an unstripped header plays as an
      // audible click. `wasWav: false` on a real (non-fake) backend means the response wasn't a
      // parseable WAV at all (e.g. a provider defaulting to a compressed codec) — that's a real
      // format mismatch this sidecar cannot fix by itself, only surface.
      const { pcm: audio, wasWav } = extractPcmFromWav(rawAudio);
      if (tts.label !== 'fake' && !wasWav) {
        // eslint-disable-next-line no-console
        console.warn(
          `voice-provider-sidecar: tts backend "${tts.label}" returned a non-WAV response — ` +
            'sending it through unmodified, playback on the client will likely be wrong. ' +
            'Check that backend\'s CONFIDENCE NOTE in src/tts/.',
        );
      }
      // The fake backend matches relay-rs's FakeProvider two-chunk fixture (320 bytes each) so T0
      // parity between the Rust fake path and this sidecar's fake path holds byte-for-byte.
      const chunkSize = tts.label === 'fake' ? 320 : 3200;
      const audio_chunks = chunkAudioBytes(audio, chunkSize);
      writeTtsChunks(res, audio_chunks);
    } catch (err: unknown) {
      sendJson(res, 502, { error: 'TTS_PROVIDER_ERROR', message: errorMessage(err) });
    }
  }

  return createServer((req, res) => {
    if (req.method === 'GET' && req.url === '/healthz') {
      return sendJson(res, 200, { status: 'ok', stt: stt.label, tts: tts.label });
    }
    if (req.method === 'POST' && req.url === '/v1/stt/push') return runHandler(res, handleSttPush(req, res));
    if (req.method === 'POST' && req.url === '/v1/stt/end') return runHandler(res, handleSttEnd(req, res));
    if (req.method === 'POST' && req.url === '/v1/tts/synthesize') {
      return runHandler(res, handleTtsSynthesize(req, res));
    }
    sendJson(res, 404, { error: 'NOT_FOUND' });
  });
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

// Fail fast at import time (mirrors gateway-sidecar's top-level `createGatewayFromEnv()`): a
// misconfigured provider must crash the process, not boot into a silent no-op.
const sttBackend = createSttBackendFromEnv();
const ttsBackend = createTtsBackendFromEnv();

export const server = buildServer(sttBackend, ttsBackend);

const PORT = Number(process.env['VOICE_PROVIDER_SIDECAR_PORT'] ?? 8083);
if (process.env['NODE_ENV'] !== 'test') {
  server.listen(PORT, () => {
    // eslint-disable-next-line no-console
    console.log(
      `voice-provider-sidecar listening on :${PORT} (stt=${sttBackend.label}, tts=${ttsBackend.label})`,
    );
  });
}
