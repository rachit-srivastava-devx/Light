# ADR 0003 — Resolving the contract-suite's spec gaps against the blueprint

**Status:** accepted. **Date:** 2026-08-04.

## Context

The Tier-0 contract suite (`docs/adr/0001`) surfaced two gaps where `docs/BUILD-DIGEST.md` — the
compressed digest, not the blueprint itself — under-specified something load-bearing:

1. R1 Goal, R2 Attention, R3 Barrier registers were only partially enumerated in the digest (it
   quoted the policy guards, which don't name every member).
2. The FSM diagram in the digest has no edge into or out of `CLARIFY`, even though the state and a
   policy veto on it both exist.

Rather than inventing values, both were resolved by reading the owning blueprint files directly
(`blueprints/ADHD-Focus-Orb-L8-Deep-Dive/13-COGNITIVE-STATE-AND-POLICY.md` and
`12-LLD-AND-SESSION-CONTRACT.md`, both read-only — not modified).

## Decision

**Registers** (doc 13 §2, exact quotes):
- R1 Goal (7, exclusive): `No Goal · Goal Formation · Goal Renegotiation · Goal Commitment · Goal
  Maintenance · Goal Completion · Goal Decay`
- R2 Attention (7, exclusive): `Initiating · Focused · Exploring · Mind Wandering · Hyperfocus ·
  External Interruption · Recovering`
- R3 Barrier (8, multi-label vector, does not sum to 1): `Blocked · Uncertain · Overwhelmed ·
  Under-stimulated · Avoiding · Waiting · Fatigued · Emotionally Dysregulated`

Encoded in `apps/mobile/src/cognitive/contracts.ts` as `GoalState`, `AttentionState`, `BarrierFlag`.

**CLARIFY** (doc 12 §4): "Atomizing straight off a vague brain-dump produces vague steps. So
`INTAKE → CLARIFY → ATOMIZE`" — `ATOMIZE` in that sentence is the atomizer dispatch action, not a
session state, so CLARIFY re-enters the same `atomize_ready` edge INTAKE already uses once
`ClarifyProtocol.ts`'s slots (scope/first_context/blocker/time_box, capped at 2 questions) are
filled. Encoded as three transitions in `apps/mobile/src/session/contracts.ts`'s `TransitionTable`
comment (the table's actual data entries are `StateMachine.ts`'s to write, Tier 1).

## Consequences

- `GoalState`/`AttentionState`/`BarrierFlag` are now closed, fully-enumerated unions — safe to
  exhaustively `switch` over in `Policy.ts` (Tier 6) without a `SPEC GAP` risk of the union widening
  underneath it later.
- The CLARIFY transitions are documented but not yet data in a `TransitionTable` value — `union`
  interruption of INTAKE (INTAKE also needs a *direct* `atomize_ready → STEP_PRESENT` edge for the
  common case where clarification isn't needed) — `StateMachine.ts` must implement both the direct
  and the CLARIFY-routed path from INTAKE.
