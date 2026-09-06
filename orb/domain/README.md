# Focus Orb domain pack

These files are ADHD-specific T0 inputs for the product's deterministic contracts. They are
versioned evidence scaffolds, not proof of live provider voice quality or physical-device audio.

- `policies/empathy-policy.v1.json` defines allowed emotional registers and hard vetoes.
- `agents/atomizer.v1.md` and `agents/check-in.v1.md` are schema-locked prompt inputs.
- `agents/focus-companion.v1.md`, `agents/converse.v1.md`, and `agents/teach.v1.md` are the
  open-domain conversation prompts for `/v1/respond`'s `mode: focus | converse | teach`, loaded
  from disk at runtime by `backend/relay-py/src/orb_relay/proxy/prompts.py` (never inlined in
  `app.py` — see that route's docstring for the placement rule this fixes).
- `agents/wait-companion.v1.json` is the deterministic rotating pool for wait-time company. The
  relay selects by session history, never RNG or wall clock, and fails closed before repeating.
- `evalsets/t0-contract-corpus.v1.json` is consumed by
  `backend/relay-py/src/orb_relay/eval/gates.py` and the `/v1/eval/metrics` route.
- `schemas/t0-contract-case.schema.json` documents the corpus record shape.

The evaluator expands the checked-in seed cases deterministically for the local replay gate:
300 atomizer cases, 200 voice replay cases, 200 consistency cases, 500 classifier utterances,
5,000 belief windows, and 200 cost/latency/policy-behavior replays. Those counts prove the T0
contract math and fail-closed wiring; the live voice-to-voice/device proof remains separate.
