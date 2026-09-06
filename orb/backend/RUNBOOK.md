# Backend runbook

Three processes (see [`../docs/adr/0004-backend-language-split.md`](../docs/adr/0004-backend-language-split.md)
for why each exists and what it owns).

## Start the stack

```bash
# 1. gateway-sidecar (TS) — the door onto @pe/llm-gateway. Uses vite-node because the registry
#    packages are TypeScript source packages in this workspace.
npm run start:sidecar:t0                              # :8082

# 2. relay-py (Python) — orchestration: atomizer, cost meter, warmup.
npm run start:relay:t0                                # :8765

# 3. relay-rs (Rust) — realtime audio data plane.
cd backend/relay-rs && cargo run                        # :8091
```

First-time Python setup: `cd backend/relay-py && python3 -m venv .venv && .venv/bin/pip install -e ".[dev]"`.

## Smoke test (what "working" looks like)

One-command local verifier:

```bash
npm run smoke:t0
npm run smoke:realtime
```

This starts the T0 gateway sidecar and Python relay, checks health, atomize, warmup, cache prime,
and eval metrics, then stops both processes.

`smoke:realtime` starts the Rust relay, opens a WebSocket, sends tenant/session tagged control
frames and mic bytes, then asserts transcript JSON plus binary TTS chunks come back.

Manual smoke:

```bash
curl -s http://127.0.0.1:8082/healthz                   # {"status":"ok"}
curl -s http://127.0.0.1:8765/healthz                   # {"status":"ok"}
curl -s -X POST http://127.0.0.1:8765/v1/session/warmup \
  -H 'content-type: application/json' \
  -d '{"tenant_id":"t1","user_id":"u1","session_id":"s1"}'
curl -s -X POST http://127.0.0.1:8765/v1/cache/prime \
  -H 'content-type: application/json' \
  -d '{"tenant_id":"t1","prompt_version":"atomizer.v4"}'

curl -s -X POST http://127.0.0.1:8765/v1/atomize \
  -H 'content-type: application/json' \
  -d '{"session_id":"s1","tenant_id":"t1","user_id":"u1","task":"file my taxes"}'
```

Against the **T0 memory adapter** this returns `"source":"fallback"` with two recorded rejections,
and that is correct, not a bug: the memory adapter is a deterministic fake that does not emit JSON,
so the atomizer's repair-once-then-fail-closed path fires exactly as designed
(`docs/BUILD-DIGEST.md` §5 degrade ladder rung 4). Real steps require the anthropic adapter wired
in the sidecar. Seeing `"source":"model"` here would mean the fake had started returning
schema-valid JSON — worth investigating, not celebrating.

## Live provider mode

T0 defaults to the registry memory adapter and needs no secret. To start the sidecar against the
registry Anthropic adapter instead:

```bash
ORB_LLM_GATEWAY_ADAPTER=anthropic ANTHROPIC_API_KEY=... npm run start:sidecar:t0
```

The sidecar fails fast if `ORB_LLM_GATEWAY_ADAPTER=anthropic` is set without `ANTHROPIC_API_KEY`.
No product code outside `backend/gateway-sidecar/src/` imports provider-facing gateway code.

Realtime STT/TTS uses the Rust relay provider contract. T0 stays fake:

```bash
cd backend/relay-rs && ORB_RELAY_PROVIDER=fake cargo run
```

A live STT/TTS sidecar can be used without changing the socket/session code by exposing:

| Endpoint | Request | Response |
|---|---|---|
| `POST /v1/stt/push` | `{"session_id": "...", "audio_bytes": [0, 1]}` | `{"text": "partial or null", "is_final": false}` |
| `POST /v1/stt/end` | `{"session_id": "..."}` | `{"text": "final transcript", "is_final": true}` |
| `POST /v1/tts/synthesize` | `{"text": "...", "voice_id": "...", "emotion": "..."}` | `{"audio_chunks": [[0, 1], [2, 3]]}` |

Start it with:

```bash
cd backend/relay-rs
ORB_RELAY_PROVIDER=http ORB_RELAY_PROVIDER_URL=http://127.0.0.1:9090 cargo run
```

This is a provider integration path, not live-provider evidence. Live proof still requires a sidecar
backed by real provider credentials plus a voice-to-voice latency run.

## Status codes the client must handle distinctly

| Code | Meaning | Retry? |
|---|---|---|
| 400 | missing/empty `tenant_id` (C8) | no — caller bug |
| 402 | session reservation or tenant budget exhausted (C12/INV5) | no — stop spending |
| 422 | request failed schema validation | no — caller bug |
| 503 | gateway-sidecar unreachable | yes |

## Verify

```bash
npm run verify   # from the repo root: boundary-lint + tsc + vitest + pytest + cargo test
```
