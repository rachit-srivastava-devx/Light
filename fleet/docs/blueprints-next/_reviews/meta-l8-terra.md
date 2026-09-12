# Meta-L8 architecture readiness review — Terra

**Verdict: FAIL.** The LLD has a strong local-authority thesis, bounded budgets, honest proposal status, and a 32-component graph. It is **not implementation-complete**: three authority domains have no executable contract/state model, so an implementer would have to invent security and recovery behavior. Labels: **verified** = directly inspected in this checkout; **unverified** = documented/proposed or provider/runtime proof absent.

## P0 — blocks implementation

### P0-1: Configuration is an unrooted authority boundary — **verified**

Evidence: the LLD makes repository configuration part of the authority chain and says branch edits cannot expand grants ([LLD §12, lines 330-342](../../LLD/LLD.md#L330)), but defines neither a typed config schema, trusted-baseline locator/signature, bootstrap/update protocol, nor provenance record. The requirements trace marks FR11 unowned ([FR trace lines 22, 67-71](../FR-TRACE.md#L22)). This is not a blueprint-only omission: config selects adapters, workflows, gates, MCP servers, and policy. Without a root of trust, the controller cannot decide whether a given `agents.toml` or `policy/` is trusted.

Required change: add LLD §12.1 and an ADR defining `TrustedConfigSnapshot`, source precedence, first-install bootstrap, signature/pin verification, atomic reload/revocation, and the receipt fields `{source_digest, source_ref, trust_root, resolved_value, override_chain}`. Add a `config` node or make `control` the explicit owner; add a mandatory startup/refusal trace for absent, invalid, stale, and branch-modified policy.

### P0-2: Lease-scoped skills/MCP mediation has no enforceable end-to-end contract — **verified**

Evidence: the LLD correctly requires the broker to authenticate the originating lease and persist PREPARED/DISPATCHING before an MCP call ([LLD §12, lines 336-340](../../LLD/LLD.md#L336)), while worker frames are only generic `{version, sequence, kind, payload}` ([LLD §5, line 151](../../LLD/LLD.md#L151)). There is no versioned `ToolRequest`/`ToolResult`, service/tool manifest digest, credential-handle lifetime, tool-list snapshot binding, or cancellation/revocation rule. FR12/FR29 explicitly remain unowned ([FR trace lines 23, 40, 73-79](../FR-TRACE.md#L23)). Therefore fd 3 does not by itself prove a child received only lease-selected capabilities.

Required change: define those two frame schemas and a `CapabilityBundle {lease_generation, manifest_digest, allowed_tools[], credential_handles[], expiry}` contract in the LLD and architecture JSON; specify broker checks, result-size/redaction limits, server-version invalidation, and the terminal/reconciliation states. Add a `worker-context`/`mcp-broker` node with real-child tests proving deny-after-revoke, changed-tool-list rejection, and no alternate endpoint.

### P0-3: Multi-repository saga is named but not durable or owned — **verified**

Evidence: LLD §15 lists six saga states and says PARTIAL is not success ([LLD lines 393-397](../../LLD/LLD.md#L393)), but the runtime model/compact schema has no `ChangeSet`, per-repository publication operation, transition/fencing, compensation authorization, or remote reconciliation record ([LLD lines 131-177](../../LLD/LLD.md#L131)). FR43 is unowned and confirms neither `integrate` nor `rollback` enumerates the states ([FR trace lines 54, 108-114](../FR-TRACE.md#L54)). A crash between remote A and B has no defined durable decision authority.

Required change: add a saga coordinator (or explicitly assign it) and a versioned persistent model: `ChangeSet`, `RepoOperation`, operation idempotency key, remote identity/readback evidence, state transition table, compensation grant binding, and terminal operator-resolution path. Add crash points before/after each remote call and prove no second publication or unauthorized compensation after restart.

## P1 — must close before the affected increment

### P1-1: Run lifecycle is not a testable transition contract — **verified**

Evidence: LLD §16 names 15 states and says illegal transitions refuse ([LLD lines 399-405](../../LLD/LLD.md#L399)); it does not specify legal `(from,event,to)` transitions, transition-side effects, per-node/run interaction, or precedence between pause, cancel, lease expiry, and unknown external effect. FR42 identifies the same absence ([FR trace lines 53, 101-106](../FR-TRACE.md#L53)).

Required change: add one authoritative transition table with guard, durable event, receipt, and idempotency/replay result for every row; enumerate its denominator and use it to generate reducer/property/crash tests. `control` must own it.

### P1-2: Economic invariants lack a durable, unique settlement model — **verified**

Evidence: §17 gives a sound reservation narrative, including unknown usage and CAS settlement ([LLD lines 417-425](../../LLD/LLD.md#L417)), but §5 only says `usage_observations` are “also” stored ([LLD line 175](../../LLD/LLD.md#L175)); no record schema, uniqueness key, reservation allocation policy, late correction handling, or provider aggregate allocation rule exists. FR21 calls the control path partial ([FR trace lines 32, 87-91](../FR-TRACE.md#L32)).

Required change: define `UsageObservation`, `ReservationSettlement`, `BillingAdjustment`, their unique source keys and transition/rounding/overrun rules. Specify whether a provider aggregate may only be recorded unallocated, and add restart/duplicate/late-stream tests with integer conservation: `reserved = settled + released + held`.

### P1-3: Connector design cannot yet meet its stated auth and ingestion guarantees — **verified**

Evidence: §16 admits GitHub/Gmail have no concrete auth or native-to-envelope mapping and calls this a prerequisite ([LLD lines 407-413](../../LLD/LLD.md#L407)); the component graph nevertheless treats `connectors` as an external ingress node. Generic at-least-once prose does not choose token storage, scope, cursor, webhook verification, deletion/out-of-order semantics, or dead-letter operator workflow.

Required change: before increment E, add connector-specific subcontracts and threat/effect matrices: auth principal, secret-handle location, verification algorithm, exact dedup/cursor key, pagination ordering, retry/dead-letter/replay controls, and revocation behavior. Keep provider proof explicitly unverified until a real authorized smoke.

## P2 — correct before claiming a mature implementation plan

### P2-1: Resource SLOs are budgets, not enforceable admission policy — **verified**

Evidence: N01-N18 call metrics “proposed targets” and say RSS/CPU sampling cannot prevent a spike ([LLD §2, lines 29-54](../../LLD/LLD.md#L29)); scheduling says use historical P95 plus margin but supplies neither calibration cohort, margin, nor handling for missing measurements ([LLD lines 227-249](../../LLD/LLD.md#L227)). FR28 has no owner ([FR trace lines 39, 93-99](../FR-TRACE.md#L39)).

Required change: specify profile schema, measurement harness/machine metadata, warm-up/repetition/percentile calculation, conservative first-run reservation, and the exact enforcement/degradation action for each budget. Assign the profile and cleanup denominator to `control`/`store`, not `ready` alone.

### P2-2: Package adoption record contains a live contradiction — **verified**

Evidence: `PACKAGE-DECISIONS.md` rejects `petgraph` and chooses an adjacency-list implementation ([lines 28-29](../_research/PACKAGE-DECISIONS.md#L28)), while the current `crates/route/Cargo.toml` still declares optional `petgraph = 0.6` and the route blueprint repeats it. The research rule correctly says a no-smoke dependency is only a claim ([lines 57-66](../_research/PACKAGE-DECISIONS.md#L57)). This is not a production bug, but it violates the declared adopt-first decision process.

Required change: select one dependency decision, record its exact locked version/license/smoke, remove the other from the LLD/blueprint/manifests, and re-run dependency evidence. No current provider, sandbox, mutation, remote-CI, or production proof may be inferred from the research reports; their own evidence table leaves several such checks unverified.

## Evidence boundary

- **Verified locally:** 32 components exist in the architecture JSON; 32 blueprint directories and their declared workspace manifests/source were inspected; the workspace is dirty; `cargo metadata --no-deps` resolves the manifests.
- **Unverified:** real compatible managed adapter, fd-3 mediation against a real provider, MCP auth, connector auth, sandbox escape resistance, resource SLOs, 72-hour soak, mutation score, remote CI, and production reliability. Research itself records that local workspace tests were not completed and external/tool proof remains incomplete ([quality evidence lines 123-152](../_research/quality-evidence.md#L123)).

**FAIL** — do not start authority-foundation implementation until P0-1 through P0-3 are resolved by LLD/ADR and owned blueprint contracts. Next: resolve the configuration trust-root ADR first; it constrains every other authority decision.
