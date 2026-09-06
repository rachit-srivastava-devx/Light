/**
 * A minimal real HTTP server implementing relay-rs's documented provider contract
 * (backend/relay-rs/src/provider.rs's `HttpContractProvider` doc comment):
 *
 *   POST /v1/stt/push        -> { "text": string|null, "is_final": bool }
 *   POST /v1/stt/end         -> { "text": string, "is_final": true }
 *   POST /v1/tts/synthesize  -> Transfer-Encoding: chunked, one HTTP chunk per audio chunk,
 *                                each chunk's payload `{ "chunk": [byte, ...] }`
 *
 * This plays the role a paid provider sidecar (Fish/Cartesia/Sarvam, or this repo's own
 * voice-provider-sidecar) plays in production, but it is a REAL process, speaking the REAL wire
 * contract over a REAL TCP socket that relay-rs's actual compiled `HttpContractProvider` connects
 * to (`ORB_RELAY_PROVIDER=http`, `ORB_RELAY_PROVIDER_URL=http://127.0.0.1:<this port>`) — not a
 * vitest-level mock, not the in-process `#[cfg(test)] FakeProvider::failing()` (which is
 * unreachable from a compiled binary at all). It differs from the real paid providers only in
 * being controllable "on command" from the driving test, which is exactly what a black-box e2e
 * fault-injection double is for.
 *
 * Two behaviours this test suite needs and the real paid providers cannot give on demand:
 *   - a TTS response that streams slowly enough to leave a real multi-hundred-ms window in which
 *     a `barge_in` frame can land while the HTTP response is still in flight (used by
 *     barge_in_cancels_call_drive.mjs);
 *   - a TTS (or STT) call that fails outright, so relay-rs's real provider-failure path
 *     (`Session::fail`, `Action::SendFallback`) fires without needing a real provider outage
 *     (used by provider_failure_presence_drive.mjs).
 */

import http from 'node:http';

/**
 * @param {object} opts
 * @param {number} [opts.chunkCount] number of TTS audio chunks to stream per synthesize call
 * @param {number} [opts.chunkIntervalMs] delay between each chunk
 * @param {number} [opts.chunkBytes] bytes per chunk
 */
export function startProviderStub(opts = {}) {
  const chunkCount = opts.chunkCount ?? 40;
  const chunkIntervalMs = opts.chunkIntervalMs ?? 60;
  const chunkBytes = opts.chunkBytes ?? 320;

  // Mutable state the driving test flips at runtime — this is the "told to fail on command" knob.
  const state = {
    failTts: false,
    failStt: false,
  };

  // One record per /v1/tts/synthesize call, so the test can inspect exactly how far a specific
  // in-flight call got before the client (relay-rs) dropped the connection on cancellation. This
  // is the direct evidence that the PROVIDER CALL ITSELF was interrupted, not just that playback
  // stopped somewhere downstream: a real HTTP server watching its own socket close early.
  const ttsCalls = [];

  const server = http.createServer((req, res) => {
    let body = '';
    req.on('data', (d) => { body += d; });
    req.on('end', () => {
      if (req.method === 'POST' && req.url === '/v1/stt/push') {
        if (state.failStt) {
          res.writeHead(500, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ error: 'stub_stt_failure' }));
          return;
        }
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ text: null, is_final: false }));
        return;
      }
      if (req.method === 'POST' && req.url === '/v1/stt/end') {
        if (state.failStt) {
          res.writeHead(500, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ error: 'stub_stt_failure' }));
          return;
        }
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ text: 'stub final transcript', is_final: true }));
        return;
      }
      if (req.method === 'POST' && req.url === '/v1/tts/synthesize') {
        if (state.failTts) {
          res.writeHead(500, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ error: 'stub_tts_failure' }));
          return;
        }
        const call = {
          startedAt: Date.now(),
          chunksWrittenBeforeClose: 0,
          plannedChunks: chunkCount,
          closedEarly: false,
          endedAt: null,
        };
        ttsCalls.push(call);
        res.writeHead(200, { 'content-type': 'application/json' }); // no content-length -> chunked
        let i = 0;
        const bytes = Array.from({ length: chunkBytes }, (_, b) => b % 256);
        const timer = setInterval(() => {
          if (i >= chunkCount) {
            clearInterval(timer);
            call.endedAt = Date.now();
            res.end();
            return;
          }
          i += 1;
          const ok = res.write(`${JSON.stringify({ chunk: bytes })}\n`, (err) => {
            if (err) { /* the peer already went away; recorded via the socket 'close' handler below */ }
          });
          call.chunksWrittenBeforeClose = i;
          if (!ok) { /* backpressure is fine at this chunk size/interval; nothing to do */ }
        }, chunkIntervalMs);
        // The real signal: relay-rs's `HttpContractProvider::synthesize_streaming` drops its
        // `TcpStream` the moment `CancelFlag` fires, mid short-read-timeout poll. That closes the
        // TCP connection from the client side while THIS server is still mid-stream — observed
        // here as the request socket closing before `i` reached `chunkCount`.
        req.socket.once('close', () => {
          clearInterval(timer);
          if (call.endedAt === null) {
            call.closedEarly = true;
            call.endedAt = Date.now();
          }
        });
        return;
      }
      res.writeHead(404, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ error: 'not_found', path: req.url }));
    });
  });

  return new Promise((resolve, reject) => {
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      resolve({
        port,
        baseUrl: `http://127.0.0.1:${port}`,
        state,
        ttsCalls,
        stop: () => new Promise((r) => server.close(() => r())),
      });
    });
  });
}
