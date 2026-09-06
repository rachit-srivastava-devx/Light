/**
 * Bounded live-provider matrix for the existing voice-provider-sidecar port.
 *
 * Exactly one STT turn (the checked-in 4.35 s fixture) and one 28-character TTS request are sent
 * per provider. API-key values are inherited and never printed. Each child sidecar is terminated
 * after its row so its listener cannot leak or keep the process alive.
 */

import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { resolve } from 'node:path';

type Provider = 'fake' | 'fish' | 'sarvam';

interface MatrixRow {
  provider: Provider;
  passed: boolean;
  health_stt?: string;
  health_tts?: string;
  transcript_chars?: number;
  stt_end_latency_ms?: number;
  tts_total_latency_ms?: number;
  tts_audio_bytes?: number;
  error?: string;
}

const ROOT = process.cwd();
const SIDECAR_ENTRY = resolve(ROOT, 'backend/voice-provider-sidecar/src/index.ts');
const VITE_NODE = resolve(ROOT, 'node_modules/.bin/vite-node');
const STT_FIXTURE = resolve(ROOT, 'e2e-human-simulator/replay/fixtures/teach-ask.wav');
const PROVIDERS: readonly Provider[] = ['fish', 'sarvam', 'fake'];
const TTS_TEXT = 'Open the first small action.';

function pcmFixture(): number[] {
  const converted = spawnSync(
    'ffmpeg',
    ['-hide_banner', '-loglevel', 'error', '-i', STT_FIXTURE, '-f', 's16le', '-ac', '1', '-ar', '16000', 'pipe:1'],
    { maxBuffer: 2_000_000 },
  );
  if (converted.status !== 0 || !converted.stdout?.length) {
    throw new Error(`ffmpeg could not decode the bounded STT fixture (exit ${converted.status ?? 'signal'})`);
  }
  return Array.from(converted.stdout);
}

async function fetchJson(url: string, init?: RequestInit): Promise<Record<string, unknown>> {
  const response = await fetch(url, { ...init, signal: AbortSignal.timeout(45_000) });
  const body = (await response.json()) as Record<string, unknown>;
  if (!response.ok) {
    const errorCode = typeof body['error'] === 'string' ? body['error'] : `HTTP_${response.status}`;
    const message = typeof body['message'] === 'string' ? body['message'] : 'provider request failed';
    throw new Error(`${errorCode}: ${message}`);
  }
  return body;
}

async function waitForHealth(baseUrl: string): Promise<Record<string, unknown>> {
  const deadline = Date.now() + 10_000;
  let lastError = 'sidecar did not start';
  while (Date.now() < deadline) {
    try {
      return await fetchJson(`${baseUrl}/healthz`);
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
      await new Promise((resolveWait) => setTimeout(resolveWait, 50));
    }
  }
  throw new Error(lastError);
}

async function stopChild(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return;
  child.kill('SIGTERM');
  await Promise.race([
    new Promise<void>((resolveExit) => child.once('exit', () => resolveExit())),
    new Promise<void>((resolveTimeout) => setTimeout(resolveTimeout, 1_000)),
  ]);
  if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
}

async function runProvider(provider: Provider, index: number, pcm: number[]): Promise<MatrixRow> {
  const port = 18_083 + index;
  const baseUrl = `http://127.0.0.1:${port}`;
  const child = spawn(VITE_NODE, [SIDECAR_ENTRY], {
    cwd: ROOT,
    env: {
      ...process.env,
      NODE_ENV: 'development',
      VOICE_PROVIDER_SIDECAR_PORT: String(port),
      ORB_STT_PROVIDER: provider,
      ORB_TTS_PROVIDER: provider,
    },
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  let stderr = '';
  child.stderr?.on('data', (chunk: Buffer) => {
    stderr += chunk.toString('utf8').slice(0, 2_000);
  });

  try {
    const health = await waitForHealth(baseUrl);
    const sessionId = `track-j-${provider}`;
    await fetchJson(`${baseUrl}/v1/stt/push`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ session_id: sessionId, audio_bytes: pcm }),
    });
    const sttStarted = performance.now();
    const transcript = await fetchJson(`${baseUrl}/v1/stt/end`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ session_id: sessionId }),
    });
    const sttEndLatencyMs = performance.now() - sttStarted;

    const ttsStarted = performance.now();
    const speech = await fetchJson(`${baseUrl}/v1/tts/synthesize`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text: TTS_TEXT, voice_id: 'orb.warm.v1', emotion: 'warm' }),
    });
    const ttsTotalLatencyMs = performance.now() - ttsStarted;
    const chunks = speech['audio_chunks'];
    if (!Array.isArray(chunks)) throw new Error('sidecar omitted audio_chunks');
    const ttsAudioBytes = chunks.reduce(
      (total: number, chunk: unknown) => total + (Array.isArray(chunk) ? chunk.length : 0),
      0,
    );
    if (ttsAudioBytes <= 0) throw new Error('sidecar returned zero TTS audio bytes');
    const transcriptText = transcript['text'];
    if (typeof transcriptText !== 'string' || transcriptText.trim().length === 0) {
      throw new Error('sidecar returned an empty final transcript');
    }
    return {
      provider,
      passed: health['stt'] === provider && health['tts'] === provider,
      health_stt: String(health['stt']),
      health_tts: String(health['tts']),
      transcript_chars: transcriptText.length,
      stt_end_latency_ms: Number(sttEndLatencyMs.toFixed(1)),
      tts_total_latency_ms: Number(ttsTotalLatencyMs.toFixed(1)),
      tts_audio_bytes: ttsAudioBytes,
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      provider,
      passed: false,
      error: stderr.trim() ? `${message}; sidecar=${stderr.trim().slice(0, 500)}` : message,
    };
  } finally {
    await stopChild(child);
  }
}

async function main(): Promise<void> {
  const pcm = pcmFixture();
  const rows: MatrixRow[] = [];
  for (const [index, provider] of PROVIDERS.entries()) {
    rows.push(await runProvider(provider, index, pcm));
  }
  const report = {
    passed: rows.every((row) => row.passed),
    bounded_spend: {
      stt_audio_seconds_per_real_provider: 4.3535,
      tts_characters_per_real_provider: TTS_TEXT.length,
      requests_per_provider: { stt_push: 1, stt_end: 1, tts: 1 },
      rupees: 'unknown until provider billing export; request counts are not billing proof',
    },
    cartesia: 'skipped: CARTESIA_API_KEY is empty and Cartesia is outside the required 3-row matrix',
    rows,
  };
  console.log(JSON.stringify(report, null, 2));
  if (!report.passed) process.exitCode = 1;
}

void main();
