# Everything fleet depends on, and what it is for

One page: every crate, package, external binary and remote endpoint this repo uses, why it is
here, and — where it matters — whether it has ever actually run.

The standing constraint is **least self-code, most stitching**. The standing failure mode is the
opposite of what people expect: not "we wrote too much", but **declaring something adopted that
never executed**. Two tools were adopted and defended for a full day without either ever having
run, and a keyless lane sat in `bin/lanes.conf` for a day returning HTTP 400 to every request it
ever received. So the right-hand column of the external tables is evidence, not intent.

`ADOPTED` means an install was followed by a real repository command with recorded output.
`--help` does not qualify.

---

## Rust — workspace `Cargo.toml` + `crates/*/Cargo.toml`

| Crate | Version | What it does here |
|---|---|---|
| `clap` (derive) | 4 | the command grammar and argument parsing for every `fleet` verb |
| `clap_complete` | 4 | generates the bash/zsh/fish completions behind `fleet completions` |
| `crossterm` | 0.28 | terminal control for the REPL and the console |
| `ratatui` | 0.29 | the operator console's TUI widgets and layout |
| `rusqlite` (bundled) | 0.32 | local state store; `bundled` so there is no system SQLite dependency |
| `tree-sitter` | 0.24 | parses source for the code graph and blast-radius analysis |
| `tree-sitter-bash` | 0.23 | bash grammar — the detectors and gate scripts (`crates/fleet-verify/gates/`) are shell |
| `tree-sitter-python` | 0.23 | python grammar — the `crew/` proposal side |
| `tree-sitter-rust` | 0.23 | rust grammar — the kernel itself |
| `uuid` (v4) | 1 | run and task identifiers |
| `rmcp` (server, transport-io) | 3.1.4 | the MCP server surface so other agents can call fleet |
| `schemars` | 1 | JSON Schema for the MCP tool contracts |
| `serde` (derive) | 1 | serialisation across every receipt, contract and config |
| `serde_json` | 1.0.151 | the ledger is JSONL; every receipt round-trips through this |
| `toml` | 0.8 | reads `agents.toml` and `skills.toml` |
| `tokio` (io-std, macros, rt) | 1 | async runtime, required by `rmcp`. Deliberately minimal features |
| `blake3` | 1 | **the hash chain.** Artifact ids, ledger links, attestation digests |
| `fs2` | 0.4 | advisory file locks so concurrent runs cannot interleave ledger writes |
| `libc` | 0.2.189 | `setsid`, process-group and fd-3 plumbing for the worker boundary |

| Dev crate | Version | What it does here |
|---|---|---|
| `insta` | 1 | snapshot tests for CLI output — catches unintended surface changes |
| `trybuild` | 1 | **compile-fail tests.** Proves illegal SDLC transitions do not compile, producing real `E0599`/`E0382`. The typestate claim rests on this |

## Python — `crates/fleet-crew/pyproject.toml`

| Package | Version | What it does here |
|---|---|---|
| `pydantic` | ≥2,<3 | validates every proposal crossing into the kernel |
| `numpy` | ≥2,<3 | the parity experiment's numeric work |
| `statsmodels` | ≥0.14,<1 | **TOST equivalence testing** and Wilson intervals for requirement 5. Chosen because a non-significant ANOVA is not evidence of equivalence and hand-rolling TOST invites exactly that error |

## External binaries

| Tool | Purpose | Status |
|---|---|---|
| `git` | diff freezing, clean-tree checks, provenance. 152 call sites | core |
| `witness` | in-toto attestation generation and verification | **ADOPTED** — real keypair, real attestation, `witness verify` passed |
| `cosign` | the keypair witness signs with | **ADOPTED** |
| `cargo-mutants` | mutation testing. A guard is not accepted until a mutant makes it go red | **ADOPTED** — 26.7% → 70.7% |
| `conftest` | Rego policy checks over `crates/fleet-verify/gates/policy/` | **UNADOPTED-WITH-REASON** — `arch.rego` does not parse; two pairs pass under legacy-Rego compatibility |
| `opa` | — | **WITHDRAWN.** Used once interactively during the `D19` migration; nothing invokes it. `conftest` embeds OPA's engine, and that is conftest working, not `opa`. Caught by detector `M5` |
| `rekor` | transparency log | **WITHDRAWN** — no keyless path |
| `shellcheck` | shell linting in the gate | active |
| `codex` | the builder agent for the autonomous loop | active — 4 parallel workers |
| `qwen` | installed, candidate lane | probed, not wired |
| `curl` | every lane call | core |

`opa` and `rekor` are listed **because they were withdrawn.** A dependency list that only shows
what survived hides the more useful half.

## Keyless inference lanes — `crates/fleet-worker/assets/lanes.conf`

No API key, no login, no signup on any required path. Ordered failover in
`crates/fleet-worker/assets/freelane.sh`.

| Lane | Model | Dialect | Evidence |
|---|---|---|---|
| `api.llm7.io` (built-in default) | `codestral-latest` | `openai` | 12/12 at 8s pacing in the sweep; 1/2 in an earlier probe — variable |
| `blockrun.ai` | `nvidia/step-3.7-flash` | `openai` | answers with real token counts; resolves its own model |
| `devtoolbox-api…workers.dev` | `llama-3.2-3b-instruct` | `prompt` | 12/12 raw. **Returned HTTP 400 to every call for a day** — `freelane.sh` sent one hardcoded OpenAI body to every lane. Fixed by the `dialect` field |
| `ch.at` | — | — | **excluded deliberately.** Answers keylessly but does not report which model answered, and provenance without a model name is the forgery this design forbids |

32 candidates were called for real in the sweep; 6 were genuinely keyless and answered. The
rejects and why they failed are in [`archive/LANE-SWEEP.md`](archive/LANE-SWEEP.md) (a
2026-08-24 point-in-time sweep, archived as history).

## What is deliberately NOT depended on

| Not used | Why |
|---|---|
| any hosted LLM API with a key | keyless is a hard constraint — no key, no login, no account on a required path |
| a server, daemon or database | single-user, local, file-backed. State is `$FLEET_STATE` |
| a cloud transparency log | `rekor` withdrawn; attestation stays local and verifiable offline |
| floats for anything measured | integers or `null`. An unmeasured value is `null`, never `0` |

## Where the counts came from

`Cargo.toml` / `crates/*/Cargo.toml`, `crates/fleet-crew/pyproject.toml`,
`crates/fleet-worker/assets/lanes.conf`, and a grep for invocations across
`crates/fleet-verify/gates/*.sh` and `crates/fleet-verify/gates/corpus/*.sh`. Adoption verdicts are
from [`archive/ADOPTION.md`](archive/ADOPTION.md) (a 2026-08-24 audit, archived as history), which
holds the recorded smoke output. The ratio argument — 8,181 hand-written Rust lines against 21
direct crates — is in [`archive/STITCH.md`](archive/STITCH.md) (same vintage, also archived).
