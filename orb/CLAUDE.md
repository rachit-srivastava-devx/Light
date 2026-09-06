# adhd-focus-orb — Claude Agent Instructions

This file is the Claude entrypoint for the Focus Orb product. It inherits the Company-OS
constitution and the product rules in [`AGENTS.md`](AGENTS.md). Read `AGENTS.md`,
[`docs/BUILD-DIGEST.md`](docs/BUILD-DIGEST.md), and the relevant
[`blueprints/ADHD-Focus-Orb-L8-Deep-Dive`](../../../blueprints/ADHD-Focus-Orb-L8-Deep-Dive/)
section before changing code.

## L8 execution standard

L8 means integrated, evidence-backed behavior—not a large diff, many interfaces, or a green unit
test island. Work on exactly one slice at a time. State its entry point, owning plane, registry
decision (`install`, `extract`, or `build-new`), and acceptance evidence before editing. A module
without a production caller is a contract or scaffold, not a completed feature.

Agents must preserve these bars:

1. **Audio bed first:** create the bed once; keep it present through failures; stop it only for the
   explicit pause/session-end contract. Wire lifecycle, interruption recovery, ducking, and gap
   measurement. A graph constructor or interruption classifier alone is not enough.
2. **Deterministic control:** production must connect transcript → intent → router → FSM/step gate
   → policy/prosody → response envelope. LLMs fill only schema-locked language/evidence slots;
   they never choose state, route, authority, completion, session end, or tone.
3. **Indexed steps:** validate atomizer output, repair once, fail closed, and speak steps by index.
   Never speak the raw task or infer `done` from silence, timeouts, or ambiguity.
4. **Identity and cost at every boundary:** carry `tenant_id`, `user_id`, and `session_id` through
   every client, relay, provider, event, and usage contract. Reserve budget before paid work and
   key admission/accounting by tenant and session. T0 fake metadata must never be presented as
   production evidence.
5. **Real registry composition:** manifests and READMEs do not prove wiring. Verify the runtime
   call path; do not claim LLM or cost-plane composition unless those contracts are actually used.
6. **Evidence matches claims:** threshold APIs, caller-supplied metrics, smoke checks, and isolated
   unit tests are scaffolding. Audible claims require voice-to-voice audio evidence; deterministic
   claims require replay/consistency evidence; cost claims require cost replay.
7. **Correct placement:** ADHD-specific prompts, policies, and evalsets belong in `domain/`; generic
   capabilities belong in the registry; dependency direction is app → feature → service → core.
8. **Honest rung:** Phase 1 targets the 20K-user thin-cloud rung, not a 50M-user platform. Keep the
   RN app buildable, dependencies explicit, and verify the real device/audio path.

## Numeric acceptance anchors

Use the blueprint’s numbers: 0ms unasked-for silence; 0 gap events; bed audible within 120ms;
−12dB ducking over 80–150ms; barge-in ≤100ms; voice-to-voice p50 ≤1.1s and p99 ≤2.0s;
empathy ≥95%; approximately ₹4 hard session reservation; ≤₹120 per active user/month.

## Anti-slop rules

- Do not represent no-op routes, fake providers, stale paths, or future-tense comments as complete.
- Do not use fixtures or metadata to simulate cache hits, cost savings, latency, provider quality, or
  voice-to-voice success.
- Every module needs a real caller or an explicit scaffold label; every test must state the invariant
  it proves.
- Green unit tests do not override missing integration, manual audio verification, tenant isolation,
  cost admission, or an unbuildable app.
- Before handoff, run the verify gate and relevant smoke/eval/manual checks, update registry,
  manifest, ADR, and docs in the same change, report real output including failures, and list gaps.
  Untracked work is not shipped work.
