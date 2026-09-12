# LLD completeness audit

Date: 2026-09-12  
Scope: read-only comparison of `docs/LLD/LLD.md`, `docs/LLD/lld-full-detail.architecture.json`, current source seams, `docs/blueprints-next/README.md`, `NODE-MAP.md`, and all 32 node blueprints.

## What Fleet is actually building

Fleet is specified as a local CLI SDLC harness: normalize a request or authorized provider event; classify intent; resolve only material ambiguity; select a versioned workflow DAG; compile bounded, digest-bound context; lease isolated worker activity; independently review and verify an exact tree; integrate locally; then stop before external publication unless an exact scoped grant authorizes it (`docs/LLD/LLD.md:1-10`, `56-102`, `179-203`). The product rationale is evidence provenance and parent-owned authority: model prose is an observation/proposal, while Fleet owns state, permissions, budgets, validation, merge, rollback, and learning promotion (`docs/blueprints-next/README.md:1-18`).

The current repository is building the contract seams and composition scaffolding, not the complete product. All 32 blueprints exist and match the 32 JSON components (`docs/blueprints-next/README.md:20-34`, `NODE-MAP.md:1-47`), but every node is marked `partial` or `greenfield`; the map explicitly says package names are not implementation proof (`NODE-MAP.md:49-51`).

## Findings

### P0 — Missing lifecycle node and incomplete authority trace

**Verified.** The LLD responsibility table names `fleet-lifecycle` as the pure transition reducer and `fleet-govern` as admission/grants/quotas/retry authority (`docs/LLD/LLD.md:108-123`), but the 32-node architecture has neither a lifecycle nor govern component. The controller node is described as reducer plus scheduler, while the `control` blueprint says it is the sole authority for reducer transitions, leases, scheduling, cancellation, and dispatch. This is a responsibility merge with no explicit edge/state ownership contract. Current source confirms a reducer seam in `crates/control/src/reducer.rs` and a `TaskState` enum in `crates/control/src/lib.rs`, but that does not resolve whether lifecycle, governance, and controller are one authority or separate modules.

**Impact.** A plan cannot prove which component owns legal transitions, grant issuance, quota settlement, retry classification, or lease expiry. This is a safety and recovery ambiguity, not a naming issue.

**Required closure.** Add an explicit lifecycle/govern node, or formally declare them as subcomponents of `control` with a single authoritative state-machine/table, ownership of every transition/effect intent, and testable edges. Do not claim the current reducer seam closes this design gap.

### P0 — External effect state is not connected to the workflow state machine

**Verified.** The LLD requires `PREPARED -> DISPATCHING -> ACKED/UNKNOWN/...`, provider readback, idempotency, and reconciliation (`docs/LLD/LLD.md:145-177`, `§12–13`), and the broker blueprint specifies this state machine (`docs/blueprints-next/broker/BLUEPRINT.md:16-18`, `31-40`). The JSON has only `approval -> broker` labelled `PR draft`; it has no broker-to-provider acknowledgement/reconciliation edge, no broker-to-notify edge, and no state transition back into controller/run state. The broker blueprint itself calls the implementation greenfield (`broker/BLUEPRINT.md:9`). Current source has `crates/broker/src/effect.rs` and `reconcile.rs` seams, not a composed durable workflow path.

**Impact.** A crash after dispatch can be represented locally as `UNKNOWN`, but the LLD does not specify how that result changes the run/node state, blocks publication, wakes reconciliation, or produces the terminal receipt. Blind retry prevention is stated but not connected to completion semantics.

**Required closure.** Model provider acknowledgement, readback, reconciliation, conflict/unknown resolution, notification, and controller state projection as explicit edges and transitions. Include the exact durable event/outbox ordering and terminal outcomes.

### P1 — Blueprint and JSON topology contradict each other

**Verified.** `plan_review` declares `context -> plan_review` as an incoming edge (`docs/blueprints-next/plan_review/BLUEPRINT.md:7`), but that connection is absent from `lld-full-detail.architecture.json`; the JSON has `context -> planner`, `context -> builder`, and `context -> review` only. `notify` declares control/rollback state-change inputs (`notify/BLUEPRINT.md:7`), while the JSON only has `approval -> notify`. `broker` declares external acknowledgement/reconciliation and `broker -> notify` (`broker/BLUEPRINT.md:7-8`), neither represented in the JSON. `approval` declares an operator/user decision input (`approval/BLUEPRINT.md:7`), but the JSON has no user/approval-decision edge. `questions` says answers return as versioned plan events (`questions/BLUEPRINT.md:7-8`), while the JSON ends at `questions -> user_cli` and does not represent the answer path.

**Impact.** The README says every arrow must be visible in the crate API and an integration test (`docs/blueprints-next/README.md:22-27`), but the canonical topology and node contracts currently disagree. A 32/32 node count therefore overstates edge completeness.

**Required closure.** Reconcile the JSON first, then require each added edge to have payload, authority, failure semantics, and an integration test. Record any intentional projection-only edge explicitly instead of leaving it implicit.

### P1 — Persistence schema is materially underspecified relative to the runtime model

**Verified.** The LLD lists `repo_heads`, `gates`, `usage_observations`, `model_snapshots`, `learning_candidates`, `policy_versions`, `artifacts`, `pins`, and `memory` as stored data, but the displayed DDL defines none of those tables and says real migration DDL still needs foreign keys, checks, indexes, and migration tests (`docs/LLD/LLD.md:153-175`). `runs.plan_digest` is nullable while the runtime model treats plan/policy/context digests as authority bindings; `effects` lacks destination/content/resource-version columns despite those fields being required in `EffectIntent`; `leases` is keyed only by `node_id`, which does not express the stated multi-attempt/generation history; and `outbox` has no delivery kind or terminal/reconciliation fields.

**Impact.** Restore, audit, retention, CAS, artifact pinning, usage provenance, policy activation, and effect reconciliation cannot be implemented from the proposed schema without inventing authority fields.

**Current evidence.** `crates/store/src/schema.rs` has migration batches for a smaller set of tables (`events`, `nodes`, `inbox`, `outbox`, `receipts`, `leases`, `blobs`), so the repository does not currently prove the full LLD persistence model.

**Required closure.** Publish authoritative migrations/schema contracts for every persisted object, including constraints, foreign keys, indexes, uniqueness/idempotency rules, retention/pin ownership, and migration/restore behavior. Keep absent values absent; do not infer them from nullable columns.

### P1 — State-machine transitions and recovery triggers are not complete enough to implement safely

**Verified.** The LLD gives readiness as a predicate and describes pause, cancellation, lease fencing, restart, external unknowns, stale plans, and rollback in prose (`docs/LLD/LLD.md:211-225`, `227-249`, `361-416`), but it does not provide a complete transition matrix for run, node, lease, plan version, grant, effect, outbox, candidate, and notification states. There is no explicit event-to-state mapping for provider timeout, worker protocol gap, controller crash before/after commit, disk-full receipt failure, changed HEAD, expired grant, or pause during external I/O.

**Impact.** The implementation can satisfy isolated predicates while disagreeing on whether work is retryable, stale, compensating, terminal, or awaiting a human. Recovery correctness is therefore unverified by design.

**Current evidence.** `crates/control/src/reducer.rs`, `crates/broker/src/effect.rs`, and `crates/broker/src/reconcile.rs` show partial state-machine seams; `src/tests/pipeline_resumes_after_crash.rs` explicitly describes Restate as deferred. These are not a complete composed recovery proof.

**Required closure.** Add state tables with legal transitions, event causes, durable-before-side-effect boundaries, retry/attempt limits, recovery owner, and receipt requirements for every failure class.

### P1 — Trust boundaries do not define connector ingress, credential storage, or provider identity strongly enough

**Verified.** The LLD names four trust zones and requires brokered MCP/provider transport, scoped credentials, token audience binding, and no token passthrough (`docs/LLD/LLD.md:100-102`, `330-359`). However, `connectors` is only “GitHub / Gmail” in the topology; the connector contract does not specify webhook authenticity, polling cursor ownership, provider account/tenant identity, replay window, secret storage boundary, or how raw payloads become immutable evidence. The connector blueprint is partial, and `crates/connectors/src/gmail.rs` contains a real-integration placeholder comment.

**Impact.** “Authorized event” is not sufficient to establish who authorized it, whether it was replayed, or whether untrusted issue/email content can influence effect-bearing intent.

**Required closure.** Define per-provider authentication/verification, credential custody, delivery identity, replay/ordering rules, payload size/redaction, and the exact ingest envelope. Mark live provider compatibility unverified until real authenticated probes pass.

### P2 — Operability and external-provider assumptions are targets, not product guarantees

**Verified.** The LLD gives measurable budgets and a 72-hour soak requirement (`docs/LLD/LLD.md:29-54`), and explicitly says budgets are proposed unless measured (`LLD.md:1-4`). It also states ACP/native adapters, MCP servers, installed CLIs, keyless login, provider quotas, sandbox mediation, and external connectors require exact-version or live smoke proof (`LLD.md:245-280`, `330-359`, `§22`). The blueprints consistently record partial/greenfield status; notably `model_catalog` says live catalog is incomplete, `probe_research` is greenfield, `notify` lacks a complete durable outbox, and `broker` lacks a complete external-effect broker (`NODE-MAP.md:15-47`).

**Impact.** The architecture is honest as a proposal, but a reader could mistake the detailed budgets and provider-adoption prose for availability, performance, or security evidence.

**Required closure.** Keep every target labelled proposed until the corresponding real-binary, real-child, authenticated-provider, fault-injection, resource, and soak receipts exist. Publish provider capability matrices with endpoint-reported identity, quota/error semantics, retry/idempotency behavior, and unsupported paths.

## Overall disposition

The 32 blueprint documents provide broad node coverage and generally preserve the intended local, parent-authoritative product boundary. Completeness fails at the edge/state/persistence level: the lifecycle/govern authority is not represented as a node, several blueprint edges are absent from the topology, external effect reconciliation is disconnected, and the persistence/state contracts are not sufficiently authoritative to implement recovery without invention. The design remains **proposed and unverified**, consistent with its own status line; no current crate or unit seam should be interpreted as production proof.
