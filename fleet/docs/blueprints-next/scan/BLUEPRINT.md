# BLUEPRINT — `scan`

## 1. Identity and LLD path

- Node id / label / tag: `scan` / Ambiguity Scan / `deterministic`
- LLD authority: `docs/LLD/LLD.md §6`, `docs/LLD/LLD-META-L8-ADDENDUM.md §H`; `docs/LLD/lld-full-detail.architecture.json:components[id=scan]`
- Why this node exists: decide whether an unknown is material before spending probe or question budget.
- Incoming edges: `route -> scan`: admitted `IntentSpec` plus unknowns, effects, acceptance refs, dependencies.
- Outgoing edges: `scan -> probe_business|probe_tech|probe_learn|probe_research` only for material unknowns; `scan -> questions` indirectly through probes; `scan -> dag` when none remain or they are resolved.
- Build status: `partial` — `crates/fleet-scan` has a four-probe assessment, but not the LLD materiality gate or three-question limit.

## 2. Responsibility and non-goals

**Owns:** a pure materiality predicate, deterministic probe selection, and a bounded scan decision.

**Does not own:** model interpretation (`intent`), probe content (`probe_*`), question ranking (`questions`), DAG admission (`dag`), or durable receipts (`store/control`).

## 3. Boundary and authority

Pure/read-only port-driven logic. Input is untrusted proposal data; output is an explanation-bearing decision. It may authorize only bounded read/research probes. It cannot authorize a write, grant, model route, or readiness decision; the parent persists the decision and controls effects.

## 4. Crate/package layout

```text
crates/fleet-scan/{Cargo.toml,src/lib.rs,src/material.rs,src/types.rs,tests/material.rs}
```

`lib.rs` exports types only (≤30 lines); `material.rs` owns the four-term predicate (≤70); `types.rs` owns versioned records (≤70); the test file owns pure cases (≤80).

> **Crate status:** `scan` is NOT a new crate. This is an edit-in-place of the existing `crates/fleet-scan/`. Do not create a separate `crates/scan/` directory.

## 5. Public API contract

```rust
pub struct Unknown { pub field: String, pub alternatives: Vec<String>, pub effect_changes: bool,
    pub acceptance_changes: bool, pub missing_grant: bool, pub blocks_ready_node: bool }
pub struct ScanInput { pub unknowns: Vec<Unknown>, pub revision: u64 }
pub enum WorkflowSelection {
    Clear { revision: u64, recipe_digest: String, evidence_ref: String },
    Probe { kinds: Vec<String>, revision: u64, recipe_digest: String, evidence_ref: String },
}
pub type ScanDecision = WorkflowSelection;
pub enum ScanError { EmptyRevision, DuplicateField(String), TooManyUnknowns }
pub fn scan(input: &ScanInput) -> Result<ScanDecision, ScanError>;
```

`WorkflowSelection` is the canonical `scan -> dag` wire type from Addendum §H; `ScanDecision` is
only its local compatibility alias. The implementation may use an internal `ProbeKind` enum, but
the boundary emits the fixed kind strings. `recipe_digest` and `evidence_ref` are nonempty,
deterministic fixture-bound values. Precondition: revision is nonzero and fields are nonempty.
Postcondition: `Probe` contains only the fixed four kinds and only unknowns satisfying at least one
materiality term. No panic or hidden IO.

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `Unknown` | material iff `effect_changes OR acceptance_changes OR missing_grant OR blocks_ready_node` | cosmetic ambiguity launching work | malformed `ScanError` / 7 |
| `ScanDecision` | clear or nonempty bounded probe set; revision preserved | stale decision reuse | stale parent refusal / 6 |
| probe set | at most four; deterministic order | unbounded fan-out | 6 |

All counts are checked integers; no floats, clock, RNG, filesystem, network, or model dependency. `Send + Sync` is expected. A zero unknown set is a valid checked input and yields `Clear`; a gate over zero candidates is not a pass.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-scan/src/assess.rs:39-61` `assess` | local source, current checkout | reuse fixed four-probe orchestration and fault collection | preserves named probe order | refactor test proves decision is not probe content |
| `crates/fleet-scan/src/input.rs:8-14` | local source | pass opaque task metadata | existing type boundary | revision/unknown mapping test |
| `serde` 1.x, MIT/Apache-2.0, [upstream](https://github.com/serde-rs/serde) | current workspace dependency; exact lockfile version must be recorded at implementation | serialize versioned records | stable wire primitive | `cargo test -p fleet-scan`; schema fixture smoke |
| `thiserror` 2.0.20, MIT/Apache-2.0, [upstream](https://github.com/dtolnay/thiserror) | local manifest; version is observed, not a future guarantee | typed errors | avoids stringly failures | `cargo test -p fleet-scan`; compile and error mapping |

Existing `merge_questions` at `crates/fleet-scan/src/merge.rs:23-46` caps at four, so it is not adopted as the LLD three-question authority.

### Cargo.toml snippet (pinned, edit-in-place)
```toml
[dependencies]
serde      = { version = "1", features = ["derive"] }
thiserror  = "2"
```

## 8. Behavior matrix

### `scan`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty unknown list -> `Clear`; missing revision -> typed refusal 7 |
| huge / negative | reject > configured maximum; unsigned revision rejects negative at parse |
| duplicate / concurrent | duplicate field -> refusal; pure calls are deterministic/concurrent-safe |
| partial failure / timeout | scan itself has none; probe timeout becomes missing evidence, not clear |
| stale / unavailable | parent compares revision; mismatch -> refusal 6; no provider call here |

## 9. Tiny implementation steps

1. In `src/types.rs`: Add `Unknown`, `ScanInput`, `ScanDecision`, and `ScanError`; in `src/lib.rs` re-export all public items; `cargo check -p fleet-scan` exits 0.
2. In `src/material.rs`: Implement the four-term OR predicate; `cargo test -p fleet-scan fleet_scan::tests::material_unknown_yields_probe` exits 0.
3. In `src/material.rs`: Add deterministic four-kind selection and duplicate-field validation; `cargo test -p fleet-scan fleet_scan::tests::duplicate_field_is_refused` exits 0.
4. In `src/lib.rs`: Wire the `route -> scan` adapter without IO using a typed `IntentSpec` fixture; `cargo test -p fleet-scan fleet_scan::tests::cosmetic_unknown_yields_clear` exits 0.
5. In `tests/material.rs`: Run `cargo test -p fleet-scan --no-fail-fast` and report `checked=8,total=8`; do not claim real binary reachability until composition exists.

## 10. Test matrix

**Unit tests:** each materiality term, all-false cosmetic unknown, duplicate field, revision preservation; assert exact variant.

**Integration/contract tests:** route admission fixture reaches scan and clear/probe payloads match `dag`/probe schemas; `checked=8,total=8` minimum.

**Hidden tests:** zero unknowns, four-term disagreement, duplicate unknown, stale revision, and oversized list; worker cannot see cases.

**Property tests:** for 256 fixed-seed generated unknowns, `decision=Probe` iff any term is true; no output exceeds four kinds; `checked=256,total=256`.

**Differential tests:** compare pure predicate with a reference truth table; current four-question merge is an explicitly allowed non-equivalent legacy behavior.

**Real-binary/effect test:** `FLEET_STATE="$PWD/var/fleet" ./target/debug/fleet` with an admitted read-only fixture; currently blocked because the proposed node is not wired to the binary.

### Named integration tests (these three must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `fleet_scan::tests::cosmetic_unknown_yields_clear` | `ScanInput` with one `Unknown` where all four terms (`effect_changes`, `acceptance_changes`, `missing_grant`, `blocks_ready_node`) are `false`; revision `1` | `Ok(ScanDecision::Clear { revision: 1, recipe_digest: "fixture-recipe-clear", evidence_ref: "fixture-scan-clear" })` | Replacing OR with AND in `is_material` still returns `Clear` on this input but breaks the one-term tests — used to anchor the all-false base case |
| `fleet_scan::tests::material_unknown_yields_probe` | `ScanInput` with one `Unknown` where only `effect_changes = true`; revision `2` | `Ok(ScanDecision::Probe { kinds: vec!["business"], revision: 2, recipe_digest: "fixture-recipe-probe", evidence_ref: "fixture-scan-probe" })` | `always_clear` stub returns `Clear`; this test fails because `Probe` is required; proves the single-term path is exercised |
| `fleet_scan::tests::duplicate_field_is_refused` | `ScanInput` with two `Unknown` records with identical `field` value `"auth_scope"`; revision `1` | `Err(ScanError::DuplicateField("auth_scope"))` | Any stub that accepts duplicates or returns `Clear`/`Probe` fails; proves input validation runs before materiality evaluation |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `is_material` in `src/material.rs` | Replace `||` (OR) with `&&` (AND) across all four terms | `fleet_scan::tests::material_unknown_yields_probe` | A single `effect_changes=true` must produce `Probe`; the AND mutation requires all four terms to be true, so the single-term input yields `Clear` and the test fails |
| `scan` in `src/lib.rs` | Always return `Ok(ScanDecision::Clear { revision: input.revision, recipe_digest: "", evidence_ref: "" })` regardless of unknowns | `fleet_scan::tests::material_unknown_yields_probe` | Constant-clear stub passes the cosmetic test but fails this one — probe admission is observable |
| `validate_revision` in `src/lib.rs` | Accept zero revision as valid | stale-revision hidden test | Stale plans must not proceed; a zero-revision pass violates `ScanError::EmptyRevision` |
| `select_probe_kinds` in `src/material.rs` | Return all four `ProbeKind` variants for every material unknown | bounded-selection hidden test | Fan-out is conditional on which term(s) triggered; a constant all-four return violates the bounded-selection invariant |
| duplicate-field check in `src/lib.rs` | Remove the `DuplicateField` branch; accept duplicates silently | `fleet_scan::tests::duplicate_field_is_refused` | Input validation is a first-class gate; a stub that skips it admits ambiguous inputs into the predicate |

Anti-stub check: a constant-return implementation, empty success, dropped reason, and wrong downstream revision must all fail named tests.

## 12. Verification recipe and denominators

```bash
cargo test -p fleet-scan materiality --no-fail-fast
cargo clippy -p fleet-scan --all-targets -- -D warnings
find crates/fleet-scan -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
```

Expected evidence: materiality `checked=8,total=8`; property `checked=256,total=256`; no file over 80 lines. Real binary remains an explicit blocker until wiring exists.

## 13. Definition of done

- `cargo test -p fleet-scan --no-fail-fast` exits 0 with `test result: ok`.
- `fleet_scan::tests::cosmetic_unknown_yields_clear` appears in test output and passes.
- `fleet_scan::tests::material_unknown_yields_probe` appears in test output and passes.
- `fleet_scan::tests::duplicate_field_is_refused` appears in test output and passes.
- `cargo clippy -p fleet-scan --all-targets -- -D warnings` exits 0.
- `find crates/fleet-scan -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- Property test output shows `checked=256,total=256` (256 fixed-seed generated unknowns; `Probe` iff any term is true).
- Mutation floor: `caught/total >= 80%` for the 5 named mutation targets in §11; a different-model reviewer re-derives materiality and kills the AND mutant manually.
- A composition test proves `route -> scan -> dag/probes`; no current provider capability claim is made.

## 14. Failure stories and review questions

**Failure → Cause → Fix:** cosmetic wording launched four agents → materiality was qualitative → enforce the four-term predicate and persist the reason.

1. Can a model or probe turn a cosmetic unknown into a write? 2. What proves stale decisions are rejected? 3. Is the current four-question behavior incorrectly being treated as the LLD three-question contract?
