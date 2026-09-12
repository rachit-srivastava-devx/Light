# BLUEPRINT — model_catalog

## 1. Identity and LLD path

- Node id / label / tag: `model_catalog / Model / Adapter Catalog / deterministic`
- LLD authority: `docs/LLD/LLD.md §9, §19` and `docs/LLD/lld-full-detail.architecture.json:components[id=model_catalog]`
- Why this node exists: Discover trusted adapters, probe capabilities, record endpoint-reported model identity, and publish immutable snapshots to route.
- Incoming edges: configured executable/provider candidates and capability probes.
- Outgoing edges: `model_catalog -> route` capability snapshot.
- Build status: `partial`
- Exact workflow path: `user_cli/connectors -> ingest -> store/control -> intent -> route -> scan`.

## 2. Responsibility and non-goals

**Owns:** candidate registry, canonical executable/version/digest capture, bounded noninteractive probe, capability evidence, probation/qualification state, and snapshot digest.

**Does not own:** installation/login, inference quality claims, budget settlement, policy authorization, scheduling, or provider truth beyond observed metadata.

### Failover contract

The catalog publishes a sorted, immutable set of qualified alternatives with freshness and
evidence. It does not retry or switch a live attempt. `route` selects an alternative only from the
snapshot; `control` persists the wait/restart fence, reservation release, and retry budget before
another attempt. An unavailable or unqualified candidate remains explicit `Unknown`/`Unsupported`;
it is never silently promoted to failover.

## 3. Boundary and authority

Discovery searches explicit configured paths and trusted PATH entries; it must not execute every binary. A probe runs with timeout and no Fleet ledger/state/socket paths, credentials, actor, timestamp, resolved model authority, or settlement fields. ACP is an adoption candidate, not proof of auth, quota, quality, or sandbox compatibility. Route consumes only an immutable catalog snapshot.

## 4. Crate/package layout

```text
crates/model-catalog/
  Cargo.toml
  src/lib.rs                 # catalog types/ports, 40 lines
  src/discover.rs            # path/provenance discovery, 80 lines
  src/probe.rs               # ACP/native probe adapters, 80 lines
  src/qualification.rs       # probation/trial predicates, 80 lines
  tests/protocol.rs          # scripted and real-binary probes, 80 lines
```

## 5. Public API contract

```rust
pub trait Probe { fn probe(&mut self, candidate: &Candidate, timeout: std::time::Duration) -> Result<CapabilityReport, CatalogError>; }
pub fn discover(config: &DiscoveryConfig) -> Result<Vec<Candidate>, CatalogError>;
pub fn snapshot(records: Vec<CapabilityReport>) -> Result<CatalogSnapshot, CatalogError>;
```

`CapabilityReport` records supported, unsupported, or unknown for every property with timestamp, executable version, and evidence reference. A reported model name is observation, not authorization.

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| candidate | canonical path, version, digest, provenance present | executable swap | 6/8 |
| capability | tri-state value plus evidence timestamp | unknown as false/true | 8 |
| qualification | configured fresh trials and floor met | availability mistaken for quality | 7 |
| snapshot | immutable sorted records and digest | route drift mid-run | 6 |

Unknown is `None` with a reason. A zero-trial cohort cannot qualify. Catalog state is single-writer through store; probes are process-isolated.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-crew/crew/adapters/capability_probe.py:35-38` | local LV; current report construction | adapter probe fixtures | preserve capability field vocabulary | real installed executable probe |
| `crates/fleet-crew/crew/adapters/capability_report.py` | local typed Python record | compatibility mapping | avoid breaking existing crew tests | Rust/Python contract parity |
| `agent-client-protocol 2.1.0` | Apache-2.0; crates.io/SDK/v2 quickstart links and checksum in research | ACP probe/transport candidate | official typed SDK | exact-pin compile, stdio pair, endpoint metadata |
| native CLI docs | Codex app-server, Gemini headless, OpenCode ACP, Claude guide links in `docs/LLD/LLD.md §22.2` | provider-specific adapters | protocol-native behavior | versioned live probes |

The current catalog is incomplete; no model/provider availability claim is made from dependency declarations or `default_runtime`.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix

### `discover`, `probe`, `snapshot`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty configured candidate set returns refusal; no PATH-wide execution; missing executable is unavailable |
| wrong type / Unicode | malformed capability JSON or invalid path refuses; Unicode model metadata is preserved |
| huge / negative | cap output at 1 MiB/frame and queue at 8 MiB; negative timeout/trial count refuses |
| duplicate / concurrent | same canonical path+digest coalesces; changed digest creates a new probation record; one probe per candidate lease |
| partial failure / timeout | return unknown capability with reason and evidence; kill/reap child; never qualify |
| stale / unavailable | stale version/digest invalidates snapshot; unsupported ACP capability remains unsupported, not assumed |

## 9. Tiny implementation steps

1. In `src/lib.rs`: define `Candidate`, `CapabilityReport`, `CatalogSnapshot`, and `CatalogError` types with tri-state capability values → run `cargo check -p model-catalog`.
2. In `src/discover.rs`: implement explicit-path discovery and digest capture rejecting PATH-wide execution → add `model_catalog::tests::unknown_path_not_executed` and `model_catalog::tests::digest_change_invalidates_candidate`; run `cargo test -p model-catalog discover`.
3. In `src/probe.rs`: add scripted ACP/native probe parser recording tri-state capability per field → add `model_catalog::tests::malformed_output_returns_unknown` and `model_catalog::tests::timeout_returns_unknown`; run `cargo test -p model-catalog probe`.
4. In `src/qualification.rs`: add probation qualification requiring nonzero fresh trial floor → add `model_catalog::tests::zero_trial_cannot_qualify`; run `cargo test -p model-catalog qualification`.
5. In `tests/protocol.rs`: publish immutable snapshot to route and run a real installed adapter probe → run `cargo test -p model-catalog --test protocol`; `cargo run --bin fleet -- agents --json` shows `checked=1,total=1`.

## 10. Test matrix

**Unit tests:** path filtering, digest change, tri-state capability, snapshot ordering/digest, qualification floor.

**Integration/contract tests:** catalog snapshot is consumed by route; stale snapshot is refused; Python capability fixture parity is asserted.

**Hidden tests:** PATH executable swap, hidden startup hook, malformed stdout, fake reported model identity, unsupported cancellation, no credential leakage.

**Property tests:** 1,000 candidate orderings produce identical snapshot digest; fixed seed.

**Differential tests:** Python `fleet-crew` capability report versus Rust catalog mapping; differences only where fields are explicitly unsupported.

**Real-binary/effect test:** launch an approved adapter through the real Fleet child path, capture answer text plus endpoint-reported model metadata; no fake child is sufficient.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `model_catalog::tests::unknown_path_not_executed` | `DiscoveryConfig` with explicit configured paths that do not include a test binary placed on PATH | No execution of the PATH-resident test binary; `discover` returns only configured candidates | Catches removal of the explicit-path filter in `src/catalog.rs`; PATH-wide discovery executes the binary, breaking the no-execution assertion |
| `model_catalog::tests::timeout_returns_unknown` | `Candidate` for a scripted adapter that sleeps beyond `timeout: Duration`; `probe` called with a short timeout | `CapabilityReport` where every capability field is `Unknown` with a timeout reason | Catches mutant that maps probe timeout to `Supported`; any implementation that qualifies a timed-out probe returns the wrong tri-state |
| `model_catalog::tests::zero_trial_cannot_qualify` | Qualification predicate called with `fresh_trials = 0` and empty evidence | `Err(CatalogError::InsufficientTrials)` | Catches removal of the trial-floor check in `src/catalog.rs`; accepting zero-trial cohorts means quality evidence has no denominator, breaking the refusal assertion |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `discover` in `src/catalog.rs` | Execute all PATH-resident binaries; remove explicit-configured-path filter | `model_catalog::tests::unknown_path_not_executed` | Test places a canary binary on PATH outside configured paths; PATH-wide execution runs it, breaking the no-execution assertion |
| `probe` in `src/probe.rs` | Map probe timeout to `Supported` instead of `Unknown` | `model_catalog::tests::timeout_returns_unknown` | Test sends a probe that times out and asserts every field is `Unknown`; the mutant returns `Supported`, breaking the tri-state assertion |
| `qualify` in `src/qualification.rs` | Skip trial-floor check; allow qualification with `fresh_trials = 0` | `model_catalog::tests::zero_trial_cannot_qualify` | Test calls qualify with zero trials and asserts `InsufficientTrials`; removing the floor check returns a qualified cohort, breaking the error assertion |
| `snapshot` in `src/catalog.rs` | Trust endpoint's self-reported model name as resolved identity | `model_catalog::tests::timeout_returns_unknown` + endpoint metadata mismatch test | Resolved identity must be observed, not claimed; accepting the self-reported name allows fake model identities, breaking the metadata-mismatch assertion |

Reviewer manually changes timeout to qualification success; hidden timeout test must fail.

Safety mutation floor: `caught/total >= 75%`.

## 12. Verification recipe and denominators

```bash
cargo test -p model-catalog --no-fail-fast
cargo clippy -p model-catalog --all-targets -- -D warnings
find crates/model-catalog -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test -p fleet-crew -- --no-fail-fast
cargo test --test catalog_route_contract --no-fail-fast
cargo run --bin fleet -- agents --json
# Mutation floor: caught/total >= 75%
```

Expected evidence: candidate fixtures `checked=5,total=5`; snapshot permutations `checked=1000,total=1000`; qualification trials `checked=12,total=12`; real adapter probe `checked=1,total=1` only after executable, answer, and endpoint model metadata are all observed. ACP exact-pin compatibility remains an external proof gate until run.

## 13. Definition of done

All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p model-catalog --no-fail-fast` exits 0 with `test result: ok` in output.
- `model_catalog::tests::zero_trial_cannot_qualify` appears in test output and passes.
- `cargo clippy -p model-catalog --all-targets -- -D warnings` exits 0.
- `find crates/model-catalog -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `cargo run --bin fleet -- agents --json` output contains `checked=1,total=1`.
- Mutation floor: `caught/total >= 75%` for the named mutation targets in §11; manually changing timeout to qualification success causes `model_catalog::tests::timeout_returns_unknown` to fail.

## 14. Failure stories and review questions

**Failure → Cause → Fix:** catalog said a requested model was active → only a config alias was observed → record executable version and endpoint-reported metadata, otherwise remain unknown.

1. What exactly qualifies a model? 2. Can PATH discovery execute arbitrary files? 3. Is provider billing/auth still unverified?
