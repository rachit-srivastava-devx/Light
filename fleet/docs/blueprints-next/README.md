# Fleet node blueprints

Fleet is a local CLI SDLC harness. It accepts a request, classifies it, asks only the questions
needed to resolve ambiguity, selects a typed workflow, prepares bounded context, leases isolated
work, reviews the candidate, runs independent deterministic evidence, integrates locally, and
stops before external publication unless an exact grant approves that effect.

The reason for this design is evidence provenance: model prose may propose intent, plans, code, or
lessons, but Fleet owns state transitions, permissions, budgets, scheduling, validation, merge,
rollback, and promotion. A green test or model completion is not proof by itself.

## Coverage

There are exactly 32 blueprint nodes, matching the component list in
`docs/LLD/lld-full-detail.architecture.json`. The canonical path is:

```text
user_cli/connectors -> ingest -> store/control -> intent -> route -> scan
  -> probes -> questions -> dag -> planner/context -> plan_review -> ready
  -> builder/next_plan -> review -> verify -> integrate -> post
  -> approval -> broker/notify
  -> rollback on failed post-merge verification
verify -> candidate -> offline -> knowledge/context
```

The arrows are not documentation decoration. Each arrow is an input/output contract that must be
visible in the crate API and in at least one integration test.

## Artifact chain

```text
LLD + meta-L8 addendum + source evidence
  -> one node BLUEPRINT.md
  -> one crate/package with ≤80-line files
  -> focused tests + real composition test
  -> mutation/property/hidden evidence as required
  -> independent different-model review
  -> src/ composition and real binary run
```

No node is `DONE` because its document sounds complete. The root review gate in
`REVIEW-GATE.md` and the node's own definition of done must both pass.

## Build waves

| Wave | Nodes | Why |
|---|---|---|
| 0 | `store`, `control`, `route`, `model-catalog` | durable state and pure authority predicates |
| 1 | `ingest`, `lifecycle` concerns inside `control`, `connectors`, `intent`, `scan`, `questions` | request/event normalization and routing inputs |
| 2 | `dag`, `knowledge`, `context`, `planner`, `plan-review`, `ready` | versioned work definition and admission |
| 3 | `builder`, `next-plan`, `review`, `verify` | isolated work and independent evidence |
| 4 | `integrate`, `candidate`, `offline`, `post`, `approval`, `broker`, `notify`, `rollback` | effects, learning, and reconciliation |
| 5 | `user-cli` and `src/` composition | only after node contracts and real binary paths exist |

The LLD's node tags still control quality: deterministic logic is independently testable; gated
model work requires an independent reviewer; ungated probes cannot write effects.

## Honest status

This directory is a proposed implementation specification. Package research, current source
reuse, and implementation proof are separate fields in each node. The absence of a live provider,
external connector, sandbox, or multi-day soak is a blocker to that claim, not an invitation to
replace it with a fake.

Next: read `NODE-MAP.md`, then open the blueprint for the exact node you are implementing.
