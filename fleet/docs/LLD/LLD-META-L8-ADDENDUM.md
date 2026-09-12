# Fleet LLD Meta-L8 Addendum

Date: 2026-09-12  
Status: design closure draft; implementation and production proof are still absent.

This addendum closes four authority omissions found during independent meta-L8 review. It does
not expand Fleet's product scope. For the four areas below, this document is normative alongside
`LLD.md`; a blueprint must link the relevant section and must not invent a second contract.

## A. Configuration trust root — owner: `control`

There is no configuration node in the 32-node topology. `control` owns configuration resolution;
`store` persists the accepted snapshot and its provenance. Repository configuration can narrow an
installed policy or grant, never expand it.

```rust
struct ConfigTrustRoot {
    root_id: String,
    algorithm: String,
    public_key_ref: String,
    revision: u64,
    revoked: bool,
}
struct ConfigSnapshot {
    snapshot_digest: String,
    schema_version: u64,
    source_ref: String,
    content_digest: String,
    trust_root_id: String,
    signature_ref: Option<String>,
    policy_digest: String,
    loaded_at: String,
}
struct ConfigDecision {
    snapshot_digest: String,
    accepted: bool,
    reason: String,
    checked: u64,
    total: u64,
}
```

The loader uses a fixed installation safety floor and an operator-approved trust-root location.
It rejects missing, unsigned, wrong-root, revoked, unknown-key, invalid-schema, duplicate-key,
and non-canonical snapshots before exposing any policy, gate, capability, or effect setting. A
complete snapshot is parsed and verified off to the side, then atomically swapped; a failed reload
keeps the previous valid snapshot and writes a refusal receipt. Every downstream decision records
`snapshot_digest`, `content_digest`, `trust_root_id`, and the verification result. Signature and
canonical-serialization code must use an adopted maintained library with an exact lockfile entry;
Fleet must not implement cryptography or canonical encoding itself.

Required tests: unsigned config refuses; revoked root refuses; repository config cannot widen a
grant; a changed snapshot digest invalidates the old decision; failed reload preserves the previous
snapshot; the real binary reports the selected snapshot and a positive denominator.

## B. Skills and MCP capability mediation — owner: `broker`, issued by `control`

The worker receives a lease-scoped capability bundle over fd 3. It never receives a credential
value, broker socket, state path, or authority field. `broker` is the only MCP-speaking process.

```rust
struct CapabilityBundle {
    lease_id: String,
    bundle_digest: String,
    manifest_digest: String,
    tool_list_digest: String,
    capabilities: Vec<ToolCapability>,
    credential_handles: Vec<CredentialHandle>,
    expires_at: String,
    revocation_epoch: u64,
}
struct ToolCapability {
    server_id: String,
    tool_name: String,
    schema_digest: String,
    resource_scope: String,
    effect_class: String,
}
struct CredentialHandle {
    handle_id: String,
    audience: String,
    lease_id: String,
    expires_at: String,
    revocation_epoch: u64,
}
struct ToolRequest {
    lease_id: String,
    bundle_digest: String,
    sequence: u64,
    server_id: String,
    tool_name: String,
    args_digest: String,
}
struct ToolResult {
    request_digest: String,
    sequence: u64,
    status: String,
    result_ref: Option<String>,
    provider_metadata_ref: Option<String>,
}
```

For every request, `broker` verifies lease generation, expiry, revocation epoch, bundle digest,
tool-list digest, schema digest, sequence, resource scope, and effect grant before reserving and
persisting the call. It writes `PREPARED`, then `DISPATCHING`, then a sanitized result or
`UNKNOWN`; a timeout is never success. A server tool-list change invalidates the bundle and blocks
new calls until `control` recompiles and reauthorizes it. Credential handles are opaque references;
their values exist only in the parent-owned provider adapter and are revoked with the lease.

Required tests: forged tool name refuses; stale bundle refuses; sequence gap refuses; expired or
revoked handle refuses; tool-list change invalidates the bundle; worker frame containing a
credential/authority field refuses; crash after `DISPATCHING` yields `UNKNOWN`; the real fd-3
child path proves a valid request reaches the broker and no credential value reaches the child.

## C. Multi-repository saga — owners: `integrate` and `rollback`

`integrate` owns preparation and publication sequencing. `rollback` owns compensation. `broker`
executes each external repository operation, but never owns the saga state.

```rust
struct ChangeSet {
    id: String,
    run_id: String,
    repo_ids: Vec<String>,
    expected_base_heads: Vec<String>,
    interface_versions: Vec<String>,
    dependency_digest: String,
    operation_key: String,
    grant_id: String,
    state: SagaState,
    revision: u64,
}
enum SagaState { Preparing, Verified, Publishing, Partial, Complete, Compensating }
struct RepoOperation {
    changeset_id: String,
    repo_id: String,
    operation_key: String,
    base_head: String,
    candidate_digest: String,
    expected_resource_version: Option<String>,
    remote_ref: Option<String>,
    state: String,
    result_digest: Option<String>,
}
```

The store has one durable `changesets` row and one `repo_operations` row per repository, each
versioned by `revision`; `operation_key` is unique per repository effect. Locks are acquired in
sorted repository-ID order. `PREPARING -> VERIFIED` requires every repository's candidate,
compatibility test, base head, and rollout order to be recorded. `VERIFIED -> PUBLISHING` requires
one grant covering the complete ordered target set. A provider acknowledgement is insufficient
until remote ref/PR readback matches the operation key and expected version.

| Current | Event/guard | Next |
|---|---|---|
| `PREPARING` | all repos locally verified | `VERIFIED` |
| `PREPARING` | preparation failure before publication | `PARTIAL` only if any remote effect exists; otherwise retain failure receipt |
| `VERIFIED` | exact complete grant consumed | `PUBLISHING` |
| `PUBLISHING` | every operation read back successfully | `COMPLETE` |
| `PUBLISHING` | one or more operations uncertain/failed | `PARTIAL` |
| `PARTIAL` | compensation grant or explicit operator decision | `COMPENSATING` |
| `COMPENSATING` | every published operation read back at its prior ref | `COMPLETE` with compensation receipt |

Restart resumes from durable rows, never from in-memory loop position. It reuses the same
per-repository operation key; it does not issue a second publication while the prior result is
unknown. A partial saga cannot be reported as success, and Git rollback cannot claim to undo a
non-Git side effect.

Required tests: repository A succeeds and B fails; restart does not duplicate A; changed base head
refuses; missing repository grant refuses before publication; remote readback mismatch remains
`PARTIAL`; compensation requires explicit scope and produces a receipt; all state transitions are
replayed from the event log.

## D. Lifecycle and governance authority — owner: `control`

The existing `fleet-lifecycle` and `fleet-govern` crates are extraction candidates only. They are
not competing state authorities. `control` owns the reducer, governance predicates, revision CAS,
grant/quota admission, and persisted run-state projection.

The complete run-state vocabulary is:

`RECEIVED`, `CLASSIFYING`, `WAITING_INPUT`, `PLANNING`, `READY`, `RUNNING`, `VERIFYING`,
`INTEGRATING`, `WAITING_PUBLICATION`, `PAUSING`, `PAUSED`, `WAITING_QUOTA`,
`NEEDS_RECONCILIATION`, `COMPLETED`, `FAILED`, `CANCELLED`.

Every accepted event is a row in the durable event log with `run_id`, `revision`, `event_id`,
`schema_version`, payload digest, previous digest, actor, and reducer version. A transition is legal
only when a matching row exists below; all other pairs are typed refusals with receipts.

| From | Event/guard | To |
|---|---|---|
| none | receive valid request | `RECEIVED` |
| `RECEIVED` | classify admitted | `CLASSIFYING` |
| `CLASSIFYING` | material unknown exists | `WAITING_INPUT` |
| `CLASSIFYING` | no material unknown | `PLANNING` |
| `WAITING_INPUT` | answer revision matches | `PLANNING` |
| `PLANNING` | plan accepted | `READY` |
| `READY` | lease and grants admitted | `RUNNING` |
| `RUNNING` | worker evidence complete | `VERIFYING` |
| `VERIFYING` | integration admitted | `INTEGRATING` |
| `INTEGRATING` | local integration complete | `WAITING_PUBLICATION` or `COMPLETED` |
| `WAITING_PUBLICATION` | exact publication grant consumed | `COMPLETED` |
| any nonterminal state | pause barrier committed | `PAUSING` |
| `PAUSING` | children stopped/detached and leases revoked | `PAUSED` |
| `PAUSED` | heads, grants, policy, credentials, and digests revalidated | saved safe state |
| any eligible state | quota exhausted | `WAITING_QUOTA` |
| `WAITING_QUOTA` | eligible route and reservation available | `READY` or saved safe state |
| any effect-uncertain state | reconciliation required | `NEEDS_RECONCILIATION` |
| `NEEDS_RECONCILIATION` | authoritative readback resolves outcome | saved safe state or terminal state |
| any nonterminal state | explicit cancellation and descendant kill | `CANCELLED` |
| any nonterminal state | unrecoverable invariant failure | `FAILED` |

`COMPLETED`, `FAILED`, and `CANCELLED` are terminal. `PAUSED` resumes only to the persisted safe
state, never directly to an effect. `WAITING_QUOTA` cannot hot-loop. Pause, cancel, lease expiry,
unknown effect, and stale revision have precedence over ordinary completion events.

Required tests: one acceptance test for every legal row; generated illegal state/event pairs all
refuse; replay is deterministic; pause barrier prevents new leases; cancellation revokes the
capability bundle; unknown effect enters reconciliation; stale revision loses CAS; terminal states
reject ordinary events; real binary output includes the transition and positive `checked,total`.

## Required persistence additions

The compact schema in `LLD.md §5` must be expanded before implementation with durable rows for
`config_snapshots`, `capability_bundles`, `tool_calls`, `changesets`, `repo_operations`,
`usage_observations`, and `run_transition_events`. Each row needs an owner, schema version,
idempotency key where applicable, digest, revision, retention rule, and migration test. A blueprint
cannot use an in-memory map as a substitute for any row named here.

## E. Token economics and settlement — owners: `control`, `store`, `route`

Routing may estimate; only `control` and `store` may reserve or settle. Every attempt, including a
failed, retried, reviewed, researched, or tool-using attempt, has an attributable record.

```rust
struct UsageObservation {
    id: String,
    attempt_id: String,
    source_namespace: String,
    provider_request_id: Option<String>,
    observation_key: String,
    input_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    tool_units: Option<u64>,
    price_microunits: Option<u64>,
    observed_at: String,
    confidence: String,
    reconciliation_status: String,
    digest: String,
}
struct CostSettlement {
    reservation_id: String,
    attempt_id: String,
    reserved_units: u64,
    settled_units: u64,
    released_units: u64,
    held_units: u64,
    usage_observation_id: Option<String>,
    adjustment_id: Option<String>,
}
struct BillingAdjustment {
    id: String,
    source_namespace: String,
    provider_record_id: String,
    delta_units: i64,
    reason: String,
    digest: String,
}
```

`observation_key` is unique for `(source_namespace, provider_request_id)` when the provider gives
an ID, otherwise for `(attempt_id, sequence, digest)`. Settlement is a compare-and-swap and must
preserve the integer conservation invariant `reserved_units = settled_units + released_units +
held_units`. Missing usage is `held`, not zero. A late provider bill is one `BillingAdjustment`,
never a second copy of request usage. Cached, uncached, output, and tool units are added only when
the provider declares them disjoint. Price arithmetic uses checked wide integer multiplication and
ceiling division; no floating-point score, cost, or token field is permitted.

Required tests: failed attempt remains attributable; duplicate observation is idempotent; missing
usage holds the reservation; late correction changes an adjustment once; conservation holds across
every settlement state; overrun blocks new admission; account-level aggregate is not double-counted;
zero accepted tasks reports undefined cost-per-accepted rather than zero.

## F. Connector identity and native mapping — owner: `connectors`

```rust
struct ConnectorIdentity {
    source_namespace: String,
    provider: String,
    principal_ref: String,
    credential_ref: String,
    scopes: Vec<String>,
    config_digest: String,
}
struct NativeEventMapping {
    source_namespace: String,
    native_kind: String,
    delivery_id: String,
    object_version: String,
    external_actor: String,
    payload_digest: String,
}
```

GitHub uses an installation-scoped App credential, webhook signature verification, and
`X-GitHub-Delivery` as `delivery_id`; the adapter maps `sender.login` to `external_actor` and the
native issue, pull-request, or comment `updated_at` to `object_version`. The source namespace is
installation plus repository, never a user-wide token. Gmail uses OAuth with the minimum configured
scope (`gmail.readonly` for intake; `gmail.send` only for explicitly enabled notification), history
cursor verification, and an account/history/message composite delivery key; the adapter maps the
message sender to `external_actor` and Gmail `historyId` to `object_version`. Password IMAP and
personal GitHub bearer tokens are compatibility fixtures only, not compliant implementations.

Secrets remain opaque `credential_ref`s in a parent-owned credential port. Cursors advance only
after the corresponding `ConnectorEnvelope` is durably accepted by `ingest`. A bad signature, scope,
cursor, schema, or native mapping is a typed refusal; a 401/403 is not an empty page.

Required tests: invalid GitHub signature refuses; installation/repository scope mismatch refuses;
Gmail history expiry refuses without cursor advance; duplicate delivery with a different payload
digest becomes a conflict; credentials never appear in `NativeEvent`; one scripted event reaches
`ingest` with the exact canonical envelope fields.

## G. Resource profiles and owned cleanup — owners: `ready`, `store`, `control`

```rust
struct ResourceProfile {
    name: String,
    controller_idle_rss_bytes: u64,
    controller_active_p95_rss_bytes: u64,
    process_tree_rss_bytes: u64,
    cpu_millis_per_second: u64,
    max_cpu_heavy_jobs: u64,
    state_bytes: u64,
    worktree_bytes: u64,
    build_cache_bytes: u64,
    free_disk_floor_bytes: u64,
    sample_period_seconds: u64,
    sample_count: u64,
}
struct ResourceObservation { profile: String, metric: String, value: u64, checked: u64, total: u64 }
```

The default profile enforces the LLD targets N01–N06 and N17: controller idle 128 MiB, active P95
256 MiB, supervised tree 2 GiB, constrained tree 768 MiB/one lane, idle CPU 1% averaged over five
minutes, at most two CPU-heavy jobs, state 2 GiB, worktrees 4 GiB, build cache 4 GiB, and 2 GiB
free-disk floor. A reservation is rejected before lease creation when measured usage plus the new
reservation would cross a profile limit. Profiles may be stricter, never looser than safety
limits, unless an explicit larger compiler profile is recorded with its reason.

An owned temporary resource is one created under the run's declared state/worktree/artifact root
with the run ID in its ownership record. Cleanup is attempted after terminal failure/cancellation;
it must preserve pinned artifacts and foreign files, report `checked,total`, and finish within 60 s
when no reader holds a pin. Sampling is evidence, not prevention: an OS-specific limiter is required
before calling an RSS ceiling enforced.

Required tests: constrained profile refuses a second lane; disk reservation preserves the free floor;
RSS/CPU samples publish positive denominators; cancellation removes only owned temporary resources;
pinned artifact survives cleanup; a fast allocation spike is reported as an observation rather than
claimed prevented.

## H. Canonical payloads for previously unresolved edges

The following names are the canonical wire types in `EDGE-TYPES.md`. The owner blueprint must
re-export the type or a byte-for-byte compatible adapter; a similarly named local struct is not
acceptable.

```rust
struct InputEnvelope { source: String, delivery_id: String, schema_version: u64, actor: String, text: String, payload_digest: String, auth_metadata_ref: Option<String> }
struct ConnectorEnvelope { source: String, delivery_id: String, object_version: String, schema_version: u64, actor: String, payload_ref: String, payload_digest: String, auth_metadata_ref: String }
struct DurableEvent { event_id: String, run_id: String, source: String, schema_version: u64, payload_ref: String, payload_digest: String }
enum StoreCommand { AppendEvent { expected_revision: u64, event_ref: String }, Cas { key: String, expected_revision: u64, record_ref: String } }
struct DispatchRequest { run_id: String, intent_ref: String, snapshot_digest: String, requested_capabilities: Vec<String> }
enum WorkflowSelection {
    Clear { revision: u64, recipe_digest: String, evidence_ref: String },
    Probe { kinds: Vec<String>, revision: u64, recipe_digest: String, evidence_ref: String },
}
struct WorkflowRecipe { id: String, version: u64, nodes: Vec<String>, edges: Vec<String>, digest: String }
struct SkipPlanningSignal { run_id: String, plan_digest: String, reason: String, checked: u64, total: u64 }
struct PublicationRequest { run_id: String, changeset_id: Option<String>, destinations: Vec<String>, content_digest: String, acceptance_digest: String }
struct PostVerifyEvidence { integration_receipt_ref: String, gate_evidence_refs: Vec<String>, checked: u64, total: u64 }
struct PostVerifyFailure { integration_receipt_ref: String, reason: String, effect_state: String, checked: u64, total: u64 }
struct PublicationGrant { approval_id: String, changeset_id: Option<String>, action: String, destinations: Vec<String>, content_digest: String, expires_at: String }
struct RepairSignal { run_id: String, failed_node_id: String, reason: String, source_revision: u64 }
```

`RequirementInput` remains the canonical scan-to-probe input and `Vec<Question>` remains the
probe-to-questions output. `ScanDecision` is the local implementation name for this exact
`WorkflowSelection` wire enum; the scan blueprint must expose the alias/adapter explicitly.
Payload fields are bounded, digests are fixed strings, counts are integers, and unknown values are
explicit options/reasons. `EDGE-TYPES.md` must not use informal labels such as “request” or “state
change” as a substitute for these types.

## Closure gate

This addendum changes the review status only after all affected blueprints link these contracts,
`EDGE-TYPES.md` names the canonical payloads, `FR-TRACE.md` assigns FR11/FR12/FR29/FR43 and the
lifecycle observations, and a different-model review finds no unresolved P0/P1 contradiction.
Until then, the legacy blueprint set must remain recoverable and implementation readiness remains
unproven.
