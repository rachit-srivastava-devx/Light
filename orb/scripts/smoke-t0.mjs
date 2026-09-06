import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const CODEX_NODE = '/Users/rachitsrivastava/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node';
const NODE = existsSync(CODEX_NODE) ? CODEX_NODE : process.argv[0];

function start(name, command, args) {
  const child = spawn(command, args, {
    cwd: ROOT,
    env: {
      ...process.env,
      NODE_ENV: 'production',
      ORB_LLM_GATEWAY_ADAPTER: 'memory',
      PATH: `/Users/rachitsrivastava/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:/opt/homebrew/bin:${process.env.PATH ?? ''}`,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.on('error', (error) => {
    throw error;
  });
  child.stdout.on('data', (chunk) => process.stdout.write(`[${name}] ${chunk}`));
  child.stderr.on('data', (chunk) => process.stderr.write(`[${name}] ${chunk}`));
  return child;
}

async function waitFor(url, label) {
  const deadline = Date.now() + 10_000;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
      lastError = new Error(`${label} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw lastError ?? new Error(`${label} did not start`);
}

async function request(method, url, body) {
  const response = await fetch(url, {
    method,
    headers: body ? { 'content-type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await response.text();
  const payload = text ? JSON.parse(text) : null;
  if (!response.ok) {
    throw new Error(`${method} ${url} failed ${response.status}: ${text}`);
  }
  return payload;
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function main() {
  const sidecar = start('sidecar', NODE, [
    'node_modules/vite-node/vite-node.mjs',
    'backend/gateway-sidecar/src/index.ts',
  ]);
  const relay = start('relay', 'backend/relay-py/.venv/bin/python', [
    '-m',
    'uvicorn',
    'orb_relay.app:app',
    '--host',
    '127.0.0.1',
    '--port',
    '8765',
  ]);

  try {
    await waitFor('http://127.0.0.1:8082/healthz', 'sidecar');
    await waitFor('http://127.0.0.1:8765/healthz', 'relay');

    const atomize = await request('POST', 'http://127.0.0.1:8765/v1/atomize', {
      tenant_id: 't0',
      user_id: 'smoke',
      session_id: 'smoke-t0',
      task: 'Open the first small action',
    });
    assert(atomize.output.steps_total === 1, 'atomize did not return a deterministic fallback step');
    assert(atomize.source === 'fallback', 'T0 memory sidecar should exercise the fallback path');

    const warmup = await request('POST', 'http://127.0.0.1:8765/v1/session/warmup', {
      tenant_id: 't0',
      user_id: 'smoke',
      session_id: 'smoke-t0',
    });
    assert(warmup.profile.tenant_id === 't0', 'warmup lost tenant attribution');
    assert(warmup.phrase_manifest['presence.here.v1'], 'warmup did not return phrase manifest');

    const cache = await request('POST', 'http://127.0.0.1:8765/v1/cache/prime', {
      tenant_id: 't0',
      prompt_version: 'atomizer.v4',
    });
    assert(cache.source === 't0:atomizer.v4', 'cache prime did not return T0 source marker');

    const metrics = await request('POST', 'http://127.0.0.1:8765/v1/eval/metrics');
    assert(metrics.passed === true, 'T0 eval replay gates did not pass');
    assert(metrics.evidence.source === 'local_domain_corpus', 'eval did not run the local domain corpus');
    assert(
      metrics.evidence.claim_boundary.startsWith('T0 replay'),
      'eval did not label the live-provider evidence boundary',
    );
    assert(
      metrics.results.some((r) => r.metric === 'voice_p50_ms' && r.passed === true),
      'eval did not run the T0 voice replay gate',
    );

    console.log('smoke:t0 passed');
  } finally {
    relay.kill('SIGINT');
    sidecar.kill('SIGINT');
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
