/**
 * E1 (Track E, anti-mirage) — provider-invocation telemetry for the LLM leg.
 *
 * The scar this file exists to prevent: an agent reporting "tested, latency good, works as
 * imagined" while the app was silently talking to a fake/no-op provider the whole time. A green
 * HTTP 200 from `/v1/complete` is not proof a real model was invoked — the `memory` adapter
 * (`registry/services/llm-gateway/src/llm-gateway/adapters/memory/index.ts`) returns 200 with a
 * deterministic canned reply and zero cost. This module is the single place that turns "which
 * adapter answered this request" into a fact recorded on every response, and a single place that
 * can tell a real answer from a fake one even if the caller's own `adapter` label is wrong.
 *
 * Kept dependency-free and side-effect-free on purpose (no `createServer`, no `.listen()`, no
 * `createGatewayFromEnv()` at module scope) so it can be imported by a script or test — such as
 * `scripts/smoke-real-chain.ts` (E2) — without booting a second gateway-sidecar HTTP server on
 * top of a real one already listening on :8082.
 */
import type { GatewayAdapterName } from './index.js';

/**
 * The memory adapter's own literal constants
 * (`registry/services/llm-gateway/src/llm-gateway/adapters/memory/index.ts:20-21`,
 * `FAKE_MODEL = 'fake-llm-memory'` / `FAKE_RESPONSE_TEXT = 'Memory adapter response.'`). Treated
 * here as read-only evidence markers, not reimplemented — this file only recognizes them.
 */
export const FAKE_ADAPTER_MODEL_MARKER = 'fake-llm-memory';
export const FAKE_ADAPTER_TEXT_MARKER = 'Memory adapter response.';

export interface AdapterInvocationEvidence {
  /** What the caller believes/declares was used. Not trusted on its own — see below. */
  readonly adapter: GatewayAdapterName;
  /** `CompletionResponse.model` as returned by the gateway (real model id, or the fake marker). */
  readonly model: string;
  /** The response text actually produced, before any speech-markup normalization. */
  readonly responseText: string;
}

/**
 * A run is flagged fake if EITHER:
 *  (a) the resolved adapter is literally `memory` (the T0/no-key default), OR
 *  (b) the evidence carries the memory adapter's own model/text marker regardless of what
 *      `adapter` claims — this is deliberate: the point of "detectable" is that a caller cannot
 *      make a fake run look real just by mislabeling it. If `adapter` and the evidence disagree,
 *      that disagreement is itself proof the run cannot be trusted as real.
 *
 * This is the ONLY place this predicate is implemented in TypeScript. `evals/qscore/veto.py`
 * mirrors it in Python (Python cannot import this file) and says so in a comment pointing back
 * here — see `evals/TELEMETRY-CONTRACT.md` for the cross-language contract.
 */
export function isFakeAdapterInvocation(evidence: AdapterInvocationEvidence): boolean {
  if (evidence.adapter === 'memory') return true;
  if (evidence.model === FAKE_ADAPTER_MODEL_MARKER) return true;
  if (evidence.responseText.includes(FAKE_ADAPTER_TEXT_MARKER)) return true;
  return false;
}

/**
 * True exactly when the adapter's own self-report and the observed evidence markers agree. A
 * caller that wants to distinguish "fake and honestly labeled memory" from "labeled real but
 * carrying fake markers" (a strictly worse, lying state) can check this in addition to
 * `isFakeAdapterInvocation`.
 */
export function adapterLabelMatchesEvidence(evidence: AdapterInvocationEvidence): boolean {
  const carriesFakeMarkers =
    evidence.model === FAKE_ADAPTER_MODEL_MARKER || evidence.responseText.includes(FAKE_ADAPTER_TEXT_MARKER);
  return evidence.adapter === 'memory' ? carriesFakeMarkers : !carriesFakeMarkers;
}
