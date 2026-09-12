# FR-TRACE.md
<!-- Maps FR1–FR46 functional requirements to their owning blueprint nodes. -->
<!-- REVIEW-GATE.md:12 requires this mapping to exist. -->

Source: LLD.md §24 "Functional requirements trace: 46 / 46 addressed in design".
The 2026-09-11 independent review found several ownership omissions. The 2026-09-12
meta-L8 addendum assigns design ownership for the four P0 omissions and records the exact
observations still required. Assignment is not implementation proof; rows remain open until
their evidence is captured.

| FR | Summary | Owning node(s) | Required observation |
|----|---------|---------------|---------------------|
| FR1 | General-purpose intent — same entrypoint for answer, review, diagnose and change | intent, route | Same entrypoint selects answer, review, diagnose and change fixtures |
| FR2 | Correct handling path — typed route with evidence; unknown intent cannot mutate | route, control | Typed route with evidence; unknown intent cannot mutate |
| FR3 | Conditional parallel ambiguity probes — no probes for clear fixture | scan, probe_* | No probes for clear fixture; relevant probes overlap for ambiguous fixture |
| FR4 | User clarifications — answer event resolves blocking field and revises plan | questions, plan_review | Answer event resolves exact blocking field and revises plan |
| FR5 | Parallel modules/tasks/blueprints and stronger review | planner, plan_review, dag | Independent plans overlap; reviewer qualification and digest verified |
| FR6 | Walkthrough and feedback — user feedback invalidates affected reviewed plan only | plan_review, user_cli | User feedback invalidates affected reviewed plan only |
| FR7 | Build ready module while planning next — immutable contract N builds while N+1 plans | ready, next_plan, builder | Immutable contract N builds while N+1 plans within budget |
| FR8 | Teach during implementation — evidence-backed milestone explanation | builder, notify | Evidence-backed milestone explanation before terminal completion |
| FR9 | Central standards/dependencies — pinned standard and cross-repo contract in manifest | context, knowledge | Pinned standard and cross-repo contract appear in manifest and gate |
| FR10 | Convention files and templates — correct nested scope and template hash | context, control | Correct nested scope and template hash; branch cannot self-authorize |
| FR11 | Repo `.fleet/` control — strict config validation and visible value provenance | control, store | `ConfigSnapshot`/`ConfigDecision` records root, content digest, signature/revocation result, and positive checked/total before policy/capability use; runtime proof pending |
| FR12 | Inject skills/MCP/standards — real child receives only lease-selected content/services | control, broker, builder | Lease-scoped `CapabilityBundle` over fd 3; no credential value or authority path in worker; forged/stale/expired bundle tests and real-child proof pending |
| FR13 | Harness supplies needed tools — missing dependency is environment fault with concrete remedy | control, builder | Missing dependency is environment fault with concrete remedy |
| FR14 | Context/memory at scale — bounded working set; exact mandatory retention; explicit omissions | context, store, knowledge | Bounded working set; exact mandatory retention; explicit omissions |
| FR15 | Installed CLI keyless connection — real native-login probe per compatible executable | route, model_catalog | Real native-login probe per compatible executable; unsupported honest |
| FR16 | Fan-out, capability assignment and tracking — disjoint leases, per-type outcomes | dag, ready, control | Disjoint leases, per-type outcomes and budget decision receipt |
| FR17 | Review returned code — independent review plus exact-tree deterministic gates | review, verify | Independent review plus exact-tree deterministic gates |
| FR18 | Worktree/branch sanity — collision and stale HEAD refused | integrate, builder | Collision and stale HEAD refused; unrelated dirty work preserved |
| FR19 | Detailed PR teaching and feedback — draft explains actual diff and evidence | plan_review, broker, notify | Draft explains actual diff and evidence; edit invalidates stale approval |
| FR20 | Days of work and quota failover — durable wait/restart with eligible provider failover | route, model_catalog, control | `route`/`model_catalog` publish only qualified alternatives; `control` owns the durable wait/restart fence and retry budget. Runtime proof pending. |
| FR21 | Tokenomics and cost — failed-attempt costs included; unknown billed data remains unknown | control, store, route | Addendum §E defines observations, settlement, late corrections, and integer conservation. Runtime/provider billing proof pending. |
| FR22 | Improve from real iterations — reproduced lesson reduces recurrence on held-out tasks | knowledge, offline, candidate | Reproduced lesson reduces recurrence on held-out applicable tasks |
| FR23 | DB retention/compression budgets — DB/WAL/blobs/logs measured; pressure preserves pinned state | store | DB/WAL/blobs/logs measured; pressure preserves pinned state |
| FR24 | GitHub/Gmail triggers — authorized event produces exactly one logical request | connectors, ingest | Authorized event produces exactly one logical request under duplicate delivery |
| FR25 | Extensible event types — new namespaced schema/handler admitted without changing core reducer | ingest, control | New namespaced schema/handler admitted without changing core reducer semantics |
| FR26 | Future flexibility — versioned adapter/event/recipe conformance | ingest, route, dag | Versioned adapter/event/recipe conformance and explicit unknown versions |
| FR27 | Current harness research — primary source snapshots and adoption gaps recorded | (LLD §22 only, no blueprint node) | Primary source snapshots and adoption gaps recorded |
| FR28 | Small local resource footprint/cleanup — RSS/CPU/disk profile and owned cleanup published | ready, store, control | Addendum §G defines the N01–N06/N17 profile, reservation boundary, owned-resource definition, cleanup denominator, and preservation rules. Runtime sampling/limiter proof pending. |
| FR29 | Skills/MCP/slash capabilities — selected capability effects declared; missing support not silently faked | control, broker, model_catalog | Deterministic applicability plus capability/tool-list digests; unsupported capability is explicit; runtime mediation and provider proof pending |
| FR30 | Visual system graph — explicit role nodes, understandable flow, validated artifact | (LLD §3 + HTML artifact, no blueprint node) | Explicit role nodes, understandable flow, validated artifact |
| FR31 | General pipeline fan-out — independent research/planning/build/review jobs share bounded mechanism | dag, ready | Independent research/planning/build/review jobs share bounded mechanism |
| FR32 | Choose agent and effort — capability-qualified route and enforced/unsupported effort fields | route, model_catalog | Capability-qualified route and enforced/unsupported effort fields |
| FR33 | Performance by task type — distinct per-type cohorts with evidence counts and exclusions | offline, knowledge | Distinct per-type cohorts with evidence counts and exclusions |
| FR34 | Multiple workflows — different intents exercise different DAGs | dag, route | Different intents exercise different DAGs |
| FR35 | Optimize tokens/cost/quality — paired candidate experiment shows bounded trade-off at quality floor | offline, route | Paired candidate experiment shows bounded trade-off at quality floor |
| FR36 | Evolve routing/prompts/config — candidate cannot activate itself; bounded approved activation rolls back | offline, knowledge, control | Candidate cannot activate itself; bounded approved activation rolls back |
| FR37 | Skip unnecessary planning — low-risk clear task skips; one-line high-risk task does not | scan, dag, ready | Low-risk clear task skips; one-line high-risk task does not |
| FR38 | Speed vs quality — profile changes effort/review without weakening mandatory gates | route, verify | Profile changes effort/review without weakening mandatory gates |
| FR39 | Central learning push/pull — scoped, pinned import; publication respects external grant | knowledge, offline | Scoped, pinned import; publication respects external grant |
| FR40 | Automatic failed-merge rollback — guarded private-ref restore; published branch uses authorized compensation | rollback, integrate | Guarded private-ref restore; published branch uses authorized compensation |
| FR41 | Explain route/model/gate — snapshot-backed reason and denominator visible | route, notify | Snapshot-backed reason and denominator visible |
| FR42 | User pause/resume — no new lease after pause acknowledgement; resume revalidates state | control | Sixteen LLD run states and legal reducer rows are in Addendum §D; every legal row, illegal pair, pause barrier, and resume revalidation still needs runtime evidence |
| FR43 | One logical multi-repo change — partial publication and compensation are explicit, not falsely atomic | integrate, rollback, approval, broker, store | Durable `ChangeSet`/`RepoOperation` saga with six states, operation keys, remote readback, and explicit compensation; partial-publication/restart proof pending |
| FR44 | Outside-terminal notification — redacted deduplicated meaningful event to authorized destination | notify, approval | Redacted deduplicated meaningful event to authorized destination |
| FR45 | Block secret commit — strict mediated-edit child cannot create Git stores; broker commits only scanned tree | broker, verify | Strict mediated-edit child cannot create Git stores; broker commits only scanned tree; raw-shell tier explicitly unsupported |
| FR46 | Adopt new models without code table edit — new runtime catalog candidate enters probation | model_catalog, route | New runtime catalog candidate enters probation, then qualified activation |

---

## Open FRs requiring implementation evidence or further design

The following FRs were identified in the 2026-09-11 independent review as either fully unowned
or partially covered. The P0 design ownership is now assigned by
`docs/LLD/LLD-META-L8-ADDENDUM.md`; none is marked implementation-complete here.

### FR11 — Repo `.fleet/` config validation
**Design owner:** `control`, persisted by `store`. **Gap remaining:** no runtime trust-root,
signature/revocation, atomic reload, or provenance proof. **Action:** implement and test Addendum
§A; do not add a node unless this ownership proves insufficient.

### FR12 / FR29 — Skills/MCP/slash injection
**Design owner:** `control` issues and `broker` enforces; `builder` supplies the real child.
**Gap remaining:** no runtime ToolRequest/ToolResult, credential-handle, revocation, or real
fd-3 mediation proof. **Action:** implement and test Addendum §B.

### FR20 — Quota failover (partial)
**Design owner:** `route`/`model_catalog` publish eligible alternatives; `control` owns the
durable wait/restart fence and retry budget. **Gap remaining:** no runtime proof of restart,
reservation release, or hot-loop prevention. **Action:** implement the failover contract and
capture a positive-denominator restart fixture.

### FR21 — Tokenomics / failed-attempt cost (partial)
**Design owner:** `control`/`store` settle and persist; `route` estimates. **Gap remaining:** no
runtime/provider billing proof for failed-attempt attribution, duplicate observations, late
corrections, or held unknown usage. **Action:** implement and test Addendum §E.

### FR28 — Resource footprint (partial)
**Design owner:** `ready` admits against the profile; `control` reserves; `store` persists
observations and cleanup receipts. **Gap remaining:** no runtime sampling, OS limiter, or cleanup
proof. **Action:** implement and test Addendum §G with positive denominators and owned-resource
fixtures.

### FR42 — Pause/resume 16 states (partial)
**Design owner:** `control`. **Gap remaining:** runtime reducer, replay, pause-barrier, and
resume-revalidation evidence is absent. The addendum enumerates the sixteen LLD run states and
legal transitions; a reviewer must verify each transition case against a named test fixture.

### FR43 — Multi-repo saga
**Design owner:** `integrate` prepares/sequences, `rollback` compensates, `approval` scopes, and
`broker` executes individual effects; `store` persists the saga. **Gap remaining:** no runtime
ChangeSet/RepoOperation rows, restart fence, readback, or compensation proof. **Action:** implement
and test Addendum §C; do not add a saga node unless this ownership proves insufficient.
