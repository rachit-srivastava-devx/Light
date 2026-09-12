# BLUEPRINT — intent

## 1. Identity and LLD path

- Node id / label / tag: `intent / Intent Agent / model,gated`
- LLD authority: `docs/LLD/LLD.md §6` and `docs/LLD/lld-full-detail.architecture.json:components[id=intent]`
- Why this node exists: Convert natural-language requests into evidence-backed, schema-constrained IntentSpec proposals.
- Incoming edges: `control -> intent` dispatch with request text and bounded context.
- Outgoing edges: `intent -> route` typed IntentSpec.
- Build status: `partial`
- Exact workflow path: `user_cli/connectors -> ingest -> store/control -> intent -> route -> scan`.

## 2. Responsibility and non-goals

**Owns:** proposal extraction for workflow kind, goal, constraints, unknowns, acceptance references, requested effects, and evidence references.

**Does not own:** acceptance, authorization, route choice, confidence calibration, persistence, or any side effect. `control` and `route` independently validate and gate it.

## 3. Boundary and authority

The model sees untrusted request/context and may propose only an immutable `IntentSpec` with source spans. It cannot write store/ledger, select a provider, claim calibrated confidence, or authorize requested effects. The gate independently checks schema, evidence, deterministic kind candidate, effect permissions, and disagreement policy before route admission. If no eligible model/reviewer cohort exists, wait or ask the user.

## 4. Crate/package layout

```text
crates/intent/
  Cargo.toml
  src/lib.rs                 # types/ports, 40 lines
  src/schema.rs              # IntentSpec validation, 80 lines
  src/prompt.rs              # versioned prompt/context projection, 70 lines
  src/gate.rs                # deterministic independent check, 80 lines
  tests/adversarial.rs       # hidden prompt-injection and malformed output tests, 80 lines
```

## 5. Public API contract

```rust
pub trait IntentModel { fn propose(&mut self, input: IntentInput) -> Result<ModelProposal, IntentError>; }
pub fn validate(proposal: ModelProposal, input: &IntentInput, policy: &PolicySnapshot) -> Result<IntentSpec, IntentError>;
pub fn independent_kind(request: &str, effects: &[Effect]) -> WorkflowKind;
```

`validate` never trusts model confidence. It rejects unknown workflow kinds, absent evidence for material fields, effect mismatches, and extra authority fields.

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `IntentSpec` | schema version, kind, goal, effects, evidence refs present | ungrounded route | 7/8 |
| `requested_effects` | each effect is explicit and policy-checkable | hidden mutation | 6 |
| `unknowns` | material unknown has impact and resolution path | silent ambiguity | 7 |
| model output | bounded JSON, no actor/time/approval/settlement fields | authority forgery | 6 |

Allowed kinds are answer, investigate, review-only, small-change, feature, refactor, incident, multi-repo-change, and research/design. Agreement with deterministic kind is not required; disagreement chooses the conservative path and persists a receipt.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `src/pipeline/classify_stage.rs:9-28` | local LV; only TaskClass classification | extraction of composition boundary | retain router as decision authority | full IntentSpec path |
| `crates/fleet-plan/src/intake/sow_intent.rs` | local seam and hash test | evidence/digest fixture | preserve accepted SOW identity behavior | schema migration contract |
| `serde_json` / JSON Schema | https://serde.rs/ and https://json-schema.org/specification | constrained output validation | standard parser/schema semantics | malformed/unknown-field tests |

No current source proves a model-backed IntentSpec runtime; this node remains partial and gated.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix

### `validate`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty request becomes `General` only for deterministic read-only handling; missing goal/evidence refuses |
| wrong type / Unicode | malformed JSON or wrong field type refuses; Unicode source spans remain exact |
| huge / negative | request/context/output limits refuse; negative budgets or counts refuse |
| duplicate / concurrent | same proposal digest is idempotent; different proposal for same request creates a new version, never overwrites |
| partial failure / timeout | model timeout produces typed unavailable result and no route; gate receipt records checked/total |
| stale / unavailable | stale policy/model snapshot refuses; no unverified fallback model |

## 9. Tiny implementation steps

1. In `src/lib.rs`: define versioned `IntentSpec`, `ModelProposal`, `IntentInput`, and `IntentError` types → run `cargo check -p intent`.
2. In `src/schema.rs`: implement `validate` rejecting unknown kinds, absent evidence, and effect mismatches → add `intent::tests::unknown_kind_refuses` and `intent::tests::missing_evidence_refuses`; run `cargo test -p intent schema`.
3. In `src/gate.rs`: implement `independent_kind` deterministic check and conservative disagreement receipt → add `intent::tests::disagreement_chooses_conservative`; run `cargo test -p intent gate`.
4. In `src/prompt.rs`: add injected `IntentModel` port with versioned context projection → add `intent::tests::extra_authority_field_refuses` in `tests/adversarial.rs`; run `cargo test -p intent --test adversarial`.
5. In `tests/adversarial.rs`: wire route contract test verifying only gated `IntentSpec` with evidence digest reaches `route` → run `cargo test --test intent_route_contract --no-fail-fast`; output shows `checked=9,total=9`.

## 10. Test matrix

**Unit tests:** all nine kinds, missing evidence, unknown kind, conservative disagreement, effect-policy mismatch.

**Integration/contract tests:** model proposal → validation → route admission; receipt includes both model and deterministic candidates.

**Hidden tests:** prompt injection in request/context, extra actor/approval fields, output truncation, stale policy, and false confidence.

**Property tests:** 1,000 generated proposals either validate to schema-safe specs or typed refusal; fixed seed.

**Differential tests:** current `classify` TaskClass mapping versus independent kind for read-only/implementation/human-only fixtures; permitted difference is richer workflow kind.

**Real-binary/effect test:** real Fleet pipeline with an injected deterministic adapter; live provider/model qualification is an external proof gate.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `intent::tests::unknown_kind_refuses` | `ModelProposal` with `workflow_kind = "deploy-infrastructure"` (not in allowed-kinds list); valid evidence and effect fields | `Err(IntentError::UnknownKind)` | Catches removal of the kind allow-list guard in `src/schema.rs`; accepting unknown kinds lets arbitrary work reach route without schema validation |
| `intent::tests::disagreement_chooses_conservative` | `ModelProposal` with `kind = "feature"`; `independent_kind` deterministic check returns `"small-change"` for the same request | `IntentSpec` where `kind = "small-change"` (conservative) and a disagreement receipt is persisted | Catches deletion of the conservative disagreement path in `src/gate.rs`; without it the model candidate is accepted directly, breaking the conservative-kind assertion |
| `intent::tests::extra_authority_field_refuses` | `ModelProposal` JSON containing an extra `"actor_id"` field alongside schema-valid fields | `Err(IntentError::AuthorityField)` | Catches removal of the extra-field rejection in `src/schema.rs`; allowing authority fields enables forgery; any stub that strips and accepts unknown fields passes the field check silently |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `validate` in `src/schema.rs` | Remove kind allow-list check; accept any `workflow_kind` string | `intent::tests::unknown_kind_refuses` | Test sends an unknown kind and asserts `UnknownKind`; removing the guard passes the proposal through, breaking the error assertion |
| `validate` in `src/schema.rs` | Accept model-supplied `confidence` field as authority without independent evidence check | low-evidence property test | Confidence is not authority; removing the independent check allows ungrounded proposals to reach route without typed refusal |
| `independent_kind` in `src/gate.rs` | Remove conservative path; always use the model's proposed kind | `intent::tests::disagreement_chooses_conservative` | Test checks that model/deterministic disagreement routes the conservative kind; removing the path returns the model choice, breaking the kind assertion |
| `validate` extra-field check in `src/schema.rs` | Accept extra `actor_id` / `approval` fields in model output | `intent::tests::extra_authority_field_refuses` | Test injects a proposal with `actor_id`; allowing extra authority fields enables forgery; the `AuthorityField` error assertion fails |

Reviewer manually deletes the extra-field rejection; authority-forgery hidden test must fail.

Safety mutation floor: `caught/total >= 75%`.

## 12. Verification recipe and denominators

```bash
cargo test -p intent --no-fail-fast
cargo clippy -p intent --all-targets -- -D warnings
find crates/intent -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test --test intent_route_contract --no-fail-fast
cargo run --bin fleet -- __pipeline_probe --help
# Mutation floor: caught/total >= 75%
```

Expected evidence: workflow fixtures `checked=9,total=9`; proposal property cases `checked=1000,total=1000`; prompt-injection corpus `checked=12,total=12`; live model qualification remains unverified.

## 13. Definition of done

All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p intent --no-fail-fast` exits 0 with `test result: ok` in output.
- `intent::tests::disagreement_chooses_conservative` appears in test output and passes.
- `cargo clippy -p intent --all-targets -- -D warnings` exits 0.
- `find crates/intent -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `cargo test --test intent_route_contract --no-fail-fast` output contains `checked=9,total=9`.
- Mutation floor: `caught/total >= 75%` for all authority predicates in §11; manually deleting the extra-field rejection causes `intent::tests::extra_authority_field_refuses` to fail.

## 14. Failure stories and review questions

**Failure → Cause → Fix:** model labeled a credential edit as a small change → kind was trusted without independent effect check → downgrade to conservative route and persist disagreement.

1. What evidence grounds each field? 2. What is the conservative fallback? 3. Can a fake constant proposal pass?
