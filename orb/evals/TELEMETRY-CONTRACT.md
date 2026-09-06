# Telemetry field contract — Track E requirements on Tracks B/C/D

Written by Track E (anti-mirage). This is a **normative spec**, not code: Track E does not own
`apps/**`, `backend/relay-py/**`, `backend/relay-rs/**`, or `backend/voice-provider-sidecar/**`
and will not edit them. The Opus reviewer enforces this doc against those tracks' diffs.

Two things are required for `e2e-human-simulator`'s journeys and `evals/quality_score`'s Q score
(E4) to be able to read the facts they need: an explicit **mode** field, and explicit **provider
telemetry**. Both must be *typed and observable in the response envelope* — never something the
caller has to infer from which endpoint it called or from trace-log archaeology.

## 1. `mode` — the field Track B/C are wiring (acceptance contract B4)

```ts
type ResponseMode = 'converse' | 'focus' | 'teach';
```

- **Location**: the response envelope for `/v1/respond` (`backend/relay-py/src/orb_relay/app.py`,
  `ConversationResponse`, currently `app.py:157-164`) and wherever a teach-specific route/branch
  lands. Today `ConversationResponse` has no `mode` field at all — every non-task turn is
  indistinguishable from every other in the response shape.
- **Type**: a real closed union (Pydantic `Literal['converse', 'focus', 'teach']` on the Python
  side), not a bare `str`. B4 in the acceptance contract is explicit: "Illegal/unknown mode is
  unrepresentable (typed union, not a string)." A `str` field that happens to contain one of three
  values does not satisfy B4 even if every current caller behaves — validate it as a Literal so a
  new, unenumerated mode fails loudly instead of round-tripping silently.
- **Why Track E needs it**: `e2e-human-simulator/journeys.json`'s three new journeys
  (`teach-me-something`, `open-domain-vent`, `topic-switch` — see §3 below) assert `expect.mode`.
  Without this field on the wire, the harness has no way to check routing landed in the right mode
  short of string-sniffing the reply text, which is exactly the kind of proxy this track exists to
  reject.

## 2. Provider telemetry — the field Track E needs for E1/E4

```ts
interface ProviderTelemetry {
  readonly llm_adapter: 'memory' | 'anthropic' | 'gemini';
  readonly stt_provider: 'fake' | 'fish' | 'sarvam' | 'deepgram';
  readonly tts_provider: 'fake' | 'fish' | 'sarvam' | 'cartesia';
}
```

- **These are not new types.** They are exactly
  `GatewayAdapterName` (`backend/gateway-sidecar/src/index.ts:25`) and
  `SttProviderName` / `TtsProviderName` (`backend/voice-provider-sidecar/src/config.ts:12-13`).
  Re-declare them in Python/wherever needed as the literal same three/four-way union — do not
  invent a fourth option or a different casing.
- **`llm_adapter` is already available for free.** As of Track E's E1 change,
  `backend/gateway-sidecar/src/index.ts`'s `POST /v1/complete` response now carries top-level
  `adapter: GatewayAdapterName` and `is_fake_adapter: boolean` fields (see
  `backend/gateway-sidecar/src/adapterEvidence.ts` for the detector, and
  `backend/gateway-sidecar/__tests__/adapterTelemetry.test.ts` for the wire-level proof). Whatever
  in `backend/relay-py/src/orb_relay/proxy/gateway_client.py` reads `/v1/complete`'s JSON body just
  needs to keep (not discard) the `adapter` key already sitting next to `content`/`usage`, and
  thread it onto `ConversationResponse` as `provider_telemetry.llm_adapter`. No new gateway-sidecar
  work is needed for this half.
- **`stt_provider` / `tts_provider`** are resolvable today from
  `voice-provider-sidecar`'s own `GET /healthz`, which already returns
  `{ status: 'ok', stt: stt.label, tts: tts.label }` (`backend/voice-provider-sidecar/src/index.ts:114-117`).
  Whichever hop owns the per-turn response envelope (relay-rs, most likely, since it is the process
  that actually calls voice-provider-sidecar on the audio hot path) should attach these labels
  per-turn, not just at boot — a provider can only be swapped by restarting the sidecar today, so
  boot-time labels are *currently* equivalent to per-turn labels, but the field should still be
  shaped as per-turn so a future hot-swap doesn't silently become a stale claim.
- **Location**: attach as a `provider_telemetry` object on the same response envelope `mode` lands
  on (§1) — one place a caller (or Track E's harness) reads both facts together, not two endpoints
  to correlate by hand.

## 3. What Track E already has, with no dependency on §1/§2 landing

Track E's E2 real-chain smoke script and E4 Q score do **not** block on this contract landing —
they get evidence today through the paths that already exist, and this doc's job is to remove the
need for that indirection once B/C/D land the fields above:

- **LLM adapter identity, today**: `backend/gateway-sidecar/src/devlog.ts` already writes
  `dev-logs/gateway-sidecar.ndjson` with an `adapter` field on every `gateway.response` event
  (`backend/gateway-sidecar/src/index.ts`, `devLog('gateway.response', { tenant_id, adapter, ... })`).
  `scripts/smoke-real-chain.ts` correlates this file by `tenant_id` after calling `/v1/respond`.
- **TTS provider identity, today**: `GET http://127.0.0.1:8083/healthz` on the running
  voice-provider-sidecar process, as described above.

Once `provider_telemetry` is a real field on `/v1/respond`'s response, `scripts/smoke-real-chain.ts`
and `evals/quality_score`'s trace ingestion should be pointed at it directly and the dev-log/healthz
correlation kept only as a fallback (not deleted — it is the only signal that also works for the
existing 6 `e2e-human-simulator` journeys, which predate this contract and do not send `mode`).

## 4. Veto-gate consumers of this contract

`evals/quality_score/qscore.py` (E4) applies a hard veto — `Q = 0` regardless of every other
sub-score — when `provider_telemetry.llm_adapter == 'memory'`, or a native-TTS fallback is
detected, or dead air exceeds threshold, or a silent fallback occurred. If `provider_telemetry` is
absent from a trace entirely, the veto check falls back to the dev-log/healthz correlation in §3;
it does **not** default to "not fake." Missing evidence is reported as `[TBM]`, never treated as a
pass — see `evals/quality_score/README.md`.
