# OrbMac SDLC plan — module breakdown, dependency graph, task allocation, quota log

Written after the fact to formalize what was actually followed turn-by-turn (contract-first, one
slice at a time, worktree-per-unit) into a single reference document, per the AI-Native SDLC gate
model already summarized in FLEET-LEARNINGS.md (G0-G9, autonomy ladder A0-A5, author != integrator).

## Module dependency graph

```
M1 scaffold (SPM package, SwiftUI shell)
 ├─ M2 protocol client (WireProtocol + RelaySocket)      ─┐
 ├─ M3 audio I/O (MicCapture + TTSPlayback)                ├─ independent, parallel
 ├─ M4 orb visual (OrbVisualState + OrbView)              ─┘
 └─ M5 app shell (3-pane UI, ChatViewModel, PanelState) — depends on M1 only, built parallel to M2-M4
      ├─ M6 wire-session-lifecycle (SessionController)     ─┐
      ├─ M7 wire-mic-input (MicInputCoordinator)             ├─ depend on M5 + M2/M3, parallel to each other
      ├─ M8 wire-tts-playback (TTSPlaybackCoordinator)       │
      └─ M9 wire-orb-state (OrbStateDeriver)               ─┘
```

## Task allocation (this session)

| Unit | Tool | Verification |
|---|---|---|
| M1-M9 | Claude (mid-engineer, contract-first: test written before implementation in every unit) | Independent verifier agent per unit, cross-checked source values against the mobile app's real TS where a port existed (M4 presets, M9 state-derivation) |
| Mutation testing (G4) on M2/M3/M4/M5/M6/M7/M8/M9 | Mix of Claude (verifier role) and opencode | Each: baseline green -> 3 targeted mutations -> kill/survive recorded -> clean revert. Found and fixed 1 real gap (M7's state-ordering, resolved as a documented false positive with a new proof-test), 1 real defect fixed separately (A9's mutation-gate follow-up, prior in this session) |
| Backend defect-hunt (relay-py) | codex | 5 findings, severity-ranked, 1 fix dispatched (cost-reservation race) |
| Eval-gate migration (cost, voice-loudness) | Claude (lead-architect for the scoping pass, mid-engineer for implementation) | Independent verifier re-ran real captured-data gates, found + the reconciliation-guard looseness (disclosed, not blocking) |

## Why tests were written during dispatch, not as a separate upfront pass

Every unit's dispatch prompt specified the exact test-first sequence (write the failing test, confirm
RED, implement, confirm GREEN) as part of the brief — this is TDD-in-the-brief, not "tests added
after." The distinction the SDLC playbook draws is spec before code, not spec-as-a-separate-document
before spec-as-a-prompt; each unit's prompt WAS its spec, written by the orchestrator before that
unit's agent touched any file.

## Quota log (consolidated from FLEET-LEARNINGS.md's scattered entries)

| Time | codex | opencode | Claude | Note |
|---|---|---|---|---|
| 21:40 | available | available | — | session start |
| 23:50 | **exhausted** (resets 02:13) | available | available | routed codex-slated work to opencode/Claude |
| 01:49 | still locked | 2 running | 1 running | rechecked, still locked |
| 02:20 | **available** | idle | several running | confirmed with a live ping, dispatched real work (defect-hunt, cost-overspend fix) |

## Honest completion state (not 90% — stated plainly, not padded)

- ADHD-Focus-Orb backend/mobile: ~15 defects fixed and merged, verified.
- Speed-of-Thought P0 (U1-U5): fully built and merged prior to the macOS work in this same session.
- OrbMac native macOS app: 9 modules built, tested, mutation-tested — **held from `main`**, per this
  session's own manual-verification rule, until a human has actually run the app (asked, not yet
  done as of this doc's writing).
- Eval-gate migration: 2 of 5 gates done, on an unmerged, human-review-only branch.
- 5 new relay-py defects found, 1 fix in flight, 4 not started.
- This is a real, substantial fraction of two blueprints' P0 phases — not 90% of either blueprint's
  full multi-week scope, which was never realistically achievable in one session (see this session's
  very first message: Speed-of-Thought's own phase doc estimates P0 alone at 1-2 weeks single-builder).
