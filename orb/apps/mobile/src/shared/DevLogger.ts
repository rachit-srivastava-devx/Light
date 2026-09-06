/**
 * Dev-only structured event log — the mobile side of the unified dev-logs/ pipeline.
 *
 * The RN client cannot write to the host filesystem, so instead of a local file it fires
 * best-effort POSTs at `backend/relay-py`'s `/dev/log` collector (only mounted there when
 * `ORB_DEV_LOGGING` is on — see `orb_relay/app.py`), which appends into the same `dev-logs/`
 * directory every backend service writes to. `tooling/dev-logs/watch.mjs` tails all of them and
 * prints one merged, chronological stream — this is how "what transcript the app is sending,
 * what came back, what broke silently, latency at every level" gets answered from one place.
 *
 * `__DEV__` (RN's global dev-mode flag) gates every call; a production build never imports this
 * module for anything other than a no-op. Fire-and-forget: a failed log POST must never affect
 * the voice loop, so errors are swallowed and requests are not awaited by callers.
 */

declare const __DEV__: boolean | undefined;

export interface DevLoggerOptions {
  readonly baseUrl: string;
  readonly fetchImpl?: typeof fetch;
}

export interface DevLogger {
  readonly log: (event: string, fields?: Record<string, unknown>, level?: 'info' | 'warn' | 'error') => void;
}

export function truncate(value: string, limit = 300): string {
  if (value.length <= limit) return value;
  return `${value.slice(0, limit)}…(+${value.length - limit} chars)`;
}

export const noopDevLogger: DevLogger = { log: () => {} };

export function createDevLogger({ baseUrl, fetchImpl = fetch }: DevLoggerOptions): DevLogger {
  if (typeof __DEV__ !== 'undefined' && !__DEV__) return noopDevLogger;
  const normalizedBaseUrl = baseUrl.replace(/\/+$/, '');

  return {
    log(event, fields = {}, level = 'info') {
      void fetchImpl(`${normalizedBaseUrl}/dev/log`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ service: 'mobile', event, level, fields }),
      }).catch(() => {
        // Best-effort only — a dev collector that is down must never disturb the voice loop.
      });
    },
  };
}
