# CONFORMANCE — does the build match the (corrected) blueprint?

Measured 2026-08-24 against `../blueprints/Fleet-L8-Deep-Dive/`, **after** the 12 `DELTA.md`
findings corrected it. Every row is a command, not an opinion.

## The headline, stated the uncomfortable way

> **The attestation carries 8 of 8 delivery elements, and all 8 carry REAL evidence (100%) after
> iteration 4.** `builder != verifier` (stub vs stub-verifier), `o1_author != o2_author`
> (lead vs verifier), and `blind_suite` records **4 of 4 absences actually CHECKED** — not asserted.
>
> The journey is the honest part: **12% → 62% → 100%**, with the first number corrected *downward*
> from a flattering 25% before it moved up.

That is **exactly what blueprint `16` P0 promises** — *"P0's attestation is STRUCTURE, NOT TRUST"* —
and it is the number that must be published, because "8 of 8 elements present" read alone is the
denominator dishonesty this whole project is named after. **A key is not evidence.**

| element | state | becomes real at |
|---|---|---|
| `sow` | **real** — the task, recorded | — |
| `blast_radius` | **placeholder** (`P0-local-repo`) — was miscounted as real | iteration 3 (wiring the recorded diff) |
| `blind_suite` | placeholder | P1 (the suite exists but is not yet author-blinded per `03` §2.4) |
| `independent_verification` | placeholder | P1 (needs a second model in the loop) |
| `adequacy` | `null` | P1 (`cargo-mutants` wired but not gating) |
| `rollback` | `null` | P1 (revert-and-assert-red not yet executed per change) |
| `cost` | placeholder | P1 (needs transcript parsing) |
| **`oracle_independence`** | placeholder | **P1 — the O2 holdout oracle. This is the one that matters** (`21` §3): until it lands, the lead authors the only oracle and the attestation proves a process ran, not that the oracle was good. |

## P0 exit criteria (`16`)

| criterion | verdict |
|---|---|
| prompt → SOW → frozen artifact → attestation → verify | **PASS**, driven end-to-end on a clean state |
| `fleet attest verify` exits 0; tampered → exit 8 | **PASS** |
| artifact provably immutable | **PASS** — the OS refuses the write (`EACCES`) |
| 20 concurrent appends: rows kept · chain valid · no `prev_hash` reuse | **PASS** (the third assertion is the one a row count misses) |
| refusals write a receipt | **PASS** |
| agent registry with charters | **PASS** |
| ≥20 corpus detectors | **EXCEEDED** — 96 files; 25 mechanisable, 70 excluded, 1 env-gated |
| witness / in-toto **tool** adopted | **PARTIAL** — the *format* conforms (in-toto Statement, custom predicateType, subject digest = artifact id). The **tool is NOT adopted**: no smoke test, `witness` is not installed (`D1`). Conforming to a format ≠ adopting a tool. |

## Gate wall — 9 of 9

`fmt · clippy -D warnings · unit tests · acceptance-builds · cargo-deny · cargo-audit · gitleaks ·
acceptance (26/26) · corpus (0 caught / 25 mechanisable / 70 excluded)`

One documented exception: `paste` unmaintained (RUSTSEC-2024-0436) — a compile-time transitive dep
of ratatui, no runtime surface, with a named reason and a review trigger in `keel/deny.toml`.

## Where the build does NOT yet conform

1. *(CLOSED 2026-08-24 — this entry was STALE.)* The O2 holdout is adversarially tested: four
   fixtures in `main.rs` (`adversarial_builder_poisoned_oracle_is_inadequate`, impossibly-strict,
   broken-build, correct-build) and `cargo test -- adversarial` reports **4 passed**. They were
   mutation-tested by the lead rather than the author: forcing the discriminator to always ACCEPT
   turns 3 of the 4 red, and the fourth IS the ACCEPT case. Found by re-auditing this file, which
   is the second time it has drifted — an honesty ledger that is not re-audited becomes fiction.

2. *(CLOSED 2026-08-24 — and the answer is bad news.)* Gate built, and it now REFUSES an
   uncalibrated floor rather than passing vacuously. Calibrated for real: **244 mutants,
   62 caught / 170 missed / 12 unviable = 26.7%, Wilson95 [21.4%, 32.8%]**. Floor committed at
   the measured `62/232`. Opt-in via `FLEET_MUTANTS=1` (a 22-minute default stage would be
   bypassed). The 170 misses are a worklist; `is_improvement` survives `>`->`==`/`<`/`>=`. See `D25`.
3. *(CLOSED 2026-08-24.)* **`crew.sow` is now on the critical path.** `fleet sow` invokes the
   Python intent validator, persists the exact-task SOW, and exits 9 for human review. Both
   `fleet run` and `fleet swarm dispatch` refuse with exit 7 until the task-bound SOW is accepted;
   the automation bypass is explicit in stderr and the receipt ledger. Model adapters remain a
   separate execution concern and are not duplicated by the Rust bridge.
4. *(RESOLVED 2026-08-24 — by withdrawing the claim, not by wiring it.)* `rekor-cli` is on PATH
   and invoked by nothing, and it should stay that way: `--rekor_server` defaults to
   `https://rekor.sigstore.dev`, a **hosted service**. Keyless means no API key, no login, and no
   hosted service on a required path — a transparency log on the attestation path would break the
   exact property this design exists to hold, and self-hosting Rekor reintroduces a server the
   operator must run. `witness` and `conftest` ARE adopted and smoke-tested; `opa` is used by
   `conftest` internally, not by fleet. Detector `M5` fails if any tool marked ADOPTED has no
   caller outside the documentation.
5. *(CLOSED 2026-08-24: `graph::reachability_report()` now enumerates every module's pub entry
   points, resolves which are reachable from `dispatch()`, publishes `N of M modules reachable`,
   and treats `M==0` as a failure so measuring nothing can never read as success. Verified by
   the lead rather than the author: adding an orphan module turns the test red naming it;
   removing it returns green.)* Originally: **Reachability is asserted, not enforced.** `D12` proposes a reachability *element*; today it is
   6 acceptance assertions, not a gate over all modules.

## The blueprint changes these findings force

| finding | blueprint change |
|---|---|
| `D12` | `10`'s delivery elements need a **reachability** element — a module unreachable from the entrypoint is dead code wearing a green build |
| `D11` | `07` §4 states the *predicate* for safe parallelism but never says **how to partition**; for a single binary, agents own modules and the lead wires the entrypoint |
| `D3`/`D5` | `02` §6 needs a **7th adapter capability** (session scaffolding disableable) and a dispatch-time precondition probe |
| `D6` | `05`'s `checked==0` rule must apply to the **acceptance suite itself**, not only to gates |
| `D8`/`D10` | `12` §5's rejection narrows to "gateways that do not report the resolved model"; `13` §2.1's vector row moves to `turbovec` |


---

## Iteration log

| iter | gates | acceptance | detectors | real elements | what changed |
|---|---|---|---|---|---|
| 1 | 9/9 | 26/26 | 25 | 1 of 8 (12%) | first working path: prompt → frozen artifact → attestation → verify |
| 2 | 9/9 | **31/31** | **27** | 1 of 8 (12%) | O1/O2 + the **2×2 discriminator with all four quadrants tested** · Wilson `(0.4441, 0.7231)` reproducing the blueprint's own number · two-sided adequacy · model adapters routed · 5 silent-refusal assertions (`D12` recurring in new code) |
| 3 | **9/9** | **31/31** | **27** | **5 of 8 (62%)** | wired `adequacy`, `blast_radius`, `rollback`, `cost` **into the `run` pipeline**. `rollback` now genuinely reverts in a scratch worktree and asserts the suite goes RED. `cost` reports `tokens: null` — not 0 — when no model ran. `adequacy` reports `no-measurable-surface` rather than fabricating a rate. |

**The gap iteration 2 exposed, stated precisely:** the capabilities exist and are unit-tested; `run`
does not call them. `adjudicate` and `adequacy` are *commands*, not *steps*. That is `D12` one level
up — **a capability that exists but is not on the path is dead code wearing a passing test** — and it
is why "1 of 8" did not move despite three merged slices of real work.

| 4 | **9/9** | **31/31** | **27** | **8 of 8 (100%)** | second-agent verification wired into `run`: `blind_suite` (4 checked absences), `independent_verification` (builder≠verifier enforced), `oracle_independence` (o1≠o2, hashes recorded, quadrant ACCEPT) |
