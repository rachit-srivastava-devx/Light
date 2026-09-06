# Junior traversal guide

Read in this order when you are new to the product:

1. `AGENTS.md` for the non-negotiable rules and current divergences.
2. `docs/BUILD-DIGEST.md` sections 1, 2, 3, 5, 6, and 8 for the executable product shape.
3. `docs/adr/LESSONS.md` before editing. A repeated logged mistake is a review failure.
4. `manifest.json` to see which registry services are installed instead of rebuilt.
5. `docs/adr/0006-registry-voice-extraction.md` to see what moved into `registry/`.

## Planes

- `apps/mobile/src/App.tsx`: the one-screen shell. It renders the envelope and owns no decisions.
- `apps/mobile/src/runtime/`: T0 app runtime glue. It consumes `@pe/realtime-voice`; it does not
  implement realtime voice primitives. `VoiceLoopController.ts` is the executable client loop that
  wires VAD, endpointing, backchannel, barge-in, relay frames, and the T0 runtime together.
- `apps/mobile/src/presence/`: the local audio bed. It must not import voice or cognitive code.
- `apps/mobile/src/voice/`: VAD, endpointing, barge-in, backchannels, and relay frames.
- `apps/mobile/src/session/`: the deterministic FSM, step gate, check-in timer, and cost reservation.
- `apps/mobile/src/router/`: intent labels and route table. Completion is explicit only.
- `apps/mobile/src/cognitive/`: belief updates, evidence tiers, and intervention policy.
- `apps/mobile/src/lld/`: context pack, clarify protocol, prosody, response envelope, voice routing.
- `backend/gateway-sidecar/`: wraps registry `@pe/llm-gateway`; provider-facing TypeScript only.
- `backend/relay-py/`: atomizer orchestration, warmup/cache routes, cost, eval gates, observability.
- `backend/relay-rs/`: realtime socket/session data plane.
- `../../../registry/services/voice-realtime/`: reusable realtime voice service contracts and T0 fake
  adapter.
- `../../../registry/features/realtime-voice/`: reusable foreground voice-session feature.

## Before you change code

- Say the C1/L2 branch: install, extract, or build-new.
- Keep pure logic pure: inject time, no random, no provider calls.
- Add missing/malformed/threshold tests next to happy-path tests.
- Run `npm run verify` before claiming done.
- Run `npm run smoke:t0` before claiming backend/runtime integration works.

## Manual smoke path

1. `npm run smoke:t0` starts the gateway sidecar and Python relay, drives the T0 HTTP path, and
   stops both processes.
2. `GET /healthz` returns `{"status":"ok"}` from both sidecar and relay.
3. `POST /v1/session/warmup` returns profile, empty T0 context arrays, open session, and phrase manifest.
4. `POST /v1/cache/prime` returns a T0 source marker.
5. `POST /v1/atomize` through the T0 memory sidecar returns the deterministic fallback path.
