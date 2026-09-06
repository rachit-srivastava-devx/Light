---
name: speed-of-thought-fleet-rules
description: Owner's standing rules for the Speed-of-Thought / fleet-rs initiative — who the workers are, what must never be trusted blindly, the UX/latency/registry/scheduling constraints, and the honesty discipline. Use whenever operating fleet-rs, dispatching a lane, reviewing what fleet produced, or making a product/architecture call for this initiative. Generic coding discipline (reuse-first, verify-at-the-claim, $?-after-pipe) is l8-code's job, not this file's — this file only holds what is specific to Speed-of-Thought.
---

# Speed-of-Thought / fleet rules

This is the human-facing **index**, not the enforcement mechanism (`11-THREE-MEMORY-LAYERS.md`
§6.2: a lessons document is retrieval at best, and retrieval is advisory — the load-bearing
scar behind this file's own existence is `E1`, documented in `l8-code` and re-violated 6× anyway).
Every rule below is tagged with where it is **actually** enforced today. `GATED` means a real
check refuses the bad case; `ADVISORY` means nothing stops the bad case yet except a human or an
agent reading this file. **Do not report a rule as followed because it is written here.**

Captured 2026-08-28. Source: owner's `/goal` directive, in the order given.

## 1. Roles and trust

| Rule | Status | Where |
|---|---|---|
| Sonnet is user + reviewer + QA; Sonnet, codex, and fleet are the workers | ADVISORY | operating convention — no structural enforcement that a *review* step ran a different model than the *build* step |
| Don't trust any worker — review everything a worker returns | PARTIAL-GATED | `keel`'s independent-verifier requirement (`swarm::tests::verifier_model_gate_refuses_same_and_accepts_distinct` — refuses when the verifier is the *same model* as the builder, per `memory/2026-08-24-swarm-dispatch.md`); does not cover a human skipping review of a *fleet* worker's own output |
| Don't grade your own work — get an independent pass | GATED | same verifier-distinctness test above; `E1` (`memory/lessons/E1-pipe-exit-code.json`) went through exactly this: self-authored initially, then two rounds of independent adversarial review (fresh subagents) that found and mostly fixed real bugs — now `Enforced`, with 2 narrow disclosed gaps, not self-graded |

## 2. Real-user verification, not proxies

| Rule | Status | Where |
|---|---|---|
| Use the app as an actual user, not just its tests | ADVISORY | `webapp-testing` skill / `Claude_Code_iOS_Simulator` tooling exist and must be *invoked*, not just installed — `adoption-requires-a-real-run` is the standing scar for a tool defended without ever executing |
| A free tool that drives the app like a real user (Gemini-evaluate-simulated equivalent) | **NOT SOURCED YET** | no such tool identified or adopted in this repo as of this session; do not claim one exists until it has actually run once |
| E2E tests with Sonnet agents simulating a real human conversation, mid-turn topic changes and returns | **ABSENT** | `10-REALTIME-VOICE-AND-LATENCY.md` / orb E2E harness — not in fleet-rs's scope; track against the orb repo |
| Golden datasets, varied cases, fetched from GitHub/HuggingFace without the owner sourcing them | **ABSENT** | no dataset pipeline exists yet in fleet-rs |
| It must be genuinely UX-friendly *when tested by the operator directly* | ADVISORY | no substitute for the owner actually driving it; a passing gate is not this |
| Look at the actual pixels — an agent reported "no visible flat edge" once and there was one | ADVISORY, scar | see `gate-must-assert-rendered-state` memory; applies whenever a UI claim is made |

## 3. Depth, gates, and reuse

| Rule | Status | Where |
|---|---|---|
| Pure L8 depth — every small thing tested and verified | PARTIAL | `verify.sh` runs 16 stages (fmt/clippy/tests/deny/audit/secrets/acceptance/readme/swarm/policy/mutants/attest-smoke/pytest/detectors/corpus); **2 of 16 are currently red** — see status below. A green `verify.sh` is the floor, not the bar (`L8-CODING-RUBRIC.md`) |
| Reuse-first is a gate, not a preference | **PARTIALLY ABSENT** | fleet-rs has no product-registry consult (`ORB-AND-FLEET-DELTA.md` gap C3) — there is no mechanism today that *refuses* a build-new duplicating a `registry/` capability. This is a real, open gap, not a rule to just "remember" |
| Production gates: security, performance, linting, hooks, plus tools trying to break the app | MOSTLY GATED | clippy (lint), cargo-audit, cargo-deny, gitleaks, **semgrep (pinned rulesets, `bin/semgrep-gate.sh`)**, **trivy (secret scanner only — `bin/trivy-gate.sh`; vuln/misconfig blocked by a stalled DB download in this environment, tracked TODO)** all ok as of 2026-08-28 (`docs/delta.d/S1-semgrep-trivy.md`); no coverage measurement, `bin/perf-gate.sh` unwired, no adversarial/fuzz tooling — still open, see B11 |
| Do not create anything from scratch — use tools/libraries/plugins/frameworks; this is a solved problem | ADVISORY, but load-bearing | `solved-problem-adopt-dont-author` memory: nine regex patches once meant the component itself was wrong, not its rules. Applies directly to §2's "free tool" ask above — go find one before writing one |
| Handwritten half-baked unreliable code is not acceptable | ADVISORY | judgement call, not mechanisable (`11-THREE-MEMORY-LAYERS.md` §4.5's honest 42.6%) |
| Registry holds features, services, and apps; apps just stitch them | ARCHITECTURAL DECISION, not yet a gate | matches `Company-OS` C1/L2 (`registry/{features,services}`); fleet-rs's own product-registry consult (see reuse-first row above) is the missing enforcement |
| Always engineer a general solution | ADVISORY | judgement call; the closest structural proxy is `arch.rego`'s single-entry-point / no-hand-rolled-permutation checks, which catch a narrow special-cased shape, not generality broadly |
| **Never lock to one provider — support multiple, ideally every installed harness/CLI** (owner, 2026-08-30) | **NOT BUILT — architectural, tracked as S5** | today `crew/crew/adapters/` and `route.rs`'s `Role`/adapter enum are hardcoded to exactly `claude` + `codex`. The generalization (discover installed CLIs, a plugin-style adapter registry instead of a fixed pair) is real work, deliberately NOT started live: S1 (a concurrently-running worker as of 2026-08-30 02:xx) is actively editing this exact routing code, so touching it now would collide. See `S5` in `handover/BACKLOG.md`, depends on S1 landing first |

## 4. Latency, cost, parallelism, scheduling

| Rule | Status | Where |
|---|---|---|
| 250ms hard constraint on time-to-first-token | **NOT MEASURED IN FLEET-RS** | this is an orb/voice-plane number (`10-REALTIME-VOICE-AND-LATENCY.md`); fleet-rs has no TTFT instrumentation — do not claim this is met without a real measured trace |
| Parallelise without asking permission | **NOT BUILT** | `swarm dispatch` currently runs role-lifecycles sequentially, not concurrently (`ORB-AND-FLEET-DELTA.md` gap C8, P3 in the phase plan) |
| Be token-efficient — route most work to Sonnet and codex | PARTIAL-GATED | `route.rs`'s 6-stage router exists but is **not wired into `run_with_evidence`/`swarm dispatch`** yet — the highest-leverage one-line-ish fix named in the P1 phase |
| Schedule every half hour, because codex stops mid-way | **NOT CONFIGURED IN THIS REPO** | no cron/`scheduled-tasks` entry exists for fleet-rs as of this session; `bin/codex-fanout.sh`/`bin/codex-tick.sh` exist as the retry mechanism but are not on an automatic half-hour timer here |
| Keep records: observability, cost meter, token meter, LLM-as-judge | PARTIAL | ledger + scorecards + meter (`ratchet.rs`) exist and are evidence, per `11-THREE-MEMORY-LAYERS.md` §4 they are **not memory** in the L3 sense; the meter is not wired into the dispatch path yet (records nothing from real work today, per `handover/PROGRESS.md`) |

## 5. Honesty and reporting

| Rule | Status | Where |
|---|---|---|
| If fleet is triggered, write how it performed and what needs improving | ADVISORY | no automatic post-run report generator identified; currently a manual discipline |
| Standing law: say what you did not do — an honest gap is information | practiced in this file | every "NOT BUILT" / "ABSENT" / "NOT SOURCED YET" row above is this rule in action |
| A check cheaper to fake than to satisfy will be faked | standing law | governs how every row above is graded — a row is only `GATED` if a bad fixture was proven to fail it, per `policy/run.sh`'s and `bin/recur-gate.sh`'s "prove both directions" convention |

## 6. Collaboration mode (governs how *I*, not fleet, operate)

Ask clarifying questions, discuss by default, offload mental load like a teacher, keep the
conversation going, research how to actually talk to someone with ADHD (`adhd-conversation-design`
skill, already adopted — 59 literature searches, evidence-tagged). Always web-search before
trusting recalled facts — assume memory is outdated. Check how ChatGPT/Gemini/Claude Code are
actually built today before assuming an architecture. These are operating instructions for the
session, not gates `fleet` can enforce on itself — they live here as the index, and in practice
they are enforced by nothing but this file being read. **That is itself the L3 scar this file
exists to name**, not to pretend away.

## 7. Rules found the hard way, this session — and standing law

Not new rules — the owner's own corrections from earlier work, plus this estate's standing law
(`fleet/PRINCIPLES.md`). Listed here, explicitly, because §1 of this file was missing them: they
were *applied* throughout tonight's work (see the citations) but never enumerated as their own
checklist, which is exactly the gap a reader of this file would hit.

| Rule | Applied tonight, where |
|---|---|
| A new gate that's green on arrival is a broken gate, not clean code | `semgrep --config=auto --quiet` returned exit 0 with zero output — indistinguishable from "ran nothing." Did not trust it; confirmed `--metrics=off --config=auto` errors outright, switched to pinned rulesets. `docs/delta.d/S1-semgrep-trivy.md` |
| Write red proofs from the verbatim shipped text before trusting any gate | `bin/recur-gate.sh --selftest` proves both directions on synthetic fixtures, then re-proven against a real reintroduction of E1 in `bin/mutants-gate.sh` and a real clean edit to `bin/lane-probe.sh` |
| A false positive that vetoes a good reply is worse than any miss | governed the semgrep exclude-list decision: 31 findings, 100% reviewed by hand as false positives for this codebase's actual patterns, not blanket-suppressed without reading each one |
| Sentence-scope a content check, or a good opening laundres a bad closing question | not directly applicable to this session's Rust/gate work (this rule is content-moderation-shaped); no content-check gate was built here to misapply it to |
| Enforcement over documentation | the organizing principle of tonight's whole L3 build: `memory/lessons/` alone would have been exactly the "lessons document" `11-THREE-MEMORY-LAYERS.md` §6.2 names as the estate's canonical failure — `bin/recur-gate.sh` is the enforcement half, not a substitute for it |
| Standing law: say what you did not do — an honest gap is information | every `ADVISORY`/`ABSENT`/`NOT BUILT` row in §1-4 above, plus this file's own S1 note: "not marking `[x]`: acceptance says per-role, what's built routes one agent for the whole dispatch" |
| Standing law: don't grade your own work — get an independent pass | `E1` (`memory/lessons/E1-pipe-exit-code.json`) went through two rounds of independent adversarial review before reaching `Enforced` — this row itself was wrong earlier tonight (said "still Candidate" past the point the schema's own bar was actually cleared) and got corrected once re-checked against the schema's literal criteria |
| Standing law: a check cheaper to fake than to satisfy will be faked | the reason `--config=auto` was rejected (see row 1) and the reason E1's gate has synthetic + real positive/negative controls, not just a synthetic pass |

**Honest limit, not claimed away:** this table shows these rules governing *my own* engineering
choices tonight. It does not show `recur-gate` or any other mechanism catching a live E1-shaped
mistake mid-session — the mistakes actually made tonight (a too-strict test assertion, a
`diff -u`-vs-`git diff` header mismatch, a D53 error-ordering slip) were outside what any of
tonight's gates are scoped to catch, and were caught by manual review and re-testing, not by the
rules framework firing automatically. That is the honest boundary of what exists today, not a gap
to paper over.

## Verified `verify.sh` status (2026-08-28, this session)

13 passed, 2 failed, 1 skipped of 16 stages, run in the background while this session was doing
unrelated concurrent work — treat the mass of `TIMEOUT ... exceeded 30s` rows in the `corpus`
stage as **unconfirmed** until re-run in isolation (resource contention is a live alternative
explanation, not yet ruled out — `check-for-live-writers-before-ab` discipline). The one failure
that does **not** smell like contention: `illegal_lifecycle_transitions_do_not_compile` (a
`trybuild` test asserting illegal state transitions fail to compile) — this needs a real look,
not a re-run, because either the type-state guarantee regressed or the fixture's expected compiler
text drifted. Next action: `cd keel && cargo test illegal_lifecycle_transitions_do_not_compile
-- --nocapture` and read the actual trybuild diff.

## `bin/recur-gate.sh` — the first real `keel-gate::recur`

Built and proven both directions this session (`bin/recur-gate.sh --selftest`), then hardened
through two rounds of independent adversarial review (fresh subagents, no stake in it looking
good): round 1 found 6 false negatives + 3 false positives, 8 of which are now permanent
regression fixtures (8/8 passing); round 2, of the fixed version, found 2 more narrow ones,
deliberately left open and disclosed rather than rushed. `E1` is `Enforced` — see
`memory/lessons/E1-pipe-exit-code.json` for the full review trail. **Wired into `verify.sh`** as a
required stage (`.githooks/pre-commit` runs `verify.sh` in full, so it's covered transitively).
L3's concrete next step is a second lesson, not re-litigating this one.
