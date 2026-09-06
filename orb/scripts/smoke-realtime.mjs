import { spawn } from 'node:child_process';
import WebSocket from 'ws';

const ROOT = new URL('..', import.meta.url);
const RELAY_URL = 'ws://127.0.0.1:8091';

function startRelay() {
  const child = spawn('cargo', ['run', '--quiet'], {
    cwd: new URL('backend/relay-rs/', ROOT),
    env: {
      ...process.env,
      ORB_RELAY_ADDR: '127.0.0.1:8091',
      ORB_RELAY_PROVIDER: 'fake',
      PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH ?? ''}`,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', (chunk) => process.stdout.write(`[relay-rs] ${chunk}`));
  child.stderr.on('data', (chunk) => process.stderr.write(`[relay-rs] ${chunk}`));
  return child;
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function connectWithRetry(url) {
  let lastError;
  for (let attempt = 0; attempt < 40; attempt++) {
    try {
      const socket = new WebSocket(url);
      await new Promise((resolve, reject) => {
        socket.once('open', resolve);
        socket.once('error', reject);
      });
      return socket;
    } catch (error) {
      lastError = error;
      await wait(250);
    }
  }
  throw lastError ?? new Error('relay-rs did not accept websocket connections');
}

function createInbox(socket) {
  const queue = [];
  const waiters = [];
  let failure = null;
  socket.on('message', (data, isBinary) => {
    const message = { data, isBinary: isBinary || Buffer.isBuffer(data) || data instanceof ArrayBuffer };
    const waiter = waiters.shift();
    if (waiter) waiter.resolve(message);
    else queue.push(message);
  });
  socket.on('error', (error) => {
    failure = error;
    for (const waiter of waiters.splice(0)) waiter.reject(error);
  });
  return {
    next() {
      if (queue.length > 0) return Promise.resolve(queue.shift());
      if (failure) return Promise.reject(failure);
      return new Promise((resolve, reject) => waiters.push({ resolve, reject }));
    },
  };
}

function sendJson(socket, frame) {
  socket.send(JSON.stringify(frame));
}

function parseJsonMessage(message) {
  if (message.isBinary) throw new Error('expected JSON frame, got binary');
  return JSON.parse(message.data.toString('utf8'));
}

function binaryLength(message) {
  return message.data.byteLength ?? message.data.length ?? 0;
}

function describeMessage(message) {
  return `${message.isBinary ? 'binary' : 'text'}:${binaryLength(message)}:${message.data.toString?.('utf8') ?? ''}`;
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function main() {
  const relay = startRelay();
  try {
    const socket = await connectWithRetry(RELAY_URL);
    const inbox = createInbox(socket);
    sendJson(socket, { type: 'start_listening', tenant_id: 't1', session_id: 's1' });

    socket.send(Buffer.alloc(320, 0));
    socket.send(Buffer.alloc(320, 1));
    socket.send(Buffer.alloc(320, 2));
    const transcript = parseJsonMessage(await inbox.next());
    assert(transcript.type === 'transcript', 'expected transcript frame');
    assert(transcript.tenant_id === 't1', 'transcript lost tenant_id');
    assert(transcript.session_id === 's1', 'transcript lost session_id');
    assert(transcript.text === 'partial after 3 frames', 'unexpected fake STT transcript');

    sendJson(socket, {
      type: 'speak',
      tenant_id: 't1',
      session_id: 's1',
      text: 'one down',
      voice_id: 'orb.warm.v1',
      emotion: 'calm',
    });
    const starting = parseJsonMessage(await inbox.next());
    assert(starting.type === 'speech_starting', 'expected speech_starting frame');
    assert(starting.tenant_id === 't1', 'speech_starting lost tenant_id');

    const firstChunk = await inbox.next();
    const secondChunk = await inbox.next();
    assert(
      firstChunk.isBinary && binaryLength(firstChunk) === 320,
      `first TTS chunk missing; got ${describeMessage(firstChunk)}`,
    );
    assert(
      secondChunk.isBinary && binaryLength(secondChunk) === 320,
      `second TTS chunk missing; got ${describeMessage(secondChunk)}`,
    );

    const complete = parseJsonMessage(await inbox.next());
    assert(complete.type === 'speech_complete', 'expected speech_complete frame');
    assert(complete.session_id === 's1', 'speech_complete lost session_id');

    socket.close();
    console.log('smoke:realtime passed');
  } finally {
    relay.kill('SIGINT');
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
