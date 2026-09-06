/**
 * Dev-only structured event log — the gateway-sidecar side of the unified dev-logs/ pipeline.
 *
 * This is the single door every LLM call passes through (C9), so it is the highest-value place to
 * capture what a provider was actually asked and what it actually returned: the transcript in,
 * the model's response, token usage, cost inputs, and latency. `tooling/dev-logs/watch.mjs` tails
 * this file (and its siblings from the other services) and prints one merged stream.
 *
 * Best-effort: a logging failure must never break a completion request.
 */
import { appendFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DEV_LOGGING = (process.env['ORB_DEV_LOGGING'] ?? '1') !== '0';

const __dirname = dirname(fileURLToPath(import.meta.url));
// backend/gateway-sidecar/src/devlog.ts -> product root is 3 parents up.
const LOG_DIR = process.env['ORB_DEV_LOG_DIR'] ?? join(__dirname, '..', '..', '..', 'dev-logs');

export function truncate(value: string, limit = 300): string {
  if (value.length <= limit) return value;
  return `${value.slice(0, limit)}…(+${value.length - limit} chars)`;
}

/**
 * Reduce a service name to a safe filename stem. Mirrors the Python side's `_safe_service_name`
 * (backend/relay-py/.../observability/devlog.py), which this file was missing.
 *
 * Why it matters (observed 2026-08-28): `dev-logs/` had accumulated files named
 * `ORB_CONTEXT_DB_PATH.ndjson`, `ORB_DEV_LOGGING.ndjson`, `5.ndjson`, `010.ndjson`, `K.ndjson` and
 * `CMëÑ.ndjson` — environment-variable names, bare numbers and binary fragments being used as log
 * filenames. Real diagnostics were scattered across dozens of junk files, which materially slowed a
 * production root-cause investigation: three separate log searches came back empty before the
 * actual telemetry was located. The Python writer sanitises and so could never produce the
 * non-alphanumeric names; this writer interpolated `service` straight into the path.
 *
 * It is also a path-traversal hazard, not merely untidy: `join(LOG_DIR, `${service}.ndjson`)` with a
 * `service` containing `../` escapes LOG_DIR and appends to an arbitrary file. Dev-only tooling
 * still runs on a developer's machine with their permissions, so an unsanitised filename derived
 * from a caller-supplied string is a defect regardless of the blast radius.
 *
 * Deliberately identical policy to Python's: keep alphanumerics plus `-`/`_`, trim those from the
 * ends, and fall back to `unknown` so an event is never silently dropped for want of a name.
 */
export function safeServiceName(service: string): string {
  const cleaned = Array.from(service)
    .filter((c) => /[A-Za-z0-9]/.test(c) || c === '-' || c === '_')
    .join('')
    .replace(/^[-_]+/, '')
    .replace(/[-_]+$/, '');
  return cleaned || 'unknown';
}

export function devLog(
  event: string,
  fields: Record<string, unknown> = {},
  level: 'info' | 'warn' | 'error' = 'info',
  service = 'gateway-sidecar',
): void {
  if (!DEV_LOGGING) return;
  try {
    const record = { ts: Date.now() / 1000, service, level, event, ...fields };
    mkdirSync(LOG_DIR, { recursive: true });
    // Sanitised: the raw `service` still goes INSIDE the record (so nothing is lost for the
    // merged-stream reader), but only a safe stem is allowed to become a filename.
    appendFileSync(join(LOG_DIR, `${safeServiceName(service)}.ndjson`), JSON.stringify(record) + '\n');
  } catch {
    // dev-only best-effort; never let logging break the request path
  }
}
