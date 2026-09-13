# Fleet LLD Meta-L8 Addendum 2 — GVS5H-informed workflow closure

Date: 2026-09-13
Status: design closure draft; implementation and production proof are still absent. Builds on
[`LLD-META-L8-ADDENDUM.md`](LLD-META-L8-ADDENDUM.md) (A-H, 2026-09-12); does not reopen it.

Two independent gap sources, closed together because both surfaced from the same L8 review pass:
(1) authority omissions found in fleet's own docs regardless of any external comparison (I, J, the
central-standards CRUD contract), and (2) concrete mechanisms a research comparison against an
external technique (`GVS5H`, github.com/slee-persis/GVS5H — "ledger-based zero-shot
self-orchestration") showed fleet's design genuinely lacks (K-P). Neither source changes Fleet's
product scope; both close authority gaps the same way Addendum 1 did. This document is normative
alongside `LLD.md` once a blueprint links the relevant section, on the same terms as Addendum 1.

## I. Standard maintainer — owner: `approval`

§11's promotion workflow ends in "approved central proposal → externally authorized publication"
with no named approver — an omission independent of any external comparison. This is not a new
authority: it is one more effect class under the existing `approval` capability-grant machinery
that already gates PR/push/email publication.

```rust
struct StandardMaintainerGrant {
    grant_id: String,
    proposal_digest: String,
    reviewer_actor: String,        // a human identity, never a model identity
    decision: MaintainerDecision,
    reason: String,
    content_digest: String,
    expires_at: String,
}
enum MaintainerDecision { Approved, Rejected, NeedsRevision }
```

A candidate lesson cannot reach externally authorized publication without exactly one
`StandardMaintainerGrant{decision: Approved}` bound to its exact `content_digest`; a later edit to
the candidate invalidates the grant, matching the existing approval-binding rule in §1. A model may
draft the promotion write-up (§7's Teach-back format) but can never self-approve its own candidate.

Required tests: unapproved candidate cannot publish; approval for digest D does not cover digest
D'; a rejected candidate re-enters `candidate` status, not silent retry; a model-authored actor
field on a grant refuses.

## J. Pre-plan bearings gate — owner: `ready` (extends §7's readiness predicate)

Context (`fleet-context`) already feeds both Planner and Builder in §3, but the `ready(module)`
predicate never checks that the context manifest a plan was reviewed against is still current by
the time a lease is issued — a plan reviewed against stale bearings is not the claim it looks like.

```
ready(module) = ...  # existing clauses unchanged
  AND context_manifest_digest == last_reviewed_context_digest
  AND context_manifest_source_commit == current_base_commit
```

A source-commit or context-digest drift between plan review and lease issuance invalidates
readiness; the module returns to `PLANNING` for a fresh manifest and re-review — it never builds
against stale bearings silently.

Required tests: base commit advances between review and lease → readiness fails; identical digest
→ readiness holds; a plan with no recorded Context Compiler call cannot reach `ready` at all.

## K. Repair-retry feedback loop — owners: `verify`, `builder`

§8's retry policy ("verification repair budget two attempts") never defined the repair payload — a
repair attempt was a blind re-run, no different in kind from the first. `GateEvidence.failures`
(already `Vec<String>` in the current type) is upgraded to a structured payload, and Verify gains a
real failure-successor back to Builder instead of only Teach/Rollback.

```rust
struct FailingCase { gate_id: String, input_ref: String, expected_ref: String, actual_ref: String, diff_digest: String }
// GateEvidence.failures becomes Vec<FailingCase>, replacing Vec<String>
struct RepairAttempt { attempt_id: String, parent_lease_id: String, prior_gate_evidence_ref: String, repair_budget_remaining: u8 }
```

On a Verify failure within repair budget, `control` issues a `RepairAttempt` lease whose compiled
context (§10) mandatorily includes the prior `FailingCase` list ahead of optional retrieval — the
builder is told exactly what failed, not asked to guess again.

**Source of this closure:** this is the single highest-value mechanism identified in the GVS5H
comparison review (2026-09-13). GVS5H's own paper credits its mid-loop sample-test reinjection —
not its model-escalation ladder — for most of its score gain over blind retry.

Required tests: a repair lease's compiled context contains the exact prior failing case; a repair
attempt that reproduces the same failing case is a required check, not merely a plausible one;
repair-budget exhaustion still routes to Teach, never an unbounded loop.

## L. Stall/plateau detector — owner: `control` (extends §8's retry policy)

Confirmed absent repo-wide (zero hits for stall/plateau/reissue). §8 already stops blind retry at
the 3rd recurrence of the same failure signature and asks one diagnostic question — but nothing
between the 2nd and 3rd recurrence tries a genuinely different approach before escalating to a
human.

```rust
struct FailureSignature { gate_id: String, normalized_diff_digest: String, occurrence_count: u8 }
enum RepairEscalation { Retry, SwitchApproach { excluded_approach_digest: String }, AskHuman }
```

On the 2nd recurrence of the same `FailureSignature`, `control` issues `SwitchApproach` rather than
`Retry`: the next lease's compiled context marks the prior approach's plan digest excluded and
instructs the builder to select a genuinely different algorithm/data structure/reduction, not patch
the stuck one. Only a 3rd recurrence — of a *different* signature that also plateaus — reaches
`AskHuman`.

**Source of this closure:** the GVS5H comparison review found fleet's retry policy stops and asks a
human where GVS5H's own manager first tries to route around the problem itself before giving up.

Required tests: 2nd same-signature failure produces `SwitchApproach` with a distinct plan digest,
never `Retry`; a `SwitchApproach` lease's context excludes the prior plan digest; a 3rd
distinct-signature plateau reaches `AskHuman`, never a 4th silent retry.

## M. Truncated-attempt summarization — owner: `builder`

Confirmed absent (zero cutoff/truncation handling in `crates/builder`). A lease that exhausts its
token/time budget mid-attempt currently produces either a partial artifact or nothing usable, with
no record of what it established or ruled out.

```rust
struct TruncatedAttempt { lease_id: String, cutoff_reason: String, partial_digest: String, digest_summary_ref: Option<String> }
```

When a lease's provider adapter reports a length/budget cutoff, `builder` issues one bounded
follow-up call — same model, fresh context, told only to summarize what the partial attempt
established, ruled out, and left unfinished — and attaches the result to
`TruncatedAttempt.digest_summary_ref`. The next repair lease's compiled context includes this
summary ahead of optional retrieval, same priority as Addendum K's `FailingCase`. This never
re-attempts the truncated solution itself; that remains a separate, ordinary repair lease.

Required tests: a cutoff attempt always produces a `TruncatedAttempt` row, never a silent discard;
the summarization call itself cannot be truncated without a bounded retry of its own; the next
lease's context contains the summary digest.

## N. Approach ideation before plan commit — owner: `planner`

No fleet mechanism proposes multiple distinct candidate approaches before committing to one plan;
`planner` goes straight from requirements/evidence to a single `PlanVersion`.

```rust
struct ApproachCandidate { id: String, plan_intake_ref: String, summary: String, tradeoffs: String, risk_notes: String }
```

For workflow kinds `investigate`, `feature`, and `refactor` only (§6's table) — never
`small-change`, where planning is already skippable — `planner` first produces 2-3
`ApproachCandidate`s (prose only, no code, no plan yet) before `PlanReview` selects one to carry
into a full `PlanVersion`. Selection uses `PlanReview`'s existing independence/qualification bar
(§7); it is not a separate, weaker authority.

Required tests: a `small-change` workflow never produces `ApproachCandidate` rows; an
`investigate`/`feature`/`refactor` `PlanVersion` always traces back to exactly one selected
`ApproachCandidate`; the rejected alternatives are logged alongside the winner, not discarded.

## O. Failure-triggered tier escalation — owners: `router`, `route`

`route::score_and_select` picks the single minimum-cost candidate once; `router`'s `Tier` ordering
has no failure-triggered promotion path (confirmed in the GVS5H comparison review, 2026-09-13).

```rust
struct EscalationCandidate { route_decision_id: String, prior_tier: String, next_tier: String, prior_failing_case_ref: String }
```

When a `RepairAttempt` (Addendum K) at the current tier exhausts its repair budget without a
`SwitchApproach` (Addendum L) having already resolved it, `route` re-scores at the next tier up. The
compiled context for that escalated lease carries the exhausted tier's last candidate plus its
`FailingCase` as a "verify and redo" hint — never a fresh blind attempt at the new tier. Escalation
is one-directional, capped at the top of the committed `ORDER` table, and can never cross below the
task's computed risk-tier floor (§17).

Required tests: escalation fires only after repair budget AND switch-approach are both exhausted;
an escalated lease's context contains the prior tier's failing case; escalation never crosses below
the §17 risk floor; the escalated attempt's cost is attributed separately (§17), never folded into
the original tier's estimate.

## P. Terminal-outcome taxonomy — owner: `fleet-types` (extends §5's data model)

Confirmed zero hits repo-wide for `infra_exhausted`/`finish_reason`-equivalent vocabulary.
`WorkerObservation` and `GateEvidence` currently cannot distinguish "the model never produced a
clean response after all retries" from "the model responded and was simply wrong" — two cases that
call for different next steps.

```rust
enum AttemptOutcome { Ok, InfraExhausted, ProtocolFault, Truncated, EmptyStop }
```

Every `WorkerObservation` and `GateEvidence` carries an `AttemptOutcome`. `control`'s retry/repair
logic (§8, Addendum L) branches on it: `InfraExhausted` retries the same tier without incrementing
`FailureSignature.occurrence_count` (an infra failure is not evidence the approach is wrong);
`Truncated` routes to Addendum M before anything else.

Required tests: an infra-exhausted attempt does not increment `occurrence_count`; a truncated
attempt always produces Addendum M's `TruncatedAttempt` row; `Ok`-with-a-failing-gate is distinct
from every non-`Ok` outcome in every downstream branch.

## Central standards CRUD — owner: `knowledge` (concretizes §11's Git/MCP transport)

§11 says central manifests are pulled "by pinned revision/signature... Transport may be Git or
MCP" but names no concrete operation set. This gives it one, matching Addendum 1-F's style for
connector identity.

```rust
enum CentralOp { Pull, Propose, Promote, Retire }
struct CentralRequest { op: CentralOp, standard_id: Option<String>, candidate_ref: Option<String>, trust_root_id: String, expected_revision: Option<u64> }
struct CentralRecord { standard_id: String, revision: u64, content_digest: String, source: String, status: String, retired_at: Option<String> }
```

`Pull` reads by pinned revision/signature against the trust root (Addendum 1-A); `Propose` writes a
`candidate` lesson upstream, never a validated/promoted one; `Promote` requires exactly one
`StandardMaintainerGrant{Approved}` (Addendum I) bound to that candidate's digest; `Retire` writes a
tombstone `CentralRecord`, never a hard delete (matching §11's existing tombstone rule). Whether the
transport is Git or MCP is an adapter detail behind this same four-operation contract; `knowledge`
never speaks either transport directly — Git goes through `integrate`-owned mechanics, MCP goes
through `broker` (Addendum 1-B), same as every other effect in this document.

Required tests: `Promote` without a bound grant refuses; `Retire` never removes the row, only marks
it; a `Pull` against a revoked trust root refuses; Git-transport and MCP-transport paths produce
byte-identical `CentralRecord` rows for the same standard.

## Closure gate

Same terms as Addendum 1: this addendum changes review status only after affected blueprints link
these contracts, `EDGE-TYPES.md` names the new canonical payloads (`FailingCase`, `RepairAttempt`,
`FailureSignature`, `TruncatedAttempt`, `ApproachCandidate`, `EscalationCandidate`,
`AttemptOutcome`, `StandardMaintainerGrant`, `CentralRequest`/`CentralRecord`), and a
different-model review finds no unresolved P0/P1 contradiction. Until then, `LLD.md` plus Addendum
1 remain the recoverable baseline and implementation readiness remains unproven.
