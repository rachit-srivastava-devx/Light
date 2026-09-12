# EDGE-TYPES.md
<!-- Canonical type registry — all 43 LLD edges. -->
<!-- Every blueprint that mentions a payload type or edge test MUST use the spelling from this table. -->
<!-- If you find a conflict, fix the blueprint, not this table. -->
<!-- Owning-crate = the crate's `name` field as used in `cargo test -p <name>`. For edit-in-place nodes (e.g. scan → fleet-scan), the existing crate name is used. -->

| From | To | Payload type | Owning crate | Canonical test id |
|------|----|-------------|-------------|------------------|
| user_cli | ingest | InputEnvelope | fleet-types | ingest::tests::user_request_normalizes |
| connectors | ingest | ConnectorEnvelope | fleet-types | ingest::tests::connector_event_deduplicates |
| ingest | store | DurableEvent | ingest | store::tests::append_event_idempotent |
| ingest | control | NormalizedEvent | ingest | control::tests::normalized_event_transitions_state |
| control | store | StoreCommand | control | store::tests::cas_revision_increment |
| control | intent | DispatchRequest | control | intent::tests::dispatch_request_emits_intent_spec |
| intent | route | IntentSpec | intent | route::tests::intent_spec_admitted_on_capability_match |
| model_catalog | route | CatalogSnapshot | model-catalog | route::tests::catalog_snapshot_filters_unavailable_candidates |
| route | scan | IntentSpec | intent | scan::tests::admitted_intent_spec_populates_scan_input |
| scan | probe_business | RequirementInput (LLD: AmbiguityRequest; see fleet-scan::input) | fleet-scan | probe_business::tests::ambiguity_request_produces_question |
| scan | probe_tech | RequirementInput (LLD: AmbiguityRequest; see fleet-scan::input) | fleet-scan | probe_tech::tests::ambiguity_request_produces_question |
| scan | probe_learn | RequirementInput (LLD: AmbiguityRequest; see fleet-scan::input) | fleet-scan | probe_learn::tests::ambiguity_request_retrieves_scoped_lessons |
| scan | probe_research | RequirementInput (LLD: AmbiguityRequest; see fleet-scan::input) | fleet-scan | probe_research::tests::ambiguity_request_produces_research_question |
| probe_business | questions | Vec\<Question\> | fleet-scan | questions::tests::business_probe_output_merges |
| probe_tech | questions | Vec\<Question\> | fleet-scan | questions::tests::tech_probe_output_merges |
| probe_learn | questions | Vec\<Question\> | fleet-scan | questions::tests::learn_probe_output_merges |
| probe_research | questions | Vec\<Question\> | fleet-scan | questions::tests::research_probe_output_merges |
| questions | user_cli | QuestionSet | questions | questions::tests::output_capped_at_three |
| scan | dag | WorkflowSelection | fleet-scan | dag::tests::workflow_selection_produces_recipe |
| dag | planner | WorkflowRecipe | dag | planner::tests::recipe_drives_module_plan |
| dag | ready | SkipPlanningSignal | dag | ready::tests::low_risk_scope_skips_planning |
| plan_review | user_cli | PlanWalkthrough | plan-review | plan_review::tests::walkthrough_emitted_before_ready |
| approval | user_cli | PublicationRequest | approval | approval::tests::publication_request_awaits_user_grant |
| planner | plan_review | PlanProposal | planner | plan_review::tests::proposal_reviewed_by_independent_model |
| plan_review | ready | ReviewedPlanDigest | plan-review | ready::tests::reviewed_digest_enables_builder_lease |
| ready | builder | Lease | ready | builder::tests::lease_scopes_private_worktree |
| ready | next_plan | NextPlanSignal | ready | next_plan::tests::n1_plan_starts_while_n_builds |
| builder | review | CandidateObservation | builder | review::tests::candidate_observation_triggers_code_review |
| review | verify | ReviewedCandidate | review | verify::tests::reviewed_candidate_runs_deterministic_gates |
| verify | integrate | GateEvidence | verify | integrate::tests::gate_evidence_passes_integration |
| integrate | post | IntegrationReceipt | integrate | post::tests::integration_receipt_triggers_post_verify |
| post | approval | PostVerifyEvidence | post | approval::tests::post_verify_pass_requests_publication |
| post | rollback | PostVerifyFailure | post | rollback::tests::post_verify_failure_triggers_rollback |
| approval | broker | PublicationGrant | approval | broker::tests::publication_grant_dispatches_pr_draft |
| approval | notify | StateChangeEvent | approval | notify::tests::state_change_emits_redacted_notification |
| rollback | dag | RepairSignal | rollback | dag::tests::repair_signal_requeues_failed_module |
| verify | candidate | GateEvidence | verify | candidate::tests::gate_evidence_creates_lesson_candidate |
| candidate | offline | CandidateLesson | candidate | offline::tests::lesson_candidate_evaluated_in_paired_trial |
| offline | knowledge | ValidatedLesson | offline | knowledge::tests::validated_lesson_promoted_to_store |
| knowledge | context | SourceManifest | knowledge | context::tests::knowledge_manifest_bounds_retrieval |
| context | planner | ContextManifest | context | planner::tests::context_manifest_scopes_plan_generation |
| context | builder | ContextManifest | context | builder::tests::context_manifest_scopes_build_context |
| context | review | ContextManifest | context | review::tests::context_manifest_scopes_review_evidence |

**Row count:** 43 edges.

---

## Type name discrepancies (LLD vs code)

These cases are where the LLD's payload name differs from the actual Rust type in the codebase.
Blueprints must align to the **code** spelling; the LLD names are recorded here for traceability.

### 1. `AmbiguityRequest` (LLD) = `RequirementInput` (code)

- **Edges affected:** scan→probe_business, scan→probe_tech, scan→probe_learn, scan→probe_research (4 edges)
- **Code location:** `crates/fleet-scan/src/input.rs`
- **Action:** All four probe blueprints must reference `RequirementInput`, not `AmbiguityRequest`.
  Fix the probe blueprint files — do not edit this table.

### 2. `CandidateLesson` (canonical)

- **Edges affected:** candidate→offline
- **Decision:** `CandidateLesson` is the canonical wire spelling because it is defined by
  `candidate/BLUEPRINT.md §5`; `offline` consumes that exact type. The former inverted spelling
  was a registry error.
- **Action:** Keep `CandidateLesson` in both blueprints and in every new edge test.

### 3. `SourceManifest` (canonical)

- **Edges affected:** knowledge→context
- **Decision:** `SourceManifest` is the canonical wire spelling because it is defined by
  `knowledge/BLUEPRINT.md §5`; the former manifest wording was a registry error.
- **Action:** Keep `SourceManifest` in both blueprints and in the edge test. The former manifest
  label remains only in historical review notes.

### 4. `RouteDecision` (EDGE-TYPES) vs `IntentSpec` (scan blueprint)

- **Edge affected:** route → scan (row 17)
- **Decision:** Row 17 updated to `IntentSpec` (owned by `intent`). `RouteDecision` at `route:39` carries `{selected, stages, reservation, snapshot_digest}` — no `unknowns`, `effects`, or `acceptance_refs` fields. `scan`'s `ScanInput` requires `unknowns: Vec<Unknown>`. Route forwards the admitted `IntentSpec` to scan; `RouteDecision` is route's internal receipt, not the edge payload.
- **Action:** The canonical test id is `scan::tests::admitted_intent_spec_populates_scan_input`. `scan/BLUEPRINT.md §1,§9` already say `IntentSpec` — no change needed there.

### 5. Edge payload types closed by the meta-L8 addendum (implementation proof pending)

These types appeared in the edge table without an exact shared definition. Their canonical shapes
are now defined in `docs/LLD/LLD-META-L8-ADDENDUM.md §H`; the listed blueprint remains responsible
for re-exporting that shape before the first implementation PR. This closes design ambiguity only;
it does not prove runtime compatibility.

| Type | Edge | Defining blueprint §5 |
|---|---|---|
| `InputEnvelope` | user_cli → ingest | `user_cli/BLUEPRINT.md §5` |
| `ConnectorEnvelope` | connectors → ingest | `connectors/BLUEPRINT.md §5` |
| `DurableEvent` | ingest → store | `ingest/BLUEPRINT.md §5` |
| `StoreCommand` | control → store | `control/BLUEPRINT.md §5` |
| `DispatchRequest` | control → intent | `control/BLUEPRINT.md §5` |
| `WorkflowSelection` | scan → dag | `fleet-scan` owns it; `scan/BLUEPRINT.md §5` defines `ScanDecision` as an exact compatibility alias for the canonical wire enum |
| `WorkflowRecipe` | dag → planner | `dag/BLUEPRINT.md §5` |
| `SkipPlanningSignal` | dag → ready | `dag/BLUEPRINT.md §5` |
| `PublicationRequest` | approval → user_cli | `approval/BLUEPRINT.md §5` |
| `PostVerifyEvidence` | post → approval | `post/BLUEPRINT.md §5` |
| `PostVerifyFailure` | post → rollback | `post/BLUEPRINT.md §5` |
| `PublicationGrant` | approval → broker | `approval/BLUEPRINT.md §5` |
| `RepairSignal` | rollback → dag | `rollback/BLUEPRINT.md §5` |

`RequirementInput` and `Vec<Question>` are likewise canonical for the four probe edges. The probe
blueprints may use node-local input structs internally, but their boundary adapters must be
explicit and tested against those exact shared types.
