import { describe, expect, it } from 'vitest';

import { safeServiceName } from '../src/devlog.js';

// Regression guard for an observed defect (2026-08-28). `dev-logs/` had accumulated files named
// `ORB_CONTEXT_DB_PATH.ndjson`, `ORB_DEV_LOGGING.ndjson`, `5.ndjson`, `010.ndjson`, `K.ndjson` and
// `CMëÑ.ndjson` — env-var names, bare numbers and binary fragments used as log filenames. The
// Python writer sanitised; this TypeScript writer interpolated `service` straight into the path, so
// real telemetry was scattered across dozens of junk files and three log searches came back empty
// during a live root-cause investigation before the actual data was found.
describe('safeServiceName', () => {
  it('keeps ordinary service names untouched', () => {
    expect(safeServiceName('gateway-sidecar')).toBe('gateway-sidecar');
    expect(safeServiceName('relay_py')).toBe('relay_py');
  });

  it('strips the binary garbage that produced CMëÑ.ndjson', () => {
    expect(safeServiceName('CMëÑ')).toBe('CM');
  });

  it('refuses path traversal instead of appending outside the log dir', () => {
    // This is the reason it is a defect rather than untidiness: join(LOG_DIR, `${service}.ndjson`)
    // with a traversing service escapes LOG_DIR entirely.
    expect(safeServiceName('../../etc/passwd')).toBe('etcpasswd');
    expect(safeServiceName('..')).toBe('unknown');
    expect(safeServiceName('/')).toBe('unknown');
  });

  it('never yields an empty stem, so an event is not silently dropped', () => {
    expect(safeServiceName('')).toBe('unknown');
    expect(safeServiceName('---')).toBe('unknown');
    expect(safeServiceName('   ')).toBe('unknown');
  });

  it('trims leading/trailing separators but keeps internal ones', () => {
    expect(safeServiceName('__relay-rs__')).toBe('relay-rs');
    expect(safeServiceName('-a_b-')).toBe('a_b');
  });

  it('matches the Python sibling policy on an env-var-shaped name', () => {
    // Python's _safe_service_name keeps alnum + - _ ; so does this. The fix for the junk files is
    // upstream (stop passing env-var names as a service), but the writer must not create them.
    expect(safeServiceName('ORB_CONTEXT_DB_PATH')).toBe('ORB_CONTEXT_DB_PATH');
  });
});
