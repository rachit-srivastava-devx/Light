# Fleet blueprint migration: quality evidence

Research group C, captured 2026-09-11 (Asia/Kolkata). This is a tool-selection
record for the `review`, `verify`, `candidate`, `offline`, and `post` nodes. A
release/version below is a point-in-time primary-source observation, not a
floating dependency recommendation.

## Decision summary

Use the Rust/Python runners already native to the workspace, retain `proptest`
and `trybuild`, and make mutation, differential, secret, dependency, SBOM,
benchmark, and model-review results explicit evidence records. Prefer pinned
lockfile versions and local binaries in the offline path. A tool is never a
passing gate merely because it started: it must report a positive denominator,
its exit status, and its artifact/input set.

| Node | Required evidence | Candidate tools |
|---|---|---|
| `review` | Independent findings with input/model/prompt hashes; no self-approval | Semgrep/Ruff plus separately invoked Gemini CLI reviewer |
| `verify` | Real binary tests, property/compile-fail tests, mutation and differential results | Cargo/Pytest, `proptest`, `trybuild`, `cargo-mutants`, `cargo-fuzz`, Hypothesis/DeepDiff |
| `candidate` | Baseline-vs-candidate behavior and statistically bounded performance | Reference oracle, Criterion, pytest-benchmark, Hyperfine, SciPy/statsmodels |
| `offline` | No network dependency, pinned toolchain, lockfiles, reproducible local inputs | Cargo `--locked`/offline, `uv --offline`, local scanners |
| `post` | Secret/dependency/SBOM/provenance receipts tied to exact artifact digest | Gitleaks, OSV-Scanner, cargo-deny/audit, Syft |

## Primary-source inventory

Versions and dates were read from official crates.io/PyPI/GitHub sources on
2026-09-11. GitHub commit rows pin the observed default-branch commit; release
rows pin the latest official release observed that day.

| Tool | Version or commit/date | License | Why it fits | Limitations / boundary | Local smoke |
|---|---|---|---|---|---|
| Cargo test / Rust harness | Cargo/rustc 1.98.0; rustc commit `88d9e12ae` (2026-08-18) | MIT/Apache-2.0 | Native Rust workspace runner; supports real binary integration tests and doctests. | Unit green is not proof of subprocess, ledger, network, or artifact behavior. | `cargo test --workspace --no-fail-fast` |
| pytest | 9.1.1 PyPI release (2026-06-19); repo lock resolves 8.4.2 | MIT | Native Python runner, fixtures, parametrization, subprocess assertions, and JUnit output. | Root manifest declares `<9.0`; system pytest 9.1.1 is not the repo environment. | `./.venv/bin/python -m pytest -q` in `crates/fleet-crew` |
| proptest | 1.11.0 (2026-03-24) | MIT OR Apache-2.0 | Already declared by `fleet-context`; generates shrinking counterexamples for state, parser, and invariant properties. | Random generation does not replace a curated corpus; flaky time/network properties need deterministic strategies. | `cargo test -p fleet-context -- --nocapture` |
| trybuild | 1.0.121 (2026-09-08) | MIT OR Apache-2.0 | Already declared by `fleet-lifecycle`; compile-fail UI tests make type/contract rejection behavior reviewable. | `.stderr` expectations are compiler-version-sensitive and can overfit diagnostics. | `cargo test -p fleet-lifecycle --test '*'` (use actual target) |
| cargo-mutants | 27.1.0 (2026-06-02); commit `fe82f1832778a591ab74248010fb40e699defafe` (2026-08-23) | MIT | Measures whether tests kill behavior-changing Rust mutants; useful at `verify` and as a candidate quality floor. | Separate mutant selection, timeouts, compile failures, and equivalent mutants; zero mutants is not a pass. | `cargo mutants --workspace --timeout 60` |
| cargo-nextest | 0.9.144 (2026-09-10); commit `9718c77afbc4b18f6e96b89acd7ce3e86c00b7e5` (2026-09-10) | MIT OR Apache-2.0 | Test isolation, retries and machine-readable status for a large workspace. Candidate; not installed locally. | Runner only; retries can hide flakes unless every retry is recorded. | `cargo nextest run --workspace --no-fail-fast` |
| cargo-llvm-cov | 0.9.1 (2026-09-06); commit `0dc905679ae6b4740b186f6e2c2ef1ab78fb76d7` (2026-09-06) | MIT OR Apache-2.0 | LLVM line/region/function coverage with machine-readable output. | Coverage is execution, not correctness; generated-code policy must be explicit. | `cargo llvm-cov --workspace --lcov --output-path /tmp/fleet.lcov` |
| cargo-fuzz | 0.13.2 (2026-06-09); commit `bf2fc668dafda5295aa6fd01825ee67b885f0f2b` (2026-07-20) | Apache-2.0 | libFuzzer harnesses support parser/state-machine stress and differential targets. | Needs recorded seed/corpus/toolchain, time budget, and corpus denominator. | `cargo fuzz list`; then bounded `cargo fuzz run <target> -- -runs=1000` |
| Differential oracle | No required package; existing Fleet reference/oracle plus canonical JSON/hash comparison. Optional DeepDiff 9.1.0 (2026-05-15), MIT. | MIT for DeepDiff | Same immutable corpus through reference and candidate, comparing exit code, receipts, state effects, and contract output. | Text equality is too strict for allowed nondeterminism; permissive normalization can hide regressions. | `reference < corpus > ref.json; candidate < corpus > cand.json; diff --unified ref.json cand.json` with schema-aware normalization |
| Gitleaks | 8.30.1 (2026-03-21); commit `b58d3f102cf3a2c84cb7f923d05c25c9b1aed84b` (2026-07-22) | MIT | History and working-tree secret detection for `post`; official repo documents Git, file, stdin, Docker, and CI modes. | Regex/entropy scanning has false positives and misses unknown encodings; upstream says it is feature-complete and future work is security patches. | `gitleaks git --no-banner --redact` |
| OSV-Scanner | 2.5.1 (2026-08-17); commit `1f87b5cb9781a03a9b8b50dfc1eac1b2056c79d5` (2026-09-11) | Apache-2.0 | Scans Cargo, Python, npm and other lockfiles against OSV; suited to post-release dependency evidence. | Advisory is not exploitability or reachability proof; record database freshness and lockfile scope. | `osv-scanner scan source -r .` |
| cargo-deny | 0.20.2 (2026-07-09) | MIT | Local policy surface for advisories, licenses, bans, and sources in Cargo dependencies. | Default policy may reject every license until an explicit allowlist is reviewed; policy is evidence. | `cargo deny check` |
| cargo-audit | 0.22.2 (2026-06-05) | MIT OR Apache-2.0 | Independent RustSec advisory scan beside cargo-deny. | Known advisories only; allowed warnings must not silently become green. | `cargo audit` |
| Syft | 1.51.1 (2026-08-27); commit `3ec2e33d73fedc786c615b04d8cbd529e32a5bf7` (2026-09-11) | Apache-2.0 | Generates SPDX/CycloneDX SBOMs from binaries, filesystems and containers. | Inventories identifiable components; it does not prove license approval, vulnerability absence, or reachability. Not installed locally. | `syft dir:. -o cyclonedx-json=/tmp/fleet-sbom.json` |
| Semgrep | 1.177.0 (2026-09-10); commit `0516c0f23a3dceac5c8f5ff3fecd402af4450182` (2026-09-10) | LGPL-2.1 | Pattern/static analysis for Python, YAML, shell and glue code around Rust. | Rule packs/auto config can change; pin rules/config and count scanned files/findings. | `semgrep scan --config auto --error --exclude target .` |
| Ruff | 0.16.7 current PyPI (2026-09-10); local system 0.15.14 | MIT | Fast Python lint/format/import checks; already declared by `fleet-crew`. | Lint is not a type checker or runtime proof; version drift matters. | `ruff check .` |
| Criterion | 0.8.2 (2026-02-04) | MIT OR Apache-2.0 | Rust statistical microbenchmarks with repeated samples and confidence intervals. | Noise and compiler/CPU changes require pinned environment and baseline comparison; never gate on one timing. | Add benchmark target, then `cargo bench --bench <name>` |
| Hyperfine | 1.20.0 (2026-02-04 GitHub release) | MIT | CLI repetitions/warmups for real Fleet commands and end-to-end latency. | Wall time is not correctness or internal attribution; control setup and cache state. | `hyperfine --warmup 1 --runs 10 'target/debug/fleet --help'` |
| pytest-benchmark | 5.3.0 (2026-08-23) | BSD-2-Clause | Python benchmark fixture, calibration and comparison output. | Environment-specific; do not compare machines without metadata. Not installed in repo venv. | `uv run --offline pytest --benchmark-only` after adding a benchmark test |
| SciPy / statsmodels | SciPy 1.18.1 (2026-08-21), statsmodels 0.15.0 (2026-08-27) | BSD-family; inspect bundled notices | Existing Python dependencies can compute confidence intervals, effect sizes, and parity statistics. | Invalid with too few/biased observations; record assumptions and versions. | `./.venv/bin/python -c 'import scipy,statsmodels; print(scipy.__version__,statsmodels.__version__)'` |
| Gemini CLI independent reviewer | 0.59.0 (2026-09-08); commit `ed2ac40df67a319bf348bd7e3d10494696b31b38` (2026-09-08) | Apache-2.0 | Open-source CLI for a separately invoked model/provider review of an immutable diff/evidence bundle. | Model opinion is not a test, proof, approval, or security scan; record provider/model and prompt injection controls. Not locally run. | `gemini --version`; then read-only review with structured findings and actor identity |

Primary sources: [Cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html),
[pytest](https://docs.pytest.org/en/stable/), [proptest](https://github.com/proptest-rs/proptest),
[trybuild](https://github.com/dtolnay/trybuild), [cargo-mutants](https://github.com/sourcefrog/cargo-mutants),
[nextest](https://github.com/nextest-rs/nextest), [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov),
[cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz), [DeepDiff](https://github.com/seperman/deepdiff),
[Gitleaks](https://github.com/gitleaks/gitleaks), [OSV-Scanner](https://github.com/google/osv-scanner),
[cargo-deny](https://github.com/EmbarkStudios/cargo-deny), [cargo-audit/RustSec](https://github.com/RustSec/rustsec),
[Syft](https://github.com/anchore/syft), [Semgrep](https://github.com/semgrep/semgrep),
[Ruff](https://github.com/astral-sh/ruff), [Criterion](https://github.com/criterion-rs/criterion.rs),
[Hyperfine](https://github.com/sharkdp/hyperfine), [pytest-benchmark](https://github.com/pytest-dev/pytest-benchmark),
[SciPy](https://github.com/scipy/scipy), [statsmodels](https://github.com/statsmodels/statsmodels),
and [Gemini CLI](https://github.com/google-gemini/gemini-cli).

## Evidence contract for all five nodes

Every result is a receipt with at least:

```json
{
  "tool": "name@version-or-commit",
  "node": "review|verify|candidate|offline|post",
  "input_digest": "sha256:...",
  "checked": 123,
  "total": 123,
  "status": "pass|fail|refused|environment_fault",
  "exit_code": 0,
  "started_at": "RFC3339",
  "finished_at": "RFC3339"
}
```

Rules:

1. `checked` and `total` are integers; `0/0` is a failure/refusal, never a
   pass. Publish the denominator in the human summary and receipt.
2. Test runners enumerate tests and invoke the real Fleet binary. Use
   `env!("CARGO_BIN_EXE_fleet")` in Rust integration tests; do not substitute a
   fake child through an environment override. Assert stdout/stderr, exit code,
   fd-3 worker protocol, ledger receipt, state mutation, and artifact identity.
3. Mutation evidence publishes `discovered`, `killed`, `timeout`, `unviable`,
   and `missed`; `discovered == 0` fails. Equivalent mutants require named,
   reviewed exclusions, not silent deletion.
4. Property/fuzz evidence publishes seed, corpus digest, generated cases,
   shrink result, time budget, and failures. One hand-written example is not a
   property denominator.
5. Differential evidence runs the same immutable corpus through reference and
   candidate, compares canonical structured outcomes, and records every allowed
   normalization. “Both exited zero” is insufficient: compare contracts,
   receipts, state effects, and hashes.
6. Secret/SBOM/static evidence publishes files, commits, lockfiles or artifact
   bytes scanned. An empty scan set fails unless the input manifest proves the
   repository/artifact is intentionally empty. SBOM output is hashed and tied
   to the exact release artifact.
7. Benchmark evidence publishes warmups, sample count, repetitions, CPU/OS,
   compiler, cache/network state, baseline, candidate, and uncertainty. Never
   report a mean or percentile without its sample denominator.
8. Model review is advisory and independent only when reviewer identity,
   provider/model metadata, prompt digest, input digest, output digest, and
   structured findings are recorded. A model cannot be the sole reviewer of its
   own patch or convert refusal/timeout into approval.
9. Offline mode fails closed if a dependency, advisory DB, model, or rule pack
   is missing. Use lockfiles and `CARGO_NET_OFFLINE=true cargo ... --locked`
   plus `uv --offline ...`; record cache/tool availability rather than claiming
   network isolation from intent alone.
10. Preserve typed exit semantics: `0` success, `3` environment fault, `6`
    invariant violation, `7` refusal, and `8` verification mismatch. Every
    refusal writes a receipt before exiting.

## Locally verified on 2026-09-11

These are commands actually run in this checkout; they are not claims about
remote CI or production:

| Command | Result |
|---|---|
| `./.venv/bin/python -m pytest -q` in `crates/fleet-crew` | PASS: `55 passed in 0.44s`; local venv pytest is 8.4.2. |
| `ruff check .` in `crates/fleet-crew` | PASS: `All checks passed!`; system Ruff was 0.15.14. |
| `cargo audit` | Exit 0; 1,243 advisories loaded and 2 allowed warnings reported (`lexical-core`, `lru`). This is not “no advisory exists.” |
| `cargo deny check` | Exit 4; advisories/bans/sources were okay, license policy failed because the repository has no allowlist configuration. |
| `osv-scanner scan source -r .` | Exit 1; 5 packages affected by 7 known vulnerabilities across Cargo.lock, `crates/fleet-crew/uv.lock`, and an unrelated dirty npm lockfile. |
| `gitleaks git --no-banner --redact` | Fail; 78 commits and about 28.37 MB scanned, 2 leaks found. Findings require triage; redaction prevents reproducing secret material. |
| `cargo test --workspace --no-fail-fast` | Started but not locally verified in this capture because other pre-existing Cargo test processes held package/build locks. No green claim. |

The checkout was already dirty before this research (`docs/DELTA.md`,
`.claude/skills/...`, `docs/LLD/...`, `docs/design/...`, and
`fleet-workflow-explorer/`). Those files are outside this research write-set;
their scan findings must not be misattributed to a clean migration artifact.

## Documented versus locally verified

“Documented” means upstream package/repository documentation and release
metadata support the capability, version, license, or command shown above.
“Locally verified” means only the commands in the preceding table and the
tool-version/availability probes were executed in this checkout. The following
remain unverified here: cargo-mutants mutation score, cargo-nextest adoption,
Syft output, Criterion/pytest-benchmark statistical baselines, cargo-fuzz
corpus quality, Gemini CLI execution/model metadata, remote CI, and production
release provenance.

No source, acceptance test, contract JSON, or existing blueprint was edited by
this research.
