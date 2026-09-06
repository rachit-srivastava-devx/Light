#!/usr/bin/env node
/**
 * A8 acceptance: barge-in must cancel the IN-FLIGHT PROVIDER CALL, not just stop local playback,
 * and a fresh turn must begin cleanly afterwards.
 *
 * This is a different claim from what `barge_in_drive.mjs` already measures. That script proves
 * relay-side FRAME-STOP LATENCY against a real Fish TTS backend — how fast the relay stops
 * FORWARDING audio to the client — and says so explicitly in its own header: "relay-side
 * frame-stop latency, a transport-level measurement... NOT audio-yield-at-the-speaker." It does
 * not, and cannot, prove that the HTTP request to the voice provider itself was severed, because
 * a real paid TTS provider cannot be told "keep streaming after the client hangs up and tell me
 * how far you got" — there is nowhere to observe that from the outside.
 *
 * This script closes exactly that gap using `provider_stub.mjs`: a real HTTP server, speaking
 * relay-rs's real documented `/v1/tts/synthesize` provider contract over a real TCP socket, that
 * a REAL COMPILED `orb-relay-rs` BINARY connects to as its configured provider
 * (`ORB_RELAY_PROVIDER=http`). The stub deliberately streams slowly (40 chunks, 60ms apart, ~2.4s
 * total) so there is a wide, real window in which a `barge_in` frame can land mid-synthesis. When
 * the relay's `CancelFlag` fires, `HttpContractProvider::synthesize_streaming`
 * (backend/relay-rs/src/provider.rs) drops its `TcpStream`, which closes the real OS-level TCP
 * connection to the stub BEFORE the stub has written all its scheduled chunks. The stub records
 * exactly how many chunks it managed to write before its socket closed. If that count is less
 * than the full plan, the provider call was truly interrupted mid-flight — not merely muted
 * downstream of a call that ran to completion.
 *
 * Two properties asserted, matching the brief:
 *   1. IN-FLIGHT CALL CANCELLED (not just local playback stopped): the stub's own record of the
 *      synthesize call in flight at the moment of barge-in shows `closedEarly` and a chunk count
 *      short of the full plan.
 *   2. A FRESH TURN BEGINS CLEANLY: a second `speak` on the SAME session/socket right after the
 *      barge-in completes as a normal, full-length, uncancelled turn — proves the session was not
 *      left wedged, half-cancelled, or unable to speak again.
 *
 * What this does NOT prove (stated up front, not discovered by a verifier later): the "user
 * started speaking" trigger is a directly-sent `barge_in` control frame, not a real 200ms-sustained
 * on-device VAD detection feeding `apps/mobile/src/voice/BargeIn.ts`'s
 * `createBargeInCoordinator` — there is no phone or simulator microphone/AEC path in this
 * harness (see e2e-human-simulator/README.md: audio-at-the-speaker is a separately owned, still
 * open gap). `apps/mobile/src/voice/BargeInPlaybackLatency.test.ts` already covers the client-side
 * 200ms-sustained-speech -> `cancelProviderCall` invocation in isolation; this script covers the
 * transport contract that call rides on, end to end, against a real relay-rs binary.
 *
 * Run:
 *   node e2e-human-simulator/barge_in_cancels_call_drive.mjs
 * Exit 0 only if both properties hold; exit 6 lists which failed.
 */

import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import WebSocket from 'ws';
import { startProviderStub } from './provider_stub.mjs';

const ROOT = new URL('..', import.meta.url);
const RELAY_PORT = 18191; // fixed, non-default port so this never collides with a dev-stack relay
const RELAY_URL = `ws://127.0.0.1:${RELAY_PORT}`;
const TENANT = 't0';
const SESSION = `barge-in-cancel-${Date.now()}`;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findRelayBinary() {
  // fileURLToPath (not `.pathname`) — this repo's own path contains a space
  // ("Principal Engineering"), and `.pathname` leaves that percent-encoded ("%20"), so
  // existsSync/spawn would silently look at a path that can never exist.
  const release = fileURLToPath(
    new URL('backend/relay-rs/target/release/orb-relay-rs', ROOT),
  );
  return existsSync(release) ? release : null;
}

function startRelay(devLogDir, providerUrl) {
  const binary = findRelayBinary();
  const env = {
    ...process.env,
    ORB_RELAY_ADDR: `127.0.0.1:${RELAY_PORT}`,
    ORB_RELAY_PROVIDER: 'http',
    ORB_RELAY_PROVIDER_URL: providerUrl,
    ORB_DEV_LOGGING: '1',
    ORB_DEV_LOG_DIR: devLogDir,
  };
  const child = binary
    ? spawn(binary, [], { env, stdio: ['ignore', 'pipe', 'pipe'] })
    : spawn('cargo', ['run', '--quiet', '--release'], {
        cwd: new URL('backend/relay-rs/', ROOT),
        env: { ...env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH ?? ''}` },
        stdio: ['ignore', 'pipe', 'pipe'],
      });
  console.log(`[relay-rs] starting via ${binary ?? 'cargo run --release (no prebuilt binary found)'}`);
  child.stdout.on('data', (chunk) => process.stdout.write(`[relay-rs] ${chunk}`));
  child.stderr.on('data', (chunk) => process.stderr.write(`[relay-rs] ${chunk}`));
  return child;
}

async function connectWithRetry(url, attempts = 60) {
  let lastError;
  for (let i = 0; i < attempts; i++) {
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
  throw lastError ?? new Error(`relay-rs never accepted a connection at ${url}`);
}

function createInbox(socket) {
  const queue = [];
  const waiters = [];
  let audioBytesTotal = 0;
  let audioFramesTotal = 0;
  socket.on('message', (data, isBinary) => {
    const binary = isBinary || Buffer.isBuffer(data) || data instanceof ArrayBuffer;
    if (binary) {
      audioBytesTotal += data.byteLength ?? data.length ?? 0;
      audioFramesTotal += 1;
    }
    const message = { data, isBinary: binary };
    const waiter = waiters.shift();
    if (waiter) waiter.resolve(message);
    else queue.push(message);
  });
  return {
    next(timeoutMs = 8000) {
      if (queue.length > 0) return Promise.resolve(queue.shift());
      return new Promise((resolve, reject) => {
        // A timed-out waiter MUST remove itself from `waiters` — otherwise it sits there
        // forever, and the next real message that arrives gets shifted to this dead,
        // already-rejected waiter (a no-op, since a settled promise can't change state) and is
        // silently dropped instead of reaching whichever call is actually still waiting for it.
        // See provider_failure_presence_drive.mjs's identical createInbox for where this bug
        // actually manifested (a caught, expected timeout swallowed the real next frame).
        const entry = {
          resolve: (m) => { clearTimeout(timer); resolve(m); },
        };
        const timer = setTimeout(() => {
          const idx = waiters.indexOf(entry);
          if (idx !== -1) waiters.splice(idx, 1);
          reject(new Error(`timed out waiting for a frame after ${timeoutMs}ms`));
        }, timeoutMs);
        waiters.push(entry);
      });
    },
    audioBytes: () => audioBytesTotal,
    audioFrames: () => audioFramesTotal,
  };
}

function parseJson(message) {
  if (message.isBinary) throw new Error('expected a JSON control frame, got binary audio');
  return JSON.parse(message.data.toString('utf8'));
}

/** Reads frames off the inbox until one matching `type` arrives, or times out. Anything of a
 * different type (audio, other control frames) is consumed and discarded — callers that need
 * those instead use inbox.audioBytes()/audioFrames() rather than reading them from here. */
async function waitForFrameType(inbox, type, timeoutMs = 8000) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const remaining = deadline - Date.now();
    if (remaining <= 0) throw new Error(`timed out waiting for a "${type}" frame`);
    const message = await inbox.next(remaining);
    if (message.isBinary) continue;
    let frame;
    try { frame = JSON.parse(message.data.toString('utf8')); } catch { continue; }
    if (frame.type === type) return frame;
  }
}

function readDevLog(devLogDir) {
  const file = path.join(devLogDir, 'relay-rs.ndjson');
  if (!existsSync(file)) return [];
  return readFileSync(file, 'utf8')
    .split('\n')
    .filter(Boolean)
    .map((line) => { try { return JSON.parse(line); } catch { return null; } })
    .filter(Boolean);
}

async function main() {
  const devLogDir = mkdtempSync(path.join(tmpdir(), 'orb-relay-devlog-'));
  const stub = await startProviderStub({ chunkCount: 40, chunkIntervalMs: 60, chunkBytes: 320 });
  console.log(`[stub] provider double listening on ${stub.baseUrl}`);
  const relay = startRelay(devLogDir, stub.baseUrl);

  const failures = [];
  const note = (label, passed, detail) => {
    console.log(`${passed ? 'PASS' : 'FAIL'}  ${label}${detail ? ` — ${detail}` : ''}`);
    if (!passed) failures.push(label);
  };

  let socket;
  try {
    socket = await connectWithRetry(RELAY_URL);
    const inbox = createInbox(socket);
    console.log(`[test] connected to ${RELAY_URL}, session=${SESSION}`);

    socket.send(JSON.stringify({ type: 'start_listening', tenant_id: TENANT, session_id: SESSION }));
    await waitForFrameType(inbox, 'listening_confirmed');

    // --- Turn 1: speak, then barge in while the stub is still mid-stream. ---------------------
    socket.send(JSON.stringify({
      type: 'speak', tenant_id: TENANT, session_id: SESSION,
      text: 'This is a long utterance the orb should never finish saying once interrupted.',
      voice_id: 'orb.warm.v1', emotion: 'calm',
    }));
    await waitForFrameType(inbox, 'speech_starting');

    // 40 chunks * 60ms = ~2.4s total stream. Barge in well inside that window (at ~250ms) so the
    // provider call is provably still in flight, not already finished, when the cancel lands.
    await sleep(250);
    const framesBeforeBargeIn = inbox.audioFrames();
    console.log(`[test] audio frames received before barge-in: ${framesBeforeBargeIn}`);

    socket.send(JSON.stringify({ type: 'barge_in', tenant_id: TENANT, session_id: SESSION }));
    const complete1 = await waitForFrameType(inbox, 'speech_complete');
    note('barge-in yields the floor (speech_complete observed)', complete1.session_id === SESSION);

    // Give the cancelled provider call time to actually unwind (CANCEL_POLL_INTERVAL is 10ms; the
    // stub's socket 'close' handler fires as soon as the OS delivers the FIN).
    await sleep(400);

    // --- Property 1: the in-flight PROVIDER CALL was cancelled, not just local playback. -------
    note('exactly one synthesize call was made for turn 1', stub.ttsCalls.length === 1,
      `stub recorded ${stub.ttsCalls.length} call(s)`);
    const call1 = stub.ttsCalls[0];
    if (call1) {
      note('the provider HTTP connection was closed by the relay before the stub finished streaming',
        call1.closedEarly === true,
        `closedEarly=${call1.closedEarly}, wrote ${call1.chunksWrittenBeforeClose}/${call1.plannedChunks} chunks`);
      note('the call was interrupted mid-stream, not right at the start or right at the end',
        call1.chunksWrittenBeforeClose > 0 && call1.chunksWrittenBeforeClose < call1.plannedChunks,
        `${call1.chunksWrittenBeforeClose}/${call1.plannedChunks} chunks written`);
    }

    // Corroborating server-truth signal: relay-rs's own devlog proof that a barge-in landed and
    // was measured against the latency budget (backend/relay-rs/src/main.rs's
    // `latency.barge_in_yield` event, described in its own code comment as "a barge-in leaves
    // proof in dev-logs/ that audio was actually suppressed, rather than only that a cancel was
    // requested").
    const events = readDevLog(devLogDir);
    const yieldEvent = events.find((e) => e.event === 'latency.barge_in_yield');
    note('relay-rs devlog recorded the barge-in yield (server-side proof, not client-reported)',
      Boolean(yieldEvent),
      yieldEvent ? `yield_latency_ms=${yieldEvent.yield_latency_ms?.toFixed?.(1)}` : 'no latency.barge_in_yield event found');

    // --- Property 2: a fresh turn begins cleanly on the same session afterwards. ---------------
    socket.send(JSON.stringify({
      type: 'speak', tenant_id: TENANT, session_id: SESSION,
      text: 'This is a second, uninterrupted turn.',
      voice_id: 'orb.warm.v1', emotion: 'calm',
    }));
    await waitForFrameType(inbox, 'speech_starting');
    const complete2 = await waitForFrameType(inbox, 'speech_complete', 10000);
    note('the fresh turn reaches speech_complete cleanly', complete2.session_id === SESSION);

    await sleep(200);
    const call2 = stub.ttsCalls[1];
    note('a second synthesize call was made for the fresh turn', Boolean(call2), `stub recorded ${stub.ttsCalls.length} call(s) total`);
    if (call2) {
      note('the fresh turn ran to completion, uncancelled (proves the session was not left wedged)',
        call2.closedEarly === false && call2.chunksWrittenBeforeClose === call2.plannedChunks,
        `closedEarly=${call2.closedEarly}, wrote ${call2.chunksWrittenBeforeClose}/${call2.plannedChunks} chunks`);
    }
  } finally {
    try { socket?.close(); } catch { /* already closed */ }
    relay.kill('SIGKILL');
    await stub.stop();
  }

  console.log(`\n${failures.length === 0 ? 'ALL CHECKS PASSED' : `FAILED: ${failures.join('; ')}`}`);
  return failures.length === 0 ? 0 : 6;
}

main().then(
  (code) => process.exit(code),
  (error) => { console.error(`[barge-in-cancels-call] ERROR ${error.stack ?? error.message}`); process.exit(1); },
);
