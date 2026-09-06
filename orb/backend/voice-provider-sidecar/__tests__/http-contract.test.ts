import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import type { AddressInfo } from 'node:net';
import { buildServer } from '../src/index';
import { FakeSttBackend } from '../src/stt/fake';
import { FakeTtsBackend } from '../src/tts/fake';

/**
 * Drives the sidecar's actual HTTP server end-to-end against the exact request/response shapes
 * `backend/relay-rs/src/provider.rs`'s `HttpContractProvider` sends/parses (see that file's
 * `start_contract_server` test fixture — this is the JS-side mirror of the same contract).
 */
describe('voice-provider-sidecar HTTP contract', () => {
  let baseUrl: string;
  let close: () => void;

  beforeAll(async () => {
    const server = buildServer(new FakeSttBackend(), new FakeTtsBackend());
    await new Promise<void>((resolve) => server.listen(0, resolve));
    const { port } = server.address() as AddressInfo;
    baseUrl = `http://127.0.0.1:${port}`;
    close = () => server.close();
  });

  afterAll(() => close());

  it('GET /healthz reports the wired backend labels', async () => {
    const res = await fetch(`${baseUrl}/healthz`);
    expect(res.status).toBe(200);
    await expect(res.json()).resolves.toEqual({ status: 'ok', stt: 'fake', tts: 'fake' });
  });

  it('POST /v1/stt/push returns {text:null,is_final:false} then a partial on the 3rd frame', async () => {
    const push = () =>
      fetch(`${baseUrl}/v1/stt/push`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ session_id: 'sess-1', audio_bytes: [1, 2, 3] }),
      }).then((r) => r.json());

    await expect(push()).resolves.toEqual({ text: null, is_final: false });
    await expect(push()).resolves.toEqual({ text: null, is_final: false });
    await expect(push()).resolves.toEqual({ text: 'partial after 3 frames', is_final: false });
  });

  it('POST /v1/stt/push rejects a request missing session_id with 400', async () => {
    const res = await fetch(`${baseUrl}/v1/stt/push`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ audio_bytes: [1] }),
    });
    expect(res.status).toBe(400);
  });

  it('POST /v1/stt/end returns the final transcript contract shape', async () => {
    const res = await fetch(`${baseUrl}/v1/stt/end`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ session_id: 'sess-2' }),
    });
    expect(res.status).toBe(200);
    await expect(res.json()).resolves.toEqual({ text: 'final transcript', is_final: true });
  });

  it('POST /v1/tts/synthesize streams one HTTP chunk per audio chunk, not one whole JSON body', async () => {
    const res = await fetch(`${baseUrl}/v1/tts/synthesize`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text: 'hello', voice_id: 'orb.warm.v1', emotion: 'calm' }),
    });
    expect(res.status).toBe(200);
    expect(res.headers.get('transfer-encoding')).toBe('chunked');
    // A JSON body would satisfy res.json(); a streamed response instead reassembles as
    // newline-delimited JSON, one audio chunk per line — that's the wire-shape assertion.
    const text = await res.text();
    const chunks = text
      .split('\n')
      .filter((line) => line.length > 0)
      .map((line) => (JSON.parse(line) as { chunk: number[] }).chunk);
    expect(chunks).toHaveLength(2);
    expect(chunks[0]).toEqual(new Array(320).fill(0));
    expect(chunks[1]).toEqual(new Array(320).fill(1));
  });

  it('POST /v1/tts/synthesize rejects a request missing voice_id with 400', async () => {
    const res = await fetch(`${baseUrl}/v1/tts/synthesize`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text: 'hello', emotion: 'calm' }),
    });
    expect(res.status).toBe(400);
  });

  it('unknown routes return 404', async () => {
    const res = await fetch(`${baseUrl}/v1/nope`, { method: 'POST' });
    expect(res.status).toBe(404);
  });

  /**
   * readJsonBody throws on malformed JSON before any handler's own try/catch runs. Route
   * dispatch calls handlers as `void handler(...)` (fire-and-forget) — without a catch on that
   * promise, this throw becomes an unhandled rejection, which crashes the whole process by
   * default in Node. A malformed request must return 400 and leave the process (and this test's
   * shared server) alive for the next request, not take the sidecar down.
   */
  it('malformed JSON returns 400 instead of crashing the process', async () => {
    const res = await fetch(`${baseUrl}/v1/stt/push`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: '{not valid json',
    });
    expect(res.status).toBe(400);

    const healthy = await fetch(`${baseUrl}/healthz`);
    expect(healthy.status).toBe(200);
  });
});
