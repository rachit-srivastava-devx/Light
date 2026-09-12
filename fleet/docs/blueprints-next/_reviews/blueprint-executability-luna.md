# Blueprint executability audit: Luna (4B)

## Verdict

**FAIL.** A low-parameter agent cannot implement the full set without intervention. The shared protocol is useful, but the node contracts are not executable as a closed set: several APIs, commands, topology statements, and adoption boundaries require human choices or contradict repository authority.

## Severity-ranked findings

### P0

1. **`verify` violates the explicit no-float authority rule.** `docs/blueprints-next/verify/BLUEPRINT.md §7` defines `LlvmCovLines { percent: f64 }` and `parse_coverage_percent(...) -> Result<f64, ...>` at lines 91-93, while the same blueprint §6 and `docs/blueprints-next/AGENTS.md` require integers/fixed strings for scores and thresholds. A 4B agent cannot implement both contracts without choosing a representation. Fix the coverage contract to a fixed-point/integer form and specify parsing/rounding semantics.

2. **The required real integration commands are not implementable from the current composition root.** `docs/blueprints-next/approval/BLUEPRINT.md §9/§12`, `broker`, `builder`, `context`, `notify`, `offline`, `ready`, `review`, `store`, and `verify` require commands such as `fleet approval`, `fleet broker`, `fleet build`, `fleet context`, `fleet notify`, `fleet offline`, `fleet ready`, `fleet review`, `fleet store`, and `fleet verify`. `src/cli/root.rs:19-79` has no corresponding visible subcommands. The only related real commands are `rollback`, `route`, `gate`, and the hidden probes. The blueprints also say the agent must not edit `src/` wiring (`IMPLEMENTATION-ORDER.md §One 4B-agent task`, item 11), so the agent is blocked between crate implementation and the mandated real-binary proof. Each node needs either an existing exact command or an explicitly authorized composition task and command contract.

3. **`probe_business` has a contradictory and incomplete public API.** `docs/blueprints-next/probe_business/BLUEPRINT.md §5` says `ProbeOutcome` and error types must be imported from `crates/fleet-scan/src/probe.rs` and not redefined, but its API uses `ProbeOutput` and `Question` without defining/importing them. Its §9 step 1 explicitly instructs the agent to define `ProbeOutput` and `ProbeError`. The imported `fleet-scan` dependency block names `Probe`/`ProbeOutcome`, not `ProbeOutput`. A 4B agent must decide whether to rename, import, or duplicate the types.

### P1

4. **LLD edge payloads are not consistently exact enough to implement.** `docs/LLD/lld-full-detail.architecture.json` names typed payloads for every connection, for example `scan -> probe_* = AmbiguityRequest`, `probe_* -> questions = Vec<Question>`, and `dag -> ready = SkipPlanningSignal`. Several blueprints replace these with informal or different payloads: `probe_business §1` says `RequirementInput`; `probe_business §5` defines `BusinessInput`; `scan §1` describes `IntentSpec plus unknowns`; `dag §1` says `clear/resolved ambiguity and typed intent`; and `ready §1` accepts `ReadyInput` without specifying the `SkipPlanningSignal` edge shape. The agent cannot produce wire-compatible edges without a human-selected canonical type mapping.

5. **`control` claims a direct `control -> store` edge while the LLD has no such connection.** `docs/blueprints-next/control/BLUEPRINT.md §1` lists `control -> store` as an incoming edge and §1/§8 describes durable effects through store/outbox. In the LLD JSON, the connection is `control -> store` only if interpreted as the JSON row `control,store`; however the blueprint labels it incoming to control, reverses the direction, and separately lists `control -> intent` as both incoming and outgoing in prose. This is an authority-direction ambiguity in the controller’s central write path. Correct the direction and provide one exact `StoreCommand`/`Commit` payload contract.

6. **Several model/adaptor boundaries leave the implementation decision open.** `connectors §7` lists `octocrab 0.54.1` and `io-gmail 0.3.0`, but says the exact Octocrab upstream commit is unresolved and leaves the Cargo pin as “version from §7 table”; `model_catalog §3` says ACP is only an adoption candidate; and the probe blueprints permit an “optional adapter” with no provider. The low-parameter protocol requires a concrete dependency, call site, smoke command, and limitation. These nodes can only be drafted, not implemented, until the allowed adapter lane and version/protocol are fixed.

7. **The planned file-size limit conflicts with the requested API and proof surface.** `docs/blueprints-next/_TEMPLATE.md §4` and `docs/blueprints-next/AGENTS.md` require every source and test file to be at most 80 lines, but `verify §7` embeds a multi-struct coverage parser and `verify §14` requires runner, secret scanning, receipt assembly, real-binary, mutation, and missing-tool proofs. `connectors §4` similarly assigns two provider adapters, auth, pagination, and fault tests while each file is capped at 80 lines. No decomposition into additional named files or exact ownership is given, so the agent must invent a package layout.

8. **`questions` exposes a caller-controlled cap despite a hard LLD limit.** `docs/blueprints-next/questions/BLUEPRINT.md §5` exposes `merge(candidates, max: NonZeroU8)` and says non-production caps cannot exceed three, but no API-level validation or error contract says how `max > 3` is rejected. The LLD edge is unconditionally “<= 3 concise questions.” A 4B implementation can legally honor `max=4` unless it infers the prose rule. Make the public API fixed at three or specify the exact `InvalidCap` behavior and test it.

### P2

9. **Fixture and binary ownership is under-specified across the set.** `IMPLEMENTATION-ORDER.md §Fixture naming contract` says the first implementation step creates each fixture, but multiple blueprints require real-binary fixtures without specifying the fixture JSON schema or the binary subcommand contract, for example `approval §9`, `broker §9`, `builder §9`, and `verify §9`. The command itself is absent for most of these nodes (P0 finding 2). A 4B agent cannot create a fixture that proves an edge rather than inventing a local test-only shape.

10. **`post` and `verify` duplicate gate authority without a typed handoff rule.** `verify §2/§5` owns gate execution, secret scan, denominators, and evidence; `post §2/§5` independently owns gate execution through another `GateRunner`, secret finding behavior, and denominator checks. The LLD has `verify -> integrate -> post`, but `post §1` accepts `required_gates` and reruns them. No rule states which result is authoritative, whether post may use a different gate set, or how the two evidence digests compose. This creates a hidden design choice in the approval path.

11. **`approval` and `broker` do not share a compilable grant contract.** `approval §5` returns `ApprovalGrant`; `broker §5` accepts an `ExternalEffect` containing `Grant`, while `broker §1` calls the incoming payload a consumed grant. No shared crate/type, conversion, serialization version, or exact digest binding is named. The LLD edge is typed `PublicationGrant`. The agent must invent an adapter or duplicate a security-sensitive capability type.

12. **The “stable-library adoption” evidence is not executable evidence in several nodes.** `connectors §7` calls external links “research” and leaves a checksum probe unresolved; `control §7` cites Tokio with a research-ledger checksum; `verify §7` says `cargo-llvm-cov` is installed by `cargo install` but does not pin/version it; and `review §7` invokes `semgrep`/`ruff` without a version or installation contract. `docs/blueprints-next/AGENTS.md` explicitly says a dependency declaration is not adoption proof. These are acceptable future proof gates, but not closed implementation instructions for Luna.

## Per-node disposition

The 32 node directories are present and each has a `BLUEPRINT.md`; the identity rows match the 32 component IDs in `docs/LLD/lld-full-detail.architecture.json`, and `NODE-MAP.md` has one row per directory. That structural coverage is a **PASS**, but executability is **FAIL** for every node in the implementation sequence because the shared real-integration rule is unsatisfied by the absent composition commands. The most direct node blockers are:

- **Blocked by P0 contract conflict:** `verify`, `probe_business`.
- **Blocked by absent real command:** `approval`, `broker`, `builder`, `context`, `notify`, `offline`, `ready`, `review`, `store`, `verify`.
- **Blocked by edge/type ambiguity:** `control`, `probe_business`, `probe_tech`, `probe_learn`, `probe_research`, `questions`, `dag`, `ready`, `approval`, `broker`, `post`.
- **Conditionally executable after explicit external-adapter decisions:** `connectors`, `model_catalog`, `probe_research`, and any node that invokes `semgrep`, `ruff`, `gitleaks`, or `cargo-llvm-cov`.

## Required closure before PASS

1. Replace all authority-conflicting numeric types and define exact fixed-point behavior.
2. Publish one canonical Rust type for every LLD edge, including `PublicationGrant`, probe input/output, skip planning, and post/verify evidence.
3. Add or explicitly authorize the composition-root commands used by each real-binary step.
4. Resolve external dependency versions, checksums, installation/probe commands, and provider lanes.
5. Split over-80-line responsibilities into named files and make each step’s write-set explicit.

**Final result: FAIL.**
