#!/usr/bin/env node
/**
 * A4 acceptance: a voice-provider failure must preserve presence — the session stays open, an
 * audio-bed-fallback signal fires, and (the specific thing the earlier verifier found the FIRST
 * fix missed) the client-side handler for that signal must never cancel presence the way a real
 * session teardown does.
 *
 * Why this is not "did the relay stay up": the pre-A4 defect closed the WebSocket outright on any
 * provider failure (`Action::SendAndClose(Closing{reason: ProviderFailure})`), which is easy to
 * describe as fixed by any test that merely asserts the process didn't crash. The actual contract
 * (docs/BUILD-DIGEST.md §5, "presence > intelligence") is stricter: the session must enter a
 * `Degraded` phase, keep the socket open, and hand the client an explicit
 * `audio_bed_fallback` frame — anything that instead falls through to the client's generic
 * "closing"/"protocol error" handling is WORSE than doing nothing, because those handlers call
 * `presenceEventsRef.current.cancelBy('error')` (apps/mobile/src/App.tsx), tearing down the one
 * thing the whole fix exists to protect. That exact regression happened once already in this
 * session's own history (FLEET-LEARNINGS.md, "client-wiring gap for A4 audio_bed_fallback") when
 * a backend fix shipped without its client-side frame-dispatch counterpart, and was found only by
 * grepping the failure handler for `cancelBy`.
 *
 * This script drives a REAL COMPILED `orb-relay-rs` BINARY, configured with an explicit provider
 * (`ORB_RELAY_PROVIDER=http`, satisfying A9's "no implicit fake default" fix) pointed at
 * `provider_stub.mjs` — a real HTTP server speaking the real `/v1/tts/synthesize` contract that
 * can be told to fail on command (there is no way to make the real paid Fish/Cartesia/Sarvam
 * providers fail to order, and `FakeProvider::failing()` in provider.rs is `#[cfg(test)]`-only —
 * unreachable from a compiled binary a real WebSocket client connects to). It asserts two layers:
 *
 *   1. RELAY-LEVEL (dynamic, driven over the real socket): the session does not close, an
 *      `audio_bed_fallback` frame is observed (not `closing`/a raw protocol error), and the
 *      session remains in the documented Degraded contract (voice work refused, Pause still
 *      honoured) rather than merely "not yet closed by accident".
 *   2. CLIENT-LEVEL (static, the same check a verifier would run by hand): the actual
 *      `onAudioBedFallback` handler body in `apps/mobile/src/App.tsx` is extracted and asserted to
 *      NOT call `cancelBy(` anywhere in it — the precise regression class the client-wiring gap
 *      entry describes. This cannot be exercised dynamically without a live RN runtime/simulator
 *      (out of scope here, no device — see the honest-gaps note at the bottom of the report), so
 *      it is checked the way the earlier verifier checked it: by reading the real handler's own
 *      source, not by trusting a summary.
 *
 * Run:
 *   node e2e-human-simulator/provider_failure_presence_drive.mjs
 * Exit 0 only if every check passes; exit 6 lists which failed.
 */

import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import WebSocket from 'ws';
import { startProviderStub } from './provider_stub.mjs';

const ROOT = new URL('..', import.meta.url);
const RELAY_PORT = 18192; // distinct fixed port from barge_in_cancels_call_drive.mjs
const RELAY_URL = `ws://127.0.0.1:${RELAY_PORT}`;
const TENANT = 't0';
const SESSION = `provider-failure-presence-${Date.now()}`;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findRelayBinary() {
  // fileURLToPath (not `.pathname`) — this repo's own path contains a space
  // ("Principal Engineering"), and `.pathname` leaves that percent-encoded ("%20"), so
  // existsSync/spawn would silently look at a path that can never exist.
  const release = fileURLToPath(new URL('backend/relay-rs/target/release/orb-relay-rs', ROOT));
  return existsSync(release) ? release : null;
}

function startRelay(providerUrl) {
  const binary = findRelayBinary();
  const env = {
    ...process.env,
    ORB_RELAY_ADDR: `127.0.0.1:${RELAY_PORT}`,
    ORB_RELAY_PROVIDER: 'http',
    ORB_RELAY_PROVIDER_URL: providerUrl,
    ORB_DEV_LOGGING: '0',
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
  let closeEvent = null;
  socket.on('message', (data, isBinary) => {
    const message = { data, isBinary: isBinary || Buffer.isBuffer(data) || data instanceof ArrayBuffer };
    const waiter = waiters.shift();
    if (waiter) waiter.resolve(message);
    else queue.push(message);
  });
  socket.on('close', (code, reason) => { closeEvent = { code, reason: reason?.toString?.() ?? '' }; });
  return {
    next(timeoutMs = 8000) {
      if (queue.length > 0) return Promise.resolve(queue.shift());
      return new Promise((resolve, reject) => {
        // A timed-out waiter MUST remove itself from `waiters` — otherwise it sits there
        // forever, and the very next real message that arrives gets shifted to this dead,
        // already-rejected waiter (a no-op, since a settled promise can't change state) and is
        // silently dropped instead of reaching whichever call is actually still waiting for it.
        // Found by exactly this symptom: an expected-to-timeout `waitForFrameType('speech_starting',
        // 1200)` check swallowed the real `closing` frame that arrived afterward.
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
    closeEvent: () => closeEvent,
  };
}

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

/** Reads apps/mobile/src/App.tsx and extracts the `onAudioBedFallback: () => { ... },` handler
 * body verbatim, by brace-counting from its opening `{` — not a regex slice, so it can't silently
 * grab the wrong span if the file is reformatted. Fails loudly (not "assume pass") if the handler
 * can't be found at all, e.g. because it was renamed or removed. */
function extractOnAudioBedFallbackHandler() {
  const file = fileURLToPath(new URL('apps/mobile/src/App.tsx', ROOT));
  const source = readFileSync(file, 'utf8');
  const anchor = 'onAudioBedFallback:';
  const anchorIndex = source.indexOf(anchor);
  if (anchorIndex === -1) {
    throw new Error(`could not find "${anchor}" in apps/mobile/src/App.tsx — has the handler been renamed?`);
  }
  const braceStart = source.indexOf('{', anchorIndex);
  if (braceStart === -1) throw new Error('found the handler key but no opening brace after it');
  let depth = 0;
  let i = braceStart;
  for (; i < source.length; i++) {
    if (source[i] === '{') depth += 1;
    else if (source[i] === '}') {
      depth -= 1;
      if (depth === 0) break;
    }
  }
  if (depth !== 0) throw new Error('unbalanced braces while scanning the handler body — refusing to guess');
  return source.slice(braceStart, i + 1);
}

async function main() {
  const stub = await startProviderStub({ chunkCount: 20, chunkIntervalMs: 40, chunkBytes: 320 });
  console.log(`[stub] provider double listening on ${stub.baseUrl}`);
  const relay = startRelay(stub.baseUrl);

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

    // Command the stub provider double to fail the NEXT synthesize call — the "fake provider mode
    // that can be told to fail on command" the brief asks for, since the real paid providers and
    // the in-process `FakeProvider` cannot be commanded to fail from outside a compiled binary.
    stub.state.failTts = true;
    console.log('[test] provider stub armed to fail the next TTS call');

    socket.send(JSON.stringify({
      type: 'speak', tenant_id: TENANT, session_id: SESSION,
      text: 'This call will fail at the provider.',
      voice_id: 'orb.warm.v1', emotion: 'calm',
    }));

    // --- Property 1: the session does NOT close. -----------------------------------------------
    const fallback = await waitForFrameType(inbox, 'audio_bed_fallback', 8000).catch((error) => {
      throw new Error(`never observed audio_bed_fallback: ${error.message}`);
    });
    note('audio_bed_fallback frame observed (not silence, not a generic error)',
      fallback.tenant_id === TENANT && fallback.session_id === SESSION);

    await sleep(300);
    note('the WebSocket is still open after the provider failure (readyState OPEN)', socket.readyState === WebSocket.OPEN,
      `readyState=${socket.readyState}`);
    note('no close frame was received', inbox.closeEvent() === null,
      inbox.closeEvent() ? JSON.stringify(inbox.closeEvent()) : 'not closed');

    // --- Degraded-mode contract: voice work is refused, but the session is still alive. ---------
    // Send another Speak while degraded — per session.rs's Phase::Degraded arm this must be
    // silently ignored (frame.ignored devlog event, no new speech_starting), proving Degraded is a
    // real state transition and not merely "the socket happens to still be connected".
    socket.send(JSON.stringify({
      type: 'speak', tenant_id: TENANT, session_id: SESSION,
      text: 'This should be ignored: the session is degraded.',
      voice_id: 'orb.warm.v1', emotion: 'calm',
    }));
    let sawUnexpectedSpeechStarting = false;
    try {
      await waitForFrameType(inbox, 'speech_starting', 1200);
      sawUnexpectedSpeechStarting = true;
    } catch { /* expected: no speech_starting should arrive while degraded */ }
    note('voice work is refused while degraded (no speech_starting for a Speak sent after failure)',
      !sawUnexpectedSpeechStarting);

    // Pause remains the one way out of Degraded, per the documented contract.
    socket.send(JSON.stringify({ type: 'pause', tenant_id: TENANT, session_id: SESSION }));
    const closing = await waitForFrameType(inbox, 'closing', 4000);
    note('an explicit Pause still closes the session cleanly from Degraded', closing.reason === 'user_pause',
      `reason=${closing.reason}`);

    // --- Property 2 (client-level, static): the real onAudioBedFallback handler in App.tsx must
    // never cancel presence. This is the exact check the earlier verifier used to catch the
    // client-wiring gap — reading the handler's own source, not the relay's behaviour. -----------
    const handlerBody = extractOnAudioBedFallbackHandler();
    // Strip `//` line comments before scanning for calls — this handler's own doc comment
    // deliberately SAYS "no stopCapture()" / "no transport close" in prose (documenting the
    // absence, per its own §5 note), and a naive regex over the raw body matches that comment
    // text as if it were a real call. Found by running this check for real: the first version of
    // this script reported a false FAIL by matching "stopCapture(" inside the comment on the very
    // line explaining why the handler must not call it.
    const codeOnly = handlerBody.replace(/\/\/.*$/gm, '');
    const callsCancelBy = /\.cancelBy\s*\(/.test(codeOnly);
    note('apps/mobile/src/App.tsx onAudioBedFallback handler does NOT call cancelBy(...) (presence preserved)',
      !callsCancelBy,
      callsCancelBy ? 'found a cancelBy(...) call inside the handler body' : `handler body is ${handlerBody.length} chars, no cancelBy call`);
    const closesTransport = /stopCapture\s*\(|\.close\s*\(/.test(codeOnly);
    note('apps/mobile/src/App.tsx onAudioBedFallback handler does NOT close the transport or stop capture',
      !closesTransport,
      closesTransport ? 'found a stopCapture()/close() call inside the handler body' : 'no transport-teardown call found');
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
  (error) => { console.error(`[provider-failure-presence] ERROR ${error.stack ?? error.message}`); process.exit(1); },
);
