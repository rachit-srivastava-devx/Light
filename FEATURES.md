# FEATURES — the atomic build list, ordered by time-to-visible-output

Derived from `blueprints/Speed-of-Thought-L8-Deep-Dive/` — the artifact LLD
(`https://claude.ai/code/artifact/8c76a74c-dc76-415c-8ff5-25aa6142ac47`), `ORB-AND-FLEET-DELTA.md`
(capabilities C1–C11), `PHASES-TO-USABLE.md` (P0–P4), and decisions D1–D15.

**The ordering principle (PHASES-TO-USABLE §0, applied one level more atomically):** not
foundations-first — whichever atomic feature produces something *you can see working* soonest goes
first. Within a phase, items are further ordered by dependency (an item cannot precede what it needs)
and, among independents, by which is smaller/faster. Phases P0→P4 are themselves already ordered this
way in the source docs; this file decomposes each phase into buildable, one-lane-sized units.

**One feature builds at a time per repo** (four repos: `orb`, `fleet`, `apps/macos`, `registry`) — so
up to 4 lanes can be active concurrently, one per repo, each working its own queue below in order.
Cross-repo integration features (tagged `orb+fleet` etc.) wait for every repo they touch to reach that
point in its own queue.

**Status legend:** ☐ pending · ◐ in progress (see `STATUS.md` for live detail) · ☑ done (verified
independently + manually driven). This file is the plan; `STATUS.md` (generated from `status/*.status`)
is the live tracker — update the checkbox here only when a feature is genuinely ☑.

**Realism note, stated up front (L8 rule: numbers, not adjectives):** at historical per-unit pace from
this same build's prior session (`FLEET-LEARNINGS.md`: 30 min–3+ hr per unit with real gotchas,
independent-verify included), and 2 initial concurrent lanes (`orb`, `fleet`), this session will likely
land somewhere in the P0–P1 range before the 10AM IST stop, not the full P0–P4 list. The full list is
here because the user asked for the whole atomic roadmap in one file, not because all of it will land
tonight. Say so honestly at each check-in rather than padding the done column.

**Found after this file was first written (2026-09-07 01:25 IST) — `fleet/.claude/skills/
speed-of-thought-fleet-rules/SKILL.md` (copied into `Light/fleet/` along with everything else, only
discovered by a system prompt after the copy).** This is the owner's own standing-rules index for this
exact initiative, captured 2026-08-28. Every fleet-touching lane's lead-architect should read it. Two
things in it change this file:
- **§3: "Never lock to one provider — support multiple, ideally every installed harness/CLI"** (owner,
  2026-08-30) — **NOT BUILT**, tracked upstream as backlog item S5, deliberately not started because a
  concurrently-running worker was mid-edit on the exact same routing code at the time. `route.rs`'s
  adapter enum is hardcoded to exactly `claude`+`codex` today. Added as **F41** below (P1) — this was a
  real gap in the original derivation, not a duplicate of F13 (F13 is *which model tier*; F41 is *which
  CLI/harness executes the dispatch at all*).
- **A specific, already-flagged possibly-still-red test**: `illegal_lifecycle_transitions_do_not_compile`
  (a `trybuild` compile-fail test in `keel`) was one of 2 real failures in a 16-stage `verify.sh` run on
  2026-08-28 (13 passed / 2 failed / 1 skipped) — flagged then as needing "a real look, not a re-run,"
  because either the type-state guarantee regressed or the fixture's expected compiler text drifted.
  Re-checked here as part of S0 (see below) since F08's lead-architect is reading this exact file.

**ARCHITECTURE DECISION (2026-09-07 02:20 IST, owner correction — supersedes how F01 was scoped):**
`apps/macos` (OrbMac, native Swift) is Light's actual frontend — the original `apps/mobile` (React
Native, iOS/Android) is a different UI shell for the original adhd-focus-orb product and is **not**
what a Light user runs. Swift cannot import TypeScript, so **any dialogue/session logic living in
`orb/apps/mobile/src/**` (the RN client) is unreachable from `apps/macos`.** The owner's explicit
call: this logic moves **server-side**, into `orb/backend/relay-py` (or `relay-rs` where it's
Rust-owned) — *any* client (Swift or the old RN app) drives it over the existing relay WebSocket
protocol, with zero per-client copy of the FSM/belief-model/build-mode state. This is fork (b) of
`18-THE-LIGHT-APP-AND-UI.md` §7's own explicitly-left-open question ("ports to Swift... OR stays
reachable via the relay") — now resolved, not open.

**Consequence for F01 (already merged):** it modified `T0FocusSession.ts`/`App.tsx` — RN client code
`apps/macos` cannot reach. Not reverted (the logic is sound and the tests are real), but its actual
capability is **not yet usable from the real Light frontend**. F04 below is redefined to close this
gap by moving build-mode session state server-side, rather than as a second, separate row — treat F01
as "proved the concept client-side," F04 as "makes it real for the app that matters."
**Every orb-tagged row from here on states explicitly whether it targets `backend/relay-py`
(reachable by both clients — the default target for anything new) or `apps/mobile/src` (RN-client-only
— avoid for new work; only touch it if the feature is genuinely about the old iOS product itself).**

---

## Prerequisite (done, not a fanned-out feature)

| # | What | Repo | Status |
|---|---|---|---|
| S1 | Copy `adhd-focus-orb`+`fleet-rs`+`OrbMac`+`fleet/registry` into `Light/`, reorganize to the D15 shape, fresh git history | Light (all) | ☑ |
| S0 | **Baseline gate check** — before any new feature lands, confirm each copied repo's *own, pre-existing* verify gate still passes unchanged in its new location (or fails identically to the original — a copy must not silently break or silently fix anything). Specifically re-check `illegal_lifecycle_transitions_do_not_compile` in `fleet/keel` (flagged possibly-red 2026-08-28, see note above) — in progress, `/tmp/f8-lifecycle-check.log` | orb, fleet, apps/macos | ◐ |

---

## P0 — talk it, watch it build one thing (target: 1 module, spoken → frozen → attested → PR, no hand-written spec)

| # | Feature | Repo | Depends on | Branch (C1) | Acceptance (one line) | Status |
|---|---|---|---|---|---|---|
| F01 | **Orb build-mode toggle** (client-side, `apps/mobile`) — a second conversation mode on the existing `ConversationPort`+FSM+gateway. **Built, merged, verified — but proved the concept in the RN client only; `apps/macos` cannot reach it (see architecture note above).** Superseded as the real path by F04 | orb (`apps/mobile` — RN client, not reachable from `apps/macos`) | S0 | install (reuse FSM/gateway) | Launching in build-mode produces a build-mode-specific greeting; focus-mode's existing test suite is unchanged and still green | ☑ (client-only; gap tracked, closed by F04) |
| F02 | lld.v1 contract - PASS, merged, found+fixed 2 more cross-lang bugs | fleet+orb | ☑ |
| F03 | Module-brief atomizer wired - PASS-WITH-CONCERNS, merged (fixed 3 cross-lane test conflicts + blind-repair bug) | orb | ☑ |
| F04 | **REDEFINED 2026-09-07 (was client-side `BeliefModel.ts` only) — server-side build-mode session + depth-completeness belief registers, in `relay-py`.** Closes F01's reachability gap: build-mode session state (which mode a session is in, the build-specific greeting/system-prompt) and the re-aimed belief math (spec-completeness/ambiguity/coverage log-odds registers, same κ/decay law as the client's `BeliefModel.ts`, ported to Python) both live in `backend/relay-py`, driven over the existing relay protocol — so `apps/macos` (Swift) *and* the old RN client can both select build mode and see belief-driven behavior with zero client-side FSM/belief copy | orb (`backend/relay-py` — NEW target, reachable by both clients) | S0 | build-new (server-side; port the client math, don't re-derive it) | A session opened with `mode=build` over the relay (from any client, including a raw WebSocket test client with no RN/Swift involved) gets a build-specific greeting and a real completeness register that moves on scripted evidence; existing focus-mode relay behavior is unchanged | ☑ PASS, merged |
| F05 | Freeze protocol (info-gain stopping rule) - PASS, merged | orb | ☑ |
| F06 | lld-ready gate - PASS, merged | fleet | ☑ |
| F07 | Fleet SOW-intake extension - PASS, merged (closed a real freeze-hash-forgery gap) | fleet | ☑ |
| F08 | PR-emit step - PASS-WITH-CONCERNS, merged. 2 follow-ups flagged (not blockers): gh-hiccup retry dead-end, non-atomic test-dir naming | fleet | ☑ |
| F09 | **Orb→Fleet handoff wiring** — the ❄ freeze event actually calls fleet's extended intake (F07); no more "stop at a frozen `lld.v1` artifact" (the prior session's Track B boundary) | orb+fleet | F05, F07 | build-new (the seam) | Freezing a real module in a live orb build-mode session results in a real fleet SOW being created, observably (not just a documented intent) | ☐ |
| F10 | **P0 capstone: one module, spoken → frozen → built → attested → PR, no hand-written spec** — integration proof of F01–F09 together, on the reused fleet-rs kernel (`run_with_evidence`, unchanged) | orb+fleet | F01–F09 | reuse (kernel) + integrate | Drive one real, small module through the full path and watch a real PR appear with a real attestation attached — this is the P0 exit trigger, measured wall-clock freeze→attested-PR | ☐ |
| F11 | **Thin status echo** — "building… tested… attested, PR #N" spoken/shown back in the conversation (full graph is P2) | orb | F10 | build-new (minimal) | After F10's freeze, the conversation surfaces a real status line that changes as the lane progresses, sourced from the ledger (not a canned string) | ☐ |
| F12 | **L1 working memory: freeze ledger as a session object** — mostly INHERITED (`conversation_store` already exists); the delta is making the freeze ledger a first-class part of it | orb | F05 | extend (small) | A session's freeze ledger survives a reconnect within the same session; existing `conversation_store` tests untouched | ☐ |

## P1 — trust the verdict (make "fleet says done" mean done)

| # | Feature | Repo | Depends on | Branch | Acceptance | Status |
|---|---|---|---|---|---|---|
| F13 | **Model routing wired into the actual run** — re-verify current state first (artifact flags this STALE/in-flux as of 2026-09-06: `route::for_plan_with_builder` exists and `run_model_agent` takes a `model` param now, but whether every real call site populates it from a real routing decision is unconfirmed); close whatever gap remains | fleet | S0 | re-verify → extend or close | A dispatch through the real build path (not just `plan`/`route`) resolves a real, non-empty model per the task-type→tier table; `MODEL_NOT_EXPLICIT` fires on an unresolved one | ☐ |
| F14 | **Verifier≠builder enforcement, confirmed airtight** — `SELF_VERIFIED` refusal path, end to end | fleet | S0 | verify existing | A same-agent verify attempt is refused outright with a named reason, driven for real (not read from source) | ☐ |
| F15 | **Mutation adequacy non-optional on the lane gate** (currently opt-in) | fleet | S0 | extend | A lane with mutation kill-rate below the floor fails the gate by default, no flag needed | ☐ |
| F16 | **Review narration surface** — "here's exactly what changed, what it was tested against (REQ-IDs), what the verifier found, the one judgment call for you" | orb or apps/macos (whichever has a renderable surface first) | F10, F13 | build-new | A real completed lane's review narration is generated from its actual evidence bundle, not a template with blanks | ☐ |
| F17 | **Depth-bar tightening** — the `lld-ready` rubric gains eval/threshold checks (a freeze claiming a rate must carry sample-size math) | orb+fleet | F06 | extend | A freeze claiming "95% accuracy" with no `n` is refused by the gate, naming the missing derivation | ☐ |
| F41 | **Multi-harness dispatch** — generalize `route.rs`'s worker adapter from a hardcoded `claude`+`codex` pair to a discoverable, plugin-style adapter registry covering every installed CLI/harness (owner's explicit standing rule, `speed-of-thought-fleet-rules` skill §3; upstream backlog S5) | fleet | F13 | build-new (S5 was blocked upstream on a since-resolved routing-code collision — re-check that collision is actually clear before starting) | Fleet detects ≥2 installed CLIs at startup without a hardcoded list, and can dispatch a real task through either | ☐ |

## P2 — real-time voice + the live graph

| # | Feature | Repo | Depends on | Branch | Acceptance | Status |
|---|---|---|---|---|---|---|
| F18 | Streaming STT partials (replace buffer-until-`end_of_turn`) | orb | S0 | extend | Partial transcripts arrive before end-of-turn, measured | ☐ |
| F19 | Streaming TTS first-audio (replace buffer-then-chunk) | orb | S0 | extend | First audio byte arrives before full synthesis completes, measured | ☐ |
| F20 | `stream-live` gate — refuse the fake/batch provider on the real build path | orb | F18, F19 | build-new | A build-mode session configured with the fake provider is refused, naming why | ☐ |
| F21 | `design-graph.v1` contract + `lane-status.v1` **push** transport (D14 — today pull/refresh only) | fleet | S0 | build-new (new WS/SSE layer; contract itself `lane-status.v1` already ships) | A lane state change is pushed to a connected subscriber within budget, no polling | ☐ |
| F22 | Live graph render — one embedded `WKWebView` pane in `apps/macos`, hosting keel-console's existing SVG unchanged | apps/macos | F21 | reuse render pipeline + new pane | A real freeze→lane-state change renders a node update in the pane, sourced from the ledger (render-gate: no node without a resolving ledger source) | ☐ |
| F23 | Barge-in during design dialogue (reuse existing VAD/barge-in mechanism) | orb | F18, F19 | reuse | Speaking over the Orb mid-freeze cancels/redirects cleanly, no dead-air | ☐ |

## P3 — parallel lanes + reuse

| # | Feature | Repo | Depends on | Branch | Acceptance | Status |
|---|---|---|---|---|---|---|
| F24 | Real concurrent worktree lanes — `swarm dispatch` actually spawns N isolated lanes (today: sequential bookkeeping), capped `min(16,cores−2)` | fleet | S0 | build-new | 3+ modules build concurrently in isolated worktrees, observed via `ps`/lane logs, not claimed | ☐ |
| F25 | Registry `install/extract/build-new` gate — real C1 verdict per module node | fleet+registry | F24 | build-new | A build-new that duplicates an existing `Light/registry/` capability is refused, naming it | ☐ |
| F26 | Knowledge-map-before-build (tree-sitter symbol graph + dep resolution) | fleet | F24 | extend (single-repo engine already specified) | A lane refuses to guess against a stale map rather than building blind | ☐ |
| F27 | `planner` role (D8) — 6th fleet role, optional per module | fleet | F06 | build-new | An ambiguous module routes to the planner role and comes back with a named missing field, same convention as the other roles | ☐ |
| F28 | Extract `voice-io` service (relay + STT/TTS sidecars) → `Light/registry/services/voice-io` | registry (from orb) | F18–F20 | extract (2nd consumer: apps/macos) | `orb` and `apps/macos` both install the same service; no drift between the two | ☐ |
| F29 | Extract `dialogue-engine` feature (clarify-to-LLD, belief model, spec-decomposer) → `Light/registry/features/dialogue-engine` | registry (from orb) | F03–F05 | extract (2nd consumer: apps/macos) | Same as F28, for the dialogue engine | ☐ |
| F30 | Extract `keel-kernel` service → `Light/registry/services/keel-kernel` | registry (from fleet) | F10 | extract (2nd consumer: apps/macos, via the codegen boundary) | `fleet` and `apps/macos` both consume the same kernel; no forked copy | ☐ |
| F31 | Build `delivery-lane` feature → `Light/registry/features/delivery-lane` | registry (from fleet) | F24 | extract+build-new | The concurrent-lane executor is installable, not hand-copied | ☐ |
| F32 | `worker-payload.v1` schema (contract only — the enforcing gate is F34) | registry+fleet | S0 | build-new | Schema round-trips a fixture; no enforcement yet (that's F34) | ☐ |
| F33 | D10 trigger mechanism — explicit-freeze-or-fleet-decides, a confidence-score proposer + deterministic threshold gate (not a free-floating auto-trigger) | fleet | F06 | build-new | A musing-level utterance does not auto-trigger a lane; an unambiguous frozen module does, via the same propose→gate law as everywhere else in this system | ☐ |

## P4 — it injects itself + learns (hardest, highest-value; time-gated)

| # | Feature | Repo | Depends on | Branch | Acceptance | Status |
|---|---|---|---|---|---|---|
| F34 | **`no-ambient` injection gate** — a spawned worker carries bundled skills + `--mcp-config` (pointing at `fleet mcp`) + `--append-system-prompt`, stamped by the launcher; refuses a worker that would read the operator's ambient `~/.claude` | fleet | F32 | build-new — **the single biggest named risk in the whole system** (DELTA §7): may only reach the weak "pin-and-verify" form if the model CLI can't be fully isolated — prove the strong form or declare the fallback, do not silently assume it | A worker spawned through the launcher has zero ambient config in its env, verified by inspecting the actual spawned process, not by reading the launcher's intent | ☐ |
| F35 | Contract-codegen pipeline — keel JSON-Schema → Swift `Codable`, CI-gated against staleness (greenfield on all sides, not an existing pattern — see `18-THE-LIGHT-APP-AND-UI.md` §3.4's own correction) | apps/macos+fleet | F02 | build-new | A schema change without a regenerated Swift type fails CI; a fixture round-trips Rust-encode→Swift-decode | ☐ |
| F36 | **FAIL 2026-09-07** — opencode/mimo-v2.5-free built non-functional code (dead callback, wrong CoreAudio call order, a test with no assertion in either branch) and its own FLEET-LEARNINGS entry falsely claimed success. Real fix still needed | apps/macos | S0 | build-new | Barge-in yield measured ≤100ms with AEC active, on real hardware | ☐ blocked, needs re-build |
| F37 | Two-pane chat UI shell — native SwiftUI left pane (real conversation turns, not a mock), app shell + orb visual state already built | apps/macos | F01, F09 | extend (app shell exists; wire real data) | The real orb conversation (not the demo mock) renders live in the left pane while build-mode runs | ☐ |
| F38 | L3 procedural memory / learning loop — `capture → signature → keel-gate::recur` | fleet | S0 | build-new | A caught defect's signature refuses a repeat of the same pattern, fleet-wide, naming the lesson id | ☐ |
| F39 | Retro loop — failures become lints/gates/scaffolds on a cadence | fleet | F38 | build-new | A retro run produces at least one new enforced check from a real prior failure, not prose | ☐ |
| F40 | Local cost dashboard (D12) — small aggregation view over orb+fleet's existing in-path meters | apps/macos | F30 | build-new (small) | Both engines' real spend for one session render side by side, sourced from their existing meters, no new storage location | ☐ |

**Out of scope for this file (explicitly, per D11/the estate diagram):** the org-wide knowledge DB is
a 5th repo, entirely outside `Light/`, not started, not part of "every repo (orb, apps/macos, fleet,
registry)" the user scoped this list to.

---

## Known landmines to brief every lane on (don't rediscover these — they're already paid for)

- **`git stash` is a shared stack across every worktree of one repo — never use it for baseline
  isolation here.** Use a disposable `git worktree add --detach <tmp> <ref>` instead. (`FLEET-LEARNINGS.md`, hit 3 times last session.)
- **A bare `build/` gitignore rule swallows `orb/apps/mobile/src/build/`** — already fixed in this
  copy's `.gitignore` (negation added); don't remove it.
- **Worktree depth breaks `file:` deps and TS path aliases sized for the old checkout depth.** Verify
  fresh in `Light/` rather than assuming the old worktree gotchas still apply at the new depth —
  they may not (this is a different tree shape now), but check, don't assume either way.
- **A subagent cannot receive its own background children's completion notifications** — only the
  top-level session can. Never brief a lane to background a slow command and then wait; foreground it
  or use `timeout N <cmd>`.
- **Concurrent agents on shared files race.** `STATUS.md`/`status/*.status` is designed around this
  (one file per feature) — don't hand-edit `STATUS.md`, and don't have two agents touch one feature's
  status file.
