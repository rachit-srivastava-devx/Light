#!/usr/bin/env node
/**
 * End-to-end HAPPY PATH drive of the full voice pipeline as it exists on main right now,
 * exercising the real compiled artifacts — not vitest mocks, not unit-level fakes injected in
 * process. This boots the actual `relay-rs` binary (`cargo run`) and the actual
 * `voice-provider-sidecar` process (the same entrypoint `scripts/dev.sh` starts in production),
 * wired together over real localhost TCP with an EXPLICIT fake provider backend on both sides
 * (per A9: there is no implicit "fake" default any more — this script sets
 * `ORB_RELAY_PROVIDER=http` + `ORB_RELAY_PROVIDER_URL=...` and `ORB_STT_PROVIDER=fake` /
 * `ORB_TTS_PROVIDER=fake` explicitly, exactly the fail-closed contract A9 introduced). A real
 * WebSocket client (the `ws` package, not the mobile app) drives the connection using a real WAV
 * fixture from `e2e-human-simulator/replay/fixtures/` as microphone audio.
 *
 * Covered, against the real stack:
 *   A2  relay confirms listening         -> `ListeningConfirmed` before any transcript
 *   A5  STT streams partial then final   -> a `Transcript{is_final:false}` arrives before the
 *                                            `Transcript{is_final:true}` that `end_of_turn` forces
 *   A7  TTS streams in real chunks       -> two separate binary WS frames arrive in order, not
 *                                            one buffered blob, via the real HTTP chunked-transfer
 *                                            hop between relay-rs and the sidecar (A9's own fix)
 *   A8  barge-in actually cancels        -> a `barge_in` sent immediately after `speak` retires
 *                                            the in-flight generation: ZERO binary audio for that
 *                                            turn reaches the client even though the (already
 *                                            dispatched) fake-provider HTTP call keeps running to
 *                                            completion in the background — proven by asserting no
 *                                            stray frames land in a bounded drain window
 *   A1  no hardcoded transcript          -> implicit: the transcript this script asserts on is the
 *                                            fake STT provider's OWN output, not a value this
 *                                            script or the app pre-seeded
 *   A9  fail-closed provider config      -> this script's own boot fails loudly (not silently) if
 *                                            either process is started without its explicit
 *                                            provider env var — see `spawnRelay`/`spawnSidecar`
 *
 * Deliberately OUT OF SCOPE, stated honestly rather than silently implied-solved:
 *   - The LLM/gateway "brain" (`backend/gateway-sidecar`, `@pe/llm-gateway`) is NOT exercised.
 *     This drives relay-rs's voice TRANSPORT (STT -> TTS) directly, the same layer A2/A5/A7/A8/A9
 *     all live in; the reply text sent as `speak` is computed by this script, standing in for
 *     the app's own gateway call (mirrors what `conversation_drive.mjs` does via a real
 *     `/v1/respond` call, which this script skips because gateway-sidecar has a separate,
 *     pre-existing, unrelated module-resolution break documented in FLEET-LEARNINGS.md).
 *   - No physical device or iOS Simulator is used — no client-side AEC/mic capture/speaker
 *     playback. This is the relay/sidecar transport layer only, driven by a headless WS client.
 *   - Only ~13k PCM samples (under 1s) of real fixture audio are streamed; the fake STT provider's
 *     transcript content ("partial after N frames" / "final transcript") is synthetic by design
 *     (that is the whole point of `fake` — see A9's doc comment) and is asserted as exactly that,
 *     not mistaken for real speech recognition.
 *   - TTS timing: the fake TTS backend returns its fixed 640-byte, two-chunk payload effectively
 *     instantly (no real synthesis latency), so "streaming" is proven structurally (two distinct
 *     WS binary frames, in order, over a real chunked-HTTP hop) rather than by a meaningful
 *     first-chunk-arrives-early timing gap — a real provider's multi-second synthesis is what
 *     makes that gap human-observable (see `barge_in_drive.mjs`'s own note on this).
 *
 * Run:
 *   node e2e-human-simulator/voice_happy_path_drive.mjs
 * Exit 0 only if every check passes; exit 6 lists which failed; exit 1 on a hard/infra error.
 */

import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import WebSocket from 'ws';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

const RELAY_ADDR = process.env.ORB_RELAY_TEST_ADDR ?? '127.0.0.1:8191';
const SIDECAR_PORT = process.env.ORB_VOICE_SIDECAR_TEST_PORT ?? '8183';
const RELAY_WS_URL = `ws://${RELAY_ADDR}`;
const SIDECAR_HTTP_URL = `http://127.0.0.1:${SIDECAR_PORT}`;

const TENANT = 't0';
const SESSION = `voice-happy-path-${Date.now()}`;
const FIXTURE_WAV = join(ROOT, 'e2e-human-simulator/replay/fixtures/hello-how-are-you.wav');
const MIC_FRAME_BYTES = 640; // 20ms of 16kHz mono PCM16, a plausible real mic-frame size

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// --- WAV parsing: a real fixture, not synthetic zero-filled bytes -----------------------------

/** Walks RIFF sub-chunks to find `data`, independent of header quirks (extra fmt fields etc). */
function extractPcmFromWav(buffer) {
  if (buffer.toString('ascii', 0, 4) !== 'RIFF' || buffer.toString('ascii', 8, 12) !== 'WAVE') {
    throw new Error(`${FIXTURE_WAV} is not a RIFF/WAVE file`);
  }
  let offset = 12;
  while (offset + 8 <= buffer.length) {
    const chunkId = buffer.toString('ascii', offset, offset + 4);
    const chunkSize = buffer.readUInt32LE(offset + 4);
    const bodyStart = offset + 8;
    if (chunkId === 'data') {
      return buffer.subarray(bodyStart, bodyStart + chunkSize);
    }
    offset = bodyStart + chunkSize + (chunkSize % 2); // chunks are word-aligned
  }
  throw new Error(`${FIXTURE_WAV} has no data chunk`);
}

function chunk(buffer, size) {
  const frames = [];
  for (let o = 0; o < buffer.length; o += size) {
    frames.push(buffer.subarray(o, Math.min(o + size, buffer.length)));
  }
  return frames;
}

// --- process management -------------------------------------------------------------------------

const children = [];

function spawnLogged(label, command, args, env, cwd) {
  const child = spawn(command, args, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.on('data', (buf) => process.stdout.write(`[${label}] ${buf}`));
  child.stderr.on('data', (buf) => process.stderr.write(`[${label}] ${buf}`));
  children.push({ label, child });
  return child;
}

function spawnRelay() {
  // A9 fail-closed contract: ORB_RELAY_PROVIDER has NO implicit default. Setting it here
  // explicitly (to 'http', pointed at the real sidecar) is the point of this script, not an
  // incidental config value — omitting it is the exact regression A9 fixed relay-rs's boot
  // against.
  return spawnLogged(
    'relay-rs',
    'cargo',
    ['run', '--quiet'],
    {
      ...process.env,
      ORB_RELAY_ADDR: RELAY_ADDR,
      ORB_RELAY_PROVIDER: 'http',
      ORB_RELAY_PROVIDER_URL: SIDECAR_HTTP_URL,
      CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? '/tmp/focus-orb-cargo-target',
      PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH ?? ''}`,
    },
    join(ROOT, 'backend/relay-rs'),
  );
}

function spawnSidecar() {
  // Same A9 contract on the sidecar side: ORB_STT_PROVIDER / ORB_TTS_PROVIDER must be set
  // explicitly. 'fake' is requested here on purpose (no real provider credentials in this
  // environment / no intent to spend real STT/TTS credit for a transport-layer test) — the
  // sidecar itself prints a SYNTHETIC warning for each when it boots, which this script's log
  // output below will show.
  return spawnLogged(
    'voice-sidecar',
    join(ROOT, 'node_modules/.bin/vite-node'),
    ['backend/voice-provider-sidecar/src/index.ts'],
    {
      ...process.env,
      VOICE_PROVIDER_SIDECAR_PORT: SIDECAR_PORT,
      ORB_STT_PROVIDER: 'fake',
      ORB_TTS_PROVIDER: 'fake',
    },
    ROOT,
  );
}

function killAll() {
  for (const { label, child } of children) {
    try {
      child.kill('SIGINT');
    } catch (err) {
      console.error(`[drive] failed to signal ${label}: ${err.message}`);
    }
  }
}

async function waitHttpHealthy(url, tries = 80) {
  for (let attempt = 0; attempt < tries; attempt++) {
    try {
      const res = await fetch(`${url}/healthz`);
      if (res.ok) return;
    } catch {
      /* not up yet */
    }
    await sleep(250);
  }
  throw new Error(`${url}/healthz never returned 200`);
}

async function connectWithRetry(url, tries = 80) {
  let lastError;
  for (let attempt = 0; attempt < tries; attempt++) {
    try {
      const socket = new WebSocket(url);
      await new Promise((resolve, reject) => {
        socket.once('open', resolve);
        socket.once('error', reject);
      });
      return socket;
    } catch (error) {
      lastError = error;
      await sleep(250);
    }
  }
  throw lastError ?? new Error(`${url} never accepted a connection`);
}

// --- inbox: a queue + waiters so `await inbox.next()` reads frames in arrival order -------------

function createInbox(socket) {
  const queue = [];
  const waiters = [];
  let failure = null;
  socket.on('message', (data, isBinary) => {
    const message = {
      isBinary: isBinary || Buffer.isBuffer(data),
      data,
      ts: Date.now(),
    };
    const waiter = waiters.shift();
    if (waiter) waiter.resolve(message);
    else queue.push(message);
  });
  socket.on('error', (error) => {
    failure = error;
    for (const waiter of waiters.splice(0)) waiter.reject(error);
  });
  socket.on('close', () => {
    failure = failure ?? new Error('socket closed');
  });
  return {
    next(timeoutMs = 8000) {
      if (queue.length > 0) return Promise.resolve(queue.shift());
      if (failure) return Promise.reject(failure);
      return new Promise((resolve, reject) => {
        const waiter = {
          resolve: (m) => { clearTimeout(timer); resolve(m); },
          reject: (e) => { clearTimeout(timer); reject(e); },
        };
        // Bug (found by this session's fleet in a sibling drive script's identical copy of this
        // inbox): a timed-out waiter that isn't removed from `waiters` stays in the queue. A
        // later real message then gets shifted to that stale, already-rejected waiter — whose
        // resolve() on an already-settled promise is a silent no-op — instead of to whichever
        // waiter is actually still pending, and the message vanishes. Must splice it out here.
        const timer = setTimeout(() => {
          const idx = waiters.indexOf(waiter);
          if (idx !== -1) waiters.splice(idx, 1);
          reject(new Error(`no frame within ${timeoutMs}ms`));
        }, timeoutMs);
        waiters.push(waiter);
      });
    },
    /** Drains whatever arrives within `windowMs`, without requiring anything to arrive at all. */
    async drain(windowMs) {
      const collected = [];
      const deadline = Date.now() + windowMs;
      for (;;) {
        const remaining = deadline - Date.now();
        if (remaining <= 0) return collected;
        try {
          collected.push(await this.next(remaining));
        } catch {
          return collected; // timeout is expected here — absence is the thing being measured
        }
      }
    },
  };
}

function parseFrame(message) {
  if (message.isBinary) return { kind: 'binary', bytes: message.data.byteLength ?? message.data.length };
  return { kind: 'text', ...JSON.parse(message.data.toString('utf8')) };
}

function sendJson(socket, frame) {
  socket.send(JSON.stringify(frame));
}

// --- checks ---------------------------------------------------------------------------------

const failures = [];
function check(label, passed, detail) {
  console.log(`${passed ? 'PASS' : 'FAIL'}  ${label}${detail ? ` — ${detail}` : ''}`);
  if (!passed) failures.push(label);
}

async function main() {
  console.log(`[drive] session=${SESSION}`);
  console.log(`[drive] relay-rs at ${RELAY_WS_URL} (ORB_RELAY_PROVIDER=http -> ${SIDECAR_HTTP_URL})`);
  console.log(`[drive] voice-provider-sidecar at ${SIDECAR_HTTP_URL} (ORB_STT_PROVIDER=fake, ORB_TTS_PROVIDER=fake)`);

  const wav = readFileSync(FIXTURE_WAV);
  const pcm = extractPcmFromWav(wav);
  console.log(`[drive] fixture ${FIXTURE_WAV}: ${pcm.length} PCM bytes`);

  spawnSidecar();
  await waitHttpHealthy(SIDECAR_HTTP_URL);
  console.log('[drive] sidecar healthy');

  spawnRelay();
  const socket = await connectWithRetry(RELAY_WS_URL);
  console.log('[drive] relay-rs accepted a websocket connection');
  const inbox = createInbox(socket);

  // --- boot: no hardcoded transcript (A1) is implicit — nothing has been sent yet and no
  // transcript exists until real frames are pushed below.

  // --- A2: relay confirms listening --------------------------------------------------------
  sendJson(socket, { type: 'start_listening', tenant_id: TENANT, session_id: SESSION });
  const listening = parseFrame(await inbox.next());
  check(
    'A2 relay sends ListeningConfirmed before anything else',
    listening.type === 'listening_confirmed' && listening.tenant_id === TENANT && listening.session_id === SESSION,
    JSON.stringify(listening),
  );

  // --- A5: STT streams a partial, then a final, transcript ---------------------------------
  const micFrames = chunk(pcm, MIC_FRAME_BYTES);
  console.log(`[drive] streaming ${micFrames.length} mic frames (${MIC_FRAME_BYTES}B each) of real fixture audio`);
  let partial = null;
  for (const frame of micFrames) {
    if (socket.readyState !== WebSocket.OPEN) break;
    socket.send(frame);
    await sleep(5);
    // Fake STT emits a partial every 3rd pushed frame — read eagerly so a partial that arrives
    // mid-stream isn't left in the inbox queue when we later ask for the final transcript.
  }
  // Read whatever text frames have queued up looking for the partial (there may be several).
  const preFinal = [];
  for (;;) {
    let msg;
    try { msg = parseFrame(await inbox.next(1500)); } catch { break; }
    preFinal.push(msg);
    if (msg.kind === 'text' && msg.type === 'transcript' && msg.is_final === false) {
      partial = msg;
    }
  }
  check(
    'A5 a partial transcript (is_final:false) arrives before end_of_turn',
    partial !== null && typeof partial.text === 'string' && partial.text.length > 0,
    partial ? JSON.stringify(partial) : `no partial frame in ${preFinal.length} frame(s): ${JSON.stringify(preFinal)}`,
  );

  sendJson(socket, { type: 'end_of_turn', tenant_id: TENANT, session_id: SESSION });
  const finalFrame = parseFrame(await inbox.next());
  check(
    'A5 end_of_turn produces a final transcript (is_final:true), distinct from the partial',
    finalFrame.kind === 'text' && finalFrame.type === 'transcript' && finalFrame.is_final === true
      && (partial === null || finalFrame.text !== partial.text),
    JSON.stringify(finalFrame),
  );

  // --- "orb responds": this script stands in for the LLM/gateway layer (out of scope — see
  // header comment) and picks a deterministic reply for the transcript it just received.
  const heard = finalFrame.kind === 'text' ? finalFrame.text : '(no transcript)';
  const reply = `You said: ${heard}. Here is a real spoken reply from the orb.`;
  console.log(`[drive] heard "${heard}" -> orb replies "${reply}"`);

  // --- A8: user barges in mid-response and it actually cancels the in-flight call ----------
  // Send `speak`, then IMMEDIATELY (no await in between, same synchronous tick) send `barge_in`.
  // relay-rs's read loop processes one inbound frame per iteration and dispatches `speak`'s
  // provider call onto a background task before it can read the next frame, so `barge_in` is
  // guaranteed to be handled strictly after that dispatch, but the dispatch itself is a real
  // localhost HTTP round trip to the sidecar — slower than parsing an already-buffered second
  // control frame — so this reliably lands the cancellation before the provider's chunks reach
  // write_loop (backend/relay-rs/src/main.rs's generation-tagged drop rule is what actually
  // enforces this, not luck; see the module comment above `write_loop`).
  sendJson(socket, {
    type: 'speak',
    tenant_id: TENANT,
    session_id: SESSION,
    text: reply,
    voice_id: 'orb.warm.v1',
    emotion: 'warm',
  });
  sendJson(socket, { type: 'barge_in', tenant_id: TENANT, session_id: SESSION });

  const bargeInFrames = [];
  for (let i = 0; i < 2; i++) {
    bargeInFrames.push(parseFrame(await inbox.next()));
  }
  // Bounded drain: if the (already-dispatched) fake provider's chunks were going to leak through
  // despite the barge-in, they would arrive within a few hundred ms of a same-host HTTP round
  // trip. Absence here is the actual finding, not an assumption.
  const strayFrames = await inbox.drain(1000).then((raw) => raw.map(parseFrame));

  const [afterSpeak1, afterSpeak2] = bargeInFrames;
  check(
    'A8 speech_starting is sent before the barge-in is processed',
    afterSpeak1.kind === 'text' && afterSpeak1.type === 'speech_starting',
    JSON.stringify(afterSpeak1),
  );
  check(
    'A8 barge-in is acknowledged with an immediate speech_complete',
    afterSpeak2.kind === 'text' && afterSpeak2.type === 'speech_complete',
    JSON.stringify(afterSpeak2),
  );
  check(
    'A8 NO binary audio for the cancelled generation ever reaches the client',
    strayFrames.every((f) => f.kind !== 'binary'),
    strayFrames.length ? `leaked frames: ${JSON.stringify(strayFrames)}` : 'nothing leaked in a 1000ms drain window',
  );
  check(
    'A8 exactly one speech_complete for the cancelled turn (not a second, delayed one)',
    strayFrames.filter((f) => f.kind === 'text' && f.type === 'speech_complete').length === 0,
    strayFrames.length ? JSON.stringify(strayFrames) : 'no extra speech_complete arrived',
  );

  // --- A7 + session-recovery: a fresh (non-barged) speak must still stream real, separate
  // audio chunks and complete normally — proving both "streams in real chunks, not buffered"
  // and that the session survived the barge-in rather than wedging.
  const recoveryText = 'All good, the orb is still listening.';
  sendJson(socket, {
    type: 'speak',
    tenant_id: TENANT,
    session_id: SESSION,
    text: recoveryText,
    voice_id: 'orb.warm.v1',
    emotion: 'calm',
  });
  const starting = parseFrame(await inbox.next());
  const chunk1 = parseFrame(await inbox.next());
  const chunk2 = parseFrame(await inbox.next());
  const complete = parseFrame(await inbox.next());
  check('A7 speech_starting sent before any audio', starting.kind === 'text' && starting.type === 'speech_starting', JSON.stringify(starting));
  check('A7 first TTS chunk arrives as its own binary WS frame', chunk1.kind === 'binary' && chunk1.bytes === 320, JSON.stringify(chunk1));
  check('A7 second TTS chunk arrives as its own, separate binary WS frame (not merged with the first)', chunk2.kind === 'binary' && chunk2.bytes === 320, JSON.stringify(chunk2));
  check('A7 speech_complete follows both real chunks, over the real HTTP-chunked relay<->sidecar hop', complete.kind === 'text' && complete.type === 'speech_complete', JSON.stringify(complete));
  // Deliberately not a separate "recovery" check: the four A7 assertions immediately above ARE
  // the recovery proof — they only pass if this second `speak`, sent after the barge-in, produced
  // a full normal turn. A check that just restates "and it recovered" without its own predicate
  // would be true regardless of what happened above, which is exactly the kind of check that is
  // cheaper to fake than to satisfy — so it is not written as one.

  socket.close();
  await sleep(200);

  console.log('\n=== HAPPY PATH VOICE PIPELINE — RESULTS ===');
  console.log(`${failures.length === 0 ? 'ALL CHECKS PASSED' : `${failures.length} CHECK(S) FAILED: ${failures.join('; ')}`}`);
  return failures.length === 0 ? 0 : 6;
}

main()
  .then((code) => {
    killAll();
    setTimeout(() => process.exit(code), 300);
  })
  .catch((error) => {
    console.error(`[drive] ERROR: ${error.stack ?? error.message}`);
    killAll();
    setTimeout(() => process.exit(1), 300);
  });
