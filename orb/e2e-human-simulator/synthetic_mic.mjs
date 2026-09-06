/**
 * Prove the SPOKEN path end to end without a human in the room.
 *
 * Why this exists: the owner reported "speaking, no reply" three times while 594 TS tests, 250
 * Python tests and a green backend gate all passed. Nothing in any suite covered whether the
 * realtime chain a real utterance travels through was even connected — and it was not
 * (`relay-rs` absent, then running `ORB_RELAY_PROVIDER=fake`). A unit test could not have caught
 * that, and neither could the HTTP-only `evals/load_direction/drive.py`, which skips the WebSocket,
 * the STT provider and the audio transport entirely.
 *
 * What it does: synthesises a real utterance with the live Fish TTS, then streams that PCM into
 * relay-rs as raw binary frames exactly the way `apps/mobile/src/voice/RelayClient.ts` does
 * (`start_listening` -> binary audio -> `end_of_turn`), and reports what comes back.
 *
 * This is a TTS -> STT round trip, so it is NOT proof that a human voice transcribes well — real
 * speech has accents, noise and disfluency that synthetic speech does not. It IS proof that the
 * transport, the STT provider, the relay, the model and the reply path are all connected and
 * carrying real content, which is exactly what was broken.
 *
 * It closes the WHOLE loop the way the app does: audio -> transcript -> POST /v1/respond -> `speak`
 * -> synthesised audio back over the socket. The app owns the middle leg (RelayClient only
 * transports), so a client that stops at the transcript proves half the product.
 *
 * Run (whole chain up — scripts/dev.sh):
 *   node e2e-human-simulator/synthetic_mic.mjs
 * Exit 0 only if a real transcript came back AND a real model reply came back as audible audio.
 */

const VOICE_URL = process.env.ORB_VOICE_URL ?? 'http://127.0.0.1:8083';
const RELAY_WS = process.env.ORB_RELAY_WS_URL ?? 'ws://127.0.0.1:8091';
const RELAY_HTTP = process.env.ORB_RELAY_HTTP_URL ?? 'http://127.0.0.1:8765';
const TENANT = 't0';
const SESSION = `synthetic-mic-${Date.now()}`;
const UTTERANCE = 'My kitchen is a total disaster and I cannot get started on it.';

// The fake provider's canned transcripts. If any of these come back, the chain is connected to a
// stub and the run is a FAILURE dressed as a success — this is the exact trap that let three
// "speaking, no reply" reports coexist with a fully green test suite.
const FAKE_MARKERS = ['the final transcript', 'fake-t0', 'partial transcript'];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function synthesize(text) {
  const res = await fetch(`${VOICE_URL}/v1/tts/synthesize`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      tenant_id: TENANT,
      session_id: SESSION,
      text,
      voice_id: 'orb.warm.v1',
      emotion: 'warm',
    }),
  });
  if (!res.ok) throw new Error(`TTS ${res.status}: ${(await res.text()).slice(0, 200)}`);
  const body = await res.json();
  const chunks = body.audio_chunks ?? [];
  if (chunks.length === 0) throw new Error('TTS returned no audio chunks');
  const bytes = Buffer.concat(chunks.map((c) => Buffer.from(c)));
  // Reject silence explicitly: a payload of zeros would make the STT leg look broken when the
  // real fault was upstream. Measuring the wrong quantity is how this build has gone wrong before.
  let peak = 0;
  for (let i = 0; i + 1 < bytes.length; i += 2) {
    const s = bytes.readInt16LE(i);
    if (Math.abs(s) > peak) peak = Math.abs(s);
  }
  if (peak < 1000) throw new Error(`TTS audio is silence (peak ${peak}) — nothing to transcribe`);
  return { bytes, peak };
}

async function main() {
  console.log(`[synthetic-mic] session=${SESSION}`);
  const { bytes, peak } = await synthesize(UTTERANCE);
  console.log(`[synthetic-mic] TTS ok: ${bytes.length} bytes, peak ${peak}`);

  const ws = new WebSocket(RELAY_WS);
  const inbound = [];
  let closedReason = null;
  let audioBytesBack = 0;
  let replyText = null;
  let respondError = null;
  ws.binaryType = 'arraybuffer';
  ws.onmessage = (event) => {
    if (typeof event.data !== 'string') {
      audioBytesBack += event.data.byteLength ?? 0;
      inbound.push('<binary audio>');
      return;
    }
    inbound.push(event.data);
    console.log(`[relay-rs -> me] ${event.data.slice(0, 220)}`);
    // This is the leg the APP owns: RelayClient only transports, so nothing replies unless a
    // client turns the final transcript into a /v1/respond call and sends `speak` back.
    let frame;
    try { frame = JSON.parse(event.data); } catch { return; }
    if (frame.type !== 'transcript' || frame.is_final !== true) return;
    void (async () => {
      try {
        const res = await fetch(`${RELAY_HTTP}/v1/respond`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            tenant_id: TENANT,
            user_id: 'synthetic-mic-user',
            session_id: SESSION,
            text: frame.text,
            mode: 'converse',
          }),
        });
        if (!res.ok) {
          respondError = `respond ${res.status}: ${(await res.text()).slice(0, 200)}`;
          return;
        }
        const body = await res.json();
        replyText = body.text ?? body.reply ?? null;
        console.log(`[orb reply] ${replyText}`);
        console.log(`[orb reply] source=${body.source} degraded=${body.degraded}`);
        if (replyText && ws.readyState === 1) {
          ws.send(JSON.stringify({
            type: 'speak',
            tenant_id: TENANT,
            session_id: SESSION,
            text: replyText,
            voice_id: 'orb.warm.v1',
            emotion: 'warm',
          }));
        }
      } catch (error) {
        respondError = error instanceof Error ? error.message : String(error);
      }
    })();
  };
  ws.onclose = (event) => {
    closedReason = `code=${event.code} reason=${event.reason || '(none)'}`;
  };

  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = () => reject(new Error(`WebSocket error connecting to ${RELAY_WS}`));
    setTimeout(() => reject(new Error(`timed out connecting to ${RELAY_WS}`)), 8000);
  });
  console.log('[synthetic-mic] connected');

  ws.send(JSON.stringify({ type: 'start_listening', tenant_id: TENANT, session_id: SESSION }));
  await sleep(250);

  // 20 ms of PCM16 mono at 44.1 kHz ~= 1764 bytes. Chunk size and pacing mimic a real mic rather
  // than dumping the whole utterance at once, because a provider that streams will behave
  // differently under a single huge frame and that difference is the thing under test.
  const CHUNK = 1764;
  for (let offset = 0; offset < bytes.length; offset += CHUNK) {
    if (ws.readyState !== 1) break;
    ws.send(bytes.subarray(offset, Math.min(offset + CHUNK, bytes.length)));
    await sleep(10);
  }
  console.log(`[synthetic-mic] streamed ${bytes.length} bytes as ${Math.ceil(bytes.length / CHUNK)} frames`);

  ws.send(JSON.stringify({ type: 'end_of_turn', tenant_id: TENANT, session_id: SESSION }));
  await sleep(12000); // give STT + model + TTS the full turn
  try { ws.close(); } catch { /* already closed */ }
  await sleep(300);

  const text = inbound.join('\n');
  const transcripts = inbound.filter((m) => String(m).includes('transcript'));
  const fake = FAKE_MARKERS.filter((m) => text.toLowerCase().includes(m));

  console.log('\n=== VERDICT ===');
  console.log(`frames received       : ${inbound.length}`);
  console.log(`transcript frames     : ${transcripts.length}`);
  console.log(`fake-provider markers : ${fake.length ? fake.join(', ') : 'none'}`);
  console.log(`orb reply             : ${replyText ? JSON.stringify(replyText.slice(0, 140)) : 'NONE'}`);
  console.log(`reply audio back      : ${audioBytesBack} bytes`);
  if (respondError) console.log(`respond error         : ${respondError}`);
  if (closedReason) console.log(`socket closed         : ${closedReason}`);

  if (inbound.length === 0) {
    console.log('FAIL: relay-rs accepted audio and sent nothing back — the spoken path is dead.');
    return 6;
  }
  if (fake.length > 0) {
    console.log('FAIL: a FAKE transcript came back. The chain is connected to a stub provider.');
    return 6;
  }
  if (transcripts.length === 0) {
    console.log('FAIL: frames came back but none carried a transcript — STT did not run.');
    return 6;
  }
  if (respondError) {
    console.log(`FAIL: the transcript never became a reply — ${respondError}`);
    return 6;
  }
  if (!replyText) {
    console.log('FAIL: transcript came back but no model reply — the answer leg is broken.');
    return 6;
  }
  if (audioBytesBack === 0) {
    // A reply the user cannot HEAR is not a reply. This is the proxy-vs-property line for a voice
    // product: text in a log is not speech in the room.
    console.log('FAIL: a reply was generated but no audio came back — the user would hear silence.');
    return 6;
  }
  console.log('PASS: real audio in -> real transcript -> real reply -> audible audio out.');
  return 0;
}

main().then(
  (code) => process.exit(code),
  (error) => {
    console.error(`[synthetic-mic] ERROR ${error.message}`);
    process.exit(1);
  },
);
