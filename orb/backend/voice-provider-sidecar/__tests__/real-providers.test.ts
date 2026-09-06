import { describe, expect, it, vi } from 'vitest';
import { readFile } from 'node:fs/promises';
import { PROVIDER_REQUEST_TIMEOUT_MS } from '../src/provider-timeout';
import { SarvamSttBackend } from '../src/stt/sarvam';
import { DeepgramSttBackend } from '../src/stt/deepgram';
import type { SttBackend, SttResult } from '../src/stt/types';
import { FishSttBackend } from '../src/stt/fish';
import { MuseSttBackend } from '../src/stt/muse';
import { FishTtsBackend, MAX_SPEAKING_RATE } from '../src/tts/fish';
import { SarvamTtsBackend } from '../src/tts/sarvam';
import { CartesiaTtsBackend } from '../src/tts/cartesia';

/**
 * These test request shaping and response parsing against a mocked `fetch` — there is no real
 * SARVAM_API_KEY / DEEPGRAM_API_KEY / FISH_API_KEY / CARTESIA_API_KEY in this environment, so none
 * of this is live-provider evidence. It proves: (a) the right endpoint/auth header/body shape is
 * sent for the assumptions documented in each backend's file, and (b) the response envelope is
 * parsed into the sidecar's contract shape correctly. See each backend file's CONFIDENCE NOTE for
 * which parts are least certain to be byte-exact against the real API.
 */

function jsonResponse(body: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    json: async () => body,
    text: async () => JSON.stringify(body),
  } as unknown as Response;
}

function binaryResponse(bytes: Uint8Array, ok = true, status = 200): Response {
  return {
    ok,
    status,
    arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
    text: async () => '<binary>',
  } as unknown as Response;
}

async function pushLongTaskFixture(backend: SttBackend): Promise<SttResult[]> {
  const wav = new Uint8Array(
    await readFile(
      new URL('../../../e2e-human-simulator/replay/fixtures/long-task.wav', import.meta.url),
    ),
  );
  const dataMarker = Buffer.from(wav).indexOf('data');
  expect(dataMarker).toBeGreaterThanOrEqual(0);
  const pcm = wav.subarray(dataMarker + 8);
  expect(pcm.byteLength).toBeGreaterThan(5 * 16_000 * 2);

  const events: SttResult[] = [];
  const frameBytes = 3_200; // 100 ms of mono PCM16 at 16 kHz.
  for (let offset = 0; offset < pcm.byteLength; offset += frameBytes) {
    events.push(await backend.pushAudio('streaming-fixture', pcm.subarray(offset, offset + frameBytes)));
  }
  events.push(await backend.endTurn('streaming-fixture'));
  return events;
}

describe('SarvamSttBackend', () => {
  it('buffers push and calls the batch endpoint with the api-subscription-key header on end', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ transcript: 'buffered result' }));
    const backend = new SarvamSttBackend('sarvam-key', fetchMock as unknown as typeof fetch);

    await expect(backend.pushAudio('s1', new Uint8Array([1, 2]))).resolves.toEqual({
      text: null,
      is_final: false,
    });
    const result = await backend.endTurn('s1');

    expect(result).toEqual({ text: 'buffered result', is_final: true });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.sarvam.ai/speech-to-text');
    expect((init.headers as Record<string, string>)['api-subscription-key']).toBe('sarvam-key');
  });

  it('surfaces a non-OK response as a thrown error, not a silent empty transcript', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({}, false, 401));
    const backend = new SarvamSttBackend('bad-key', fetchMock as unknown as typeof fetch);
    await backend.pushAudio('s1', new Uint8Array([1]));
    await expect(backend.endTurn('s1')).rejects.toThrow(/sarvam stt request failed: 401/);
  });
});

describe('DeepgramSttBackend', () => {
  it('sends Token auth and parses the channels[0].alternatives[0].transcript envelope', async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      jsonResponse({ results: { channels: [{ alternatives: [{ transcript: 'nova result' }] }] } }),
    );
    const backend = new DeepgramSttBackend('dg-key', fetchMock as unknown as typeof fetch);
    await backend.pushAudio('s1', new Uint8Array([1, 2, 3]));
    const result = await backend.endTurn('s1');

    expect(result).toEqual({ text: 'nova result', is_final: true });
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain('api.deepgram.com/v1/listen');
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Token dg-key');
  });
});

describe('real STT streaming partial delivery', () => {
  it.each([
    {
      provider: 'sarvam',
      create: (fetchMock: typeof fetch) => new SarvamSttBackend('sarvam-key', fetchMock),
      response: (text: string) => ({ transcript: text }),
    },
    {
      provider: 'deepgram',
      create: (fetchMock: typeof fetch) => new DeepgramSttBackend('deepgram-key', fetchMock),
      response: (text: string) => ({
        results: { channels: [{ alternatives: [{ transcript: text }] }] },
      }),
    },
    {
      provider: 'muse',
      create: (fetchMock: typeof fetch) => new MuseSttBackend('muse-key', fetchMock),
      response: (text: string) => ({ transcript: text, turns: [] }),
    },
  ])('$provider emits a partial before the final for multi-second audio', async ({ create, response }) => {
    let requestCount = 0;
    const fetchMock = vi.fn().mockImplementation(async () => {
      requestCount += 1;
      return jsonResponse(response(requestCount === 1 ? 'early partial' : 'complete final'));
    });
    const events = await pushLongTaskFixture(create(fetchMock as unknown as typeof fetch));

    const partialIndex = events.findIndex((event) => event.text !== null && !event.is_final);
    const finalIndex = events.findIndex((event) => event.text !== null && event.is_final);
    expect(partialIndex).toBeGreaterThanOrEqual(0);
    expect(partialIndex).toBeLessThan(finalIndex);
    expect(events[partialIndex]?.text).toBe('early partial');
    expect(events[finalIndex]?.text).toBe('complete final');
  });
});

describe('FishSttBackend', () => {
  it('buffers PCM, sends Fish multipart ASR fields, and parses the transcript', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ text: 'fish result', duration: 1.2, segments: [] }));
    const backend = new FishSttBackend('fish-key', fetchMock as unknown as typeof fetch);
    await backend.pushAudio('s1', new Uint8Array([1, 2, 3, 4]));

    await expect(backend.endTurn('s1')).resolves.toEqual({ text: 'fish result', is_final: true });
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.fish.audio/v1/asr');
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer fish-key');
    expect(init.body).toBeInstanceOf(FormData);
    const fields = init.body as FormData;
    expect(fields.get('language')).toBe('en');
    expect(fields.get('ignore_timestamps')).toBe('true');
    expect(fields.get('audio')).toBeInstanceOf(File);
  });

  it('surfaces a missing Fish transcript as a provider error', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ duration: 1.2, segments: [] }));
    const backend = new FishSttBackend('fish-key', fetchMock as unknown as typeof fetch);
    await backend.pushAudio('s1', new Uint8Array([1, 2]));
    await expect(backend.endTurn('s1')).rejects.toThrow(/fish audio asr response omitted text/);
  });
});

describe('MuseSttBackend', () => {
  it('buffers PCM, sends the sessionId query param + Bearer auth, and parses the transcript', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ transcript: 'muse result', turns: [] }));
    const backend = new MuseSttBackend('muse-key', fetchMock as unknown as typeof fetch);
    await backend.pushAudio('sess-1', new Uint8Array([1, 2, 3, 4]));

    await expect(backend.endTurn('sess-1')).resolves.toEqual({ text: 'muse result', is_final: true });
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.meta.ai/v1/asr/transcribe?sessionId=sess-1');
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer muse-key');
    expect(init.body).toBeInstanceOf(FormData);
    const fields = init.body as FormData;
    expect(fields.get('audio')).toBeInstanceOf(File);
    expect(fields.get('request')).toBeTruthy();
  });

  it('surfaces a missing Muse transcript as a provider error', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ turns: [] }));
    const backend = new MuseSttBackend('muse-key', fetchMock as unknown as typeof fetch);
    await backend.pushAudio('s1', new Uint8Array([1, 2]));
    await expect(backend.endTurn('s1')).rejects.toThrow(/muse asr response omitted transcript/);
  });

  it('surfaces a non-OK response as a thrown error, not a silent empty transcript', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({}, false, 429));
    const backend = new MuseSttBackend('muse-key', fetchMock as unknown as typeof fetch);
    await backend.pushAudio('s1', new Uint8Array([1]));
    await expect(backend.endTurn('s1')).rejects.toThrow(/muse asr request failed: 429/);
  });
});

describe('FishTtsBackend', () => {
  it('sends Bearer auth and returns the raw response bytes', async () => {
    const audioBytes = new Uint8Array([9, 9, 9]);
    const fetchMock = vi.fn().mockResolvedValue(binaryResponse(audioBytes));
    const backend = new FishTtsBackend('fish-key', fetchMock as unknown as typeof fetch);

    const result = await backend.synthesize('hi', 'voice-a', 'calm');
    expect(Array.from(result)).toEqual([9, 9, 9]);
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.fish.audio/v1/tts');
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer fish-key');
  });

  it('sends the documented quality model header', async () => {
    const fetchMock = vi.fn().mockResolvedValue(binaryResponse(new Uint8Array([1])));
    const backend = new FishTtsBackend('fish-key', fetchMock as unknown as typeof fetch);
    await backend.synthesize('hi', 'voice-a', 'calm');
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect((init.headers as Record<string, string>)['model']).toBe('s2-pro');
  });

  it('maps the selected emotion to bounded Fish prosody controls', async () => {
    const fetchMock = vi.fn().mockResolvedValue(binaryResponse(new Uint8Array([1])));
    const backend = new FishTtsBackend('fish-key', fetchMock as unknown as typeof fetch);
    await backend.synthesize('ship it', 'voice-a', 'celebratory');
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const body = JSON.parse(String(init.body)) as Record<string, unknown>;
    expect(body.temperature).toBe(0.86);
    expect(body.top_p).toBe(0.88);
    expect(body.prosody).toEqual({ speed: 1.0, volume: 2, normalize_loudness: true });
    expect(body.latency).toBe('normal');
    expect(body.chunk_length).toBe(300);
    expect(body.normalize).toBe(true);
    expect(body.condition_on_previous_chunks).toBe(true);
  });

  it('maps the stimulation engage voice to brighter Fish prosody, never faster than natural', async () => {
    const fetchMock = vi.fn().mockResolvedValue(binaryResponse(new Uint8Array([1])));
    const backend = new FishTtsBackend('fish-key', fetchMock as unknown as typeof fetch);
    await backend.synthesize('one small win', 'voice-a', 'upbeat');
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const body = JSON.parse(String(init.body)) as Record<string, unknown>;
    expect(body.prosody).toEqual({ speed: 1.0, volume: 2, normalize_loudness: true });
  });

  it('never speaks faster than natural, for ANY emotion', async () => {
    // The invariant, not the seven numbers. The table used to sit above 1.0 for five of seven
    // emotions (default `warm` at 1.04), which measured 3.4 words/sec of real Fish output against a
    // natural conversational 2.2-3.0 — the owner's "I can not make that out clearly".
    for (const emotion of [
      'warm', 'calm', 'curious', 'upbeat', 'celebratory', 'gentle', 'matter_of_fact', 'unknown-emotion',
    ]) {
      const fetchMock = vi.fn().mockResolvedValue(binaryResponse(new Uint8Array([1])));
      const backend = new FishTtsBackend('fish-key', fetchMock as unknown as typeof fetch);
      await backend.synthesize('one small step', 'voice-a', emotion);
      const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
      const body = JSON.parse(String(init.body)) as Record<string, unknown>;
      const prosody = body.prosody as { speed: number };
      expect(prosody.speed, `${emotion} speaks faster than natural`).toBeLessThanOrEqual(
        MAX_SPEAKING_RATE,
      );
      // And not so slow it sounds sedated — a floor keeps a retune honest in both directions.
      expect(prosody.speed, `${emotion} is unnaturally slow`).toBeGreaterThanOrEqual(0.85);
    }
  });

  it('passes Fish inline emotion markup through unchanged', async () => {
    const fetchMock = vi.fn().mockResolvedValue(binaryResponse(new Uint8Array([1])));
    const backend = new FishTtsBackend('fish-key', fetchMock as unknown as typeof fetch);
    const markedText = '[chuckle] Nice. [emphasis]One small move. [long pause] Then we pause.';

    await backend.synthesize(markedText, 'voice-a', 'warm');

    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const body = JSON.parse(String(init.body)) as Record<string, unknown>;
    expect(body.text).toBe(markedText);
  });

  it('adds bounded pacing to bare local speech while preserving any selected Fish voice id', async () => {
    const fetchMock = vi.fn().mockResolvedValue(binaryResponse(new Uint8Array([1])));
    const backend = new FishTtsBackend('fish-key', fetchMock as unknown as typeof fetch);

    await backend.synthesize('First step. Then we pause.', '59e9dc1cb20c452584788a2690c80970', 'upbeat');

    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const body = JSON.parse(String(init.body)) as Record<string, unknown>;
    expect(body.reference_id).toBe('59e9dc1cb20c452584788a2690c80970');
    expect(body.text).toBe('[inhale] [excited] First step. [short pause] Then we pause.');
  });
});

describe('SarvamTtsBackend', () => {
  it('decodes the base64 audios[0] envelope into raw bytes', async () => {
    const base64 = Buffer.from([1, 2, 3]).toString('base64');
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ audios: [base64] }));
    const backend = new SarvamTtsBackend('sarvam-key', fetchMock as unknown as typeof fetch);

    const result = await backend.synthesize('hi', 'speaker-a', 'calm');
    expect(Array.from(result)).toEqual([1, 2, 3]);
  });

  it('throws instead of returning empty audio when the provider omits audios', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({}));
    const backend = new SarvamTtsBackend('sarvam-key', fetchMock as unknown as typeof fetch);
    await expect(backend.synthesize('hi', 'speaker-a', 'calm')).rejects.toThrow(/omitted audio/);
  });
});

describe('CartesiaTtsBackend', () => {
  it('sends X-API-Key and the Cartesia-Version header and returns raw bytes', async () => {
    const audioBytes = new Uint8Array([5, 6, 7, 8]);
    const fetchMock = vi.fn().mockResolvedValue(binaryResponse(audioBytes));
    const backend = new CartesiaTtsBackend('cartesia-key', fetchMock as unknown as typeof fetch);

    const result = await backend.synthesize('hi', 'voice-a', 'calm');
    expect(Array.from(result)).toEqual([5, 6, 7, 8]);
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers['X-API-Key']).toBe('cartesia-key');
    expect(headers['Cartesia-Version']).toBeTruthy();
  });
});

/**
 * A stalled TLS handshake or a dead connection to any of these providers would otherwise hold the
 * awaiting request handler open forever — the same "the stop path never actually released it"
 * shape as the SIGTERM/recordVideo bug, here for a socket instead of a subprocess. Every backend
 * must attach a bounded AbortSignal to its provider request.
 */
describe('provider requests never hang forever', () => {
  it('PROVIDER_REQUEST_TIMEOUT_MS is a sane positive, finite bound', () => {
    expect(Number.isFinite(PROVIDER_REQUEST_TIMEOUT_MS)).toBe(true);
    expect(PROVIDER_REQUEST_TIMEOUT_MS).toBeGreaterThan(0);
  });

  it('every STT backend attaches an AbortSignal to its provider request', async () => {
    const sarvamFetch = vi.fn().mockResolvedValue(jsonResponse({ transcript: 'x' }));
    const sarvam = new SarvamSttBackend('k', sarvamFetch as unknown as typeof fetch);
    await sarvam.pushAudio('s', new Uint8Array([1]));
    await sarvam.endTurn('s');
    expect((sarvamFetch.mock.calls[0]![1] as RequestInit).signal).toBeInstanceOf(AbortSignal);

    const deepgramFetch = vi.fn().mockResolvedValue(
      jsonResponse({ results: { channels: [{ alternatives: [{ transcript: 'x' }] }] } }),
    );
    const deepgram = new DeepgramSttBackend('k', deepgramFetch as unknown as typeof fetch);
    await deepgram.pushAudio('s', new Uint8Array([1]));
    await deepgram.endTurn('s');
    expect((deepgramFetch.mock.calls[0]![1] as RequestInit).signal).toBeInstanceOf(AbortSignal);

    const fishFetch = vi.fn().mockResolvedValue(jsonResponse({ text: 'x' }));
    const fish = new FishSttBackend('k', fishFetch as unknown as typeof fetch);
    await fish.pushAudio('s', new Uint8Array([1]));
    await fish.endTurn('s');
    expect((fishFetch.mock.calls[0]![1] as RequestInit).signal).toBeInstanceOf(AbortSignal);

    const museFetch = vi.fn().mockResolvedValue(jsonResponse({ transcript: 'x', turns: [] }));
    const muse = new MuseSttBackend('k', museFetch as unknown as typeof fetch);
    await muse.pushAudio('s', new Uint8Array([1]));
    await muse.endTurn('s');
    expect((museFetch.mock.calls[0]![1] as RequestInit).signal).toBeInstanceOf(AbortSignal);
  });

  it('every TTS backend attaches an AbortSignal to its provider request', async () => {
    const fishTtsFetch = vi.fn().mockResolvedValue(binaryResponse(new Uint8Array([1])));
    const fishTts = new FishTtsBackend('k', fishTtsFetch as unknown as typeof fetch);
    await fishTts.synthesize('hi', 'voice-a', 'calm');
    expect((fishTtsFetch.mock.calls[0]![1] as RequestInit).signal).toBeInstanceOf(AbortSignal);

    const sarvamTtsFetch = vi.fn().mockResolvedValue(jsonResponse({ audios: [Buffer.from([1]).toString('base64')] }));
    const sarvamTts = new SarvamTtsBackend('k', sarvamTtsFetch as unknown as typeof fetch);
    await sarvamTts.synthesize('hi', 'speaker-a', 'calm');
    expect((sarvamTtsFetch.mock.calls[0]![1] as RequestInit).signal).toBeInstanceOf(AbortSignal);

    const cartesiaFetch = vi.fn().mockResolvedValue(binaryResponse(new Uint8Array([1])));
    const cartesia = new CartesiaTtsBackend('k', cartesiaFetch as unknown as typeof fetch);
    await cartesia.synthesize('hi', 'voice-a', 'calm');
    expect((cartesiaFetch.mock.calls[0]![1] as RequestInit).signal).toBeInstanceOf(AbortSignal);
  });
});
