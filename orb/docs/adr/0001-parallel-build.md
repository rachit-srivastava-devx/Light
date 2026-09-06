# ADR 0001 — Parallel worktree build, overriding "one slice at a time"

**Status:** accepted. **Date:** 2026-08-04.

## Context

`../../../Company-OS/AGENTS.md` and the root `AGENTS.md` session ritual both default to "one
slice at a time... don't one-shot the product." The human's `/goal` for this build explicitly asked
to parallelize as many agents as possible across git worktrees, with Opus reviewing each worktree's
output before merge, and to keep going autonomously until Phase 1 is functionally complete.

## Decision

Per the constitution's own precedence rule (a one-off human instruction wins for its task; note the
divergence), this build:

1. Parallelizes tiers of `docs/BUILD-DIGEST.md` §8 that have no real data dependency between them
   (e.g. the noise engine/audio graph and the pure state-machine skeleton can be built in sibling
   worktrees simultaneously — neither reads the other's output at build time).
2. Serializes strictly where a genuine dependency exists (Router needs the FSM's state enum fixed;
   Policy needs the belief model complete) — a worktree for a Tier-N module is only opened after the
   Tier-(N-1) module it depends on has an Opus merge verdict on `main`.
3. Every worktree's output is reviewed by an Opus `lead-architect` agent against: (a) not AI slop,
   (b) readability, (c) manually driven/tested rather than trusting unit tests alone, (d) repeats no
   mistake already logged in `docs/adr/LESSONS.md`, (e) a written go/no-go verdict before merge, (f)
   every function/line accounted for against the L8 rubric, (g) no wasted rework.
4. Contracts (interfaces, schemas, the FSM's state enum, the atomizer schema) are written FIRST by
   the lead-architect, before any parallel build work starts on modules that consume them — this is
   what makes the parallel worktrees safe: they build against a fixed contract, not against each
   other's in-progress code.

## Consequences

- Slightly more coordination overhead up front (contracts-first) in exchange for real wall-clock
  parallelism without merge conflicts or rework.
- If a contract turns out wrong after parallel building starts, every worktree built against it
  needs a coordinated fix — this is the risk this ADR accepts in exchange for speed, and the reason
  contracts are reviewed by the lead-architect before any parallel work begins.
