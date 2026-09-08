# NIGHT-PLAN — resume anchor for the unattended run

Owner is asleep. Directive: make fleet **work**, and work **beautifully**. Sonnet agents only
(Opus quota exhausted); code fixes may also be driven by the keyless CLI (see Tooling). Every claim
needs literal pasted output — a proxy is not the property.

## Tooling: opencode is DEAD, aider is the replacement

`opencode` 1.18.25 is installed and its `--version`/`--help` work, but `opencode run` **hangs and
emits zero bytes**, reproduced 3× (bare, `--pure --print-logs`, and with a configured keyless
provider). Do not use it. Its config was left pointing at the keyless provider; a backup of the
original sits beside it as `opencode.jsonc.bak-*`.

`aider` 0.86.2 works **keyless, no account**, verified by writing real code that compiled and passed
a test written independently afterwards:

```
export PATH="$HOME/.local/bin:$PATH"
OPENAI_API_BASE=https://api.llm7.io/v1 OPENAI_API_KEY=unused \
  aider --model openai/codestral-latest --yes --no-auto-commits --message '<task>'
```

Keyless endpoint facts (verified this session): `https://api.llm7.io/v1` answers `/v1/models` and
`/v1/chat/completions` with **no key**, and **supports tool calling** (returned a well-formed
`tool_calls` payload). Only `codestral-latest` is keyless — every other model on that host returns
401. Second verified keyless lane, `prompt` dialect (not OpenAI-shaped):
`https://devtoolbox-api.devtoolbox-api.workers.dev/ai/generate|llama-3.2-3b-instruct`.

## Baseline at the start of the night

```
cargo test --workspace --no-fail-fast   → 395 passed / 0 failed
cargo clippy --workspace --all-targets -- -D warnings → exit 0
find src crates -name '*.rs' | xargs wc -l | awk '$1>80'  → empty (hard rule holds)
```

## Ground truth: `docs/DX-AUDIT.md`

112/112 checks over 28 public + 5 hidden commands. **13 S1 (broken), 9 S2 (unusable).** Read it
before touching anything. Worst three:

1. **`fleet run` hangs forever, zero output** — the flagship command, and the one the whole docs
   walkthrough builds toward. Hangs even with stdin closed.
2. **Every copy-pasteable example in `docs/USING-FLEET.md` fails to parse.** The docs describe a
   nested-subcommand CLI (`sow accept`, `ledger verify`, `agents list`) that does not exist; the real
   binary is flat-flag.
3. **8 of 28 commands are undisclosed stubs** (`console skills adjudicate attest pr contract freeze
   mcp`) — always exit 3, "owning crate does not yet expose a public entry point", with nothing in
   `--help` saying so.

Also: all 28 help descriptions are **empty**; only **1 of 28** supports `--json`; `oracle` and `gate`
hang on no-args; `swarm` cross-wires `--task`/`--prompt`.

## Work order (a beautiful CLI over a hanging command is worthless)

| # | Lane | Scope | State |
|---|---|---|---|
| 1 | Fix the three hangs (`run`, `oracle`, `gate`) + `swarm` flag cross-wiring | `src/` | running |
| 2 | Repair the corpus detector self-test (`0 of 25` proven → honest N of 25) | `crates/fleet-verify/gates/` | running |
| 3 | Docs truth pass — rewrite every example against the real binary, verified by execution | `docs/` | queued |
| 4 | Help text for all 28 + `--json` consistency | `src/cli/` | queued, blocked on lane 1 |
| 5 | Disclose the 8 stubs in `--help`, or implement them | `src/` | queued |
| 6 | TUI / streaming presentation layer | `src/` + `fleet-stream` | queued |
| 7 | Harness engineering: tool calls, agent spawning, MCP + platform connectors | `crates/` | queued |
| 8 | Independent test+verify: harness, context, LLM-as-judge, observability, continuous learning, recording; load-test on free public datasets | `crates/` + `src/tests/` | queued |

### Lane 4 design decision (already settled — do not re-litigate)

Adding help text as doc comments on the 28 `Commands` variants would push `src/cli/root.rs` from 78
to ~106 lines, breaking the hard ≤80 rule. **Use clap's builder-side augmentation instead**: a
`src/cli/help_text.rs` exposing `fn with_descriptions(cmd: Command) -> Command` that calls
`.mut_subcommand("meter", |c| c.about("…"))` per command, invoked once before `parse`. Zero lines
added to `root.rs`; split the strings across two files if one exceeds 80 lines.

### Lane 3 note

`docs/USING-FLEET.md` was already partly corrected earlier tonight (stale `keel/` paths, a removed
`fleet ratchet` row, a deleted autonomous-loop section). The remaining defect is that its **examples
do not parse**. Every example must be executed and its real output pasted, not hand-written.

## Standing rules that keep being violated in this repo

- **`$?` after a pipe reads the WRONG command.** Violated 6× in the predecessor, twice in zsh where
  `PIPESTATUS` expands EMPTY — and once more by Opus tonight. Redirect to a file, read `$?`.
- **Fail-loud, never fail-safe.** Five instances found tonight: a corpus check that `exit 77`-skipped
  when its source moved; the `recur` gate reading a `memory/lessons` absent from the materialized
  gates root; a SessionStart hook that `exit 0`s and injects nothing if its contract file moves;
  gates that reported success while every check failed; a pipeline stub that returned success.
- **A test seam that substitutes the binary hides missing entry points.** 351 tests were green while
  `fleet __agent` did not exist, because every test swapped in a fake child via
  `FLEET_WORKER_TEST_CHILD_EXE`. Drive `env!("CARGO_BIN_EXE_fleet")` — the real product binary — in at
  least one test per spawn path.
- **A check you never watched fail is not a check.** Reintroduce the defect, watch it fail, restore
  byte-identically (`diff` to prove it), watch it pass.
- **Publish the denominator.** `checked==0` is a failure, never a pass.

## Open items only the owner can close

- `git rm -r -f fleet/docs/pdfs` — 179 files, 34MB, zero PDFs, tracked at `HEAD` under `tmp/`,
  referenced by nothing. Blocked twice by the permission classifier. Recover with
  `git checkout HEAD -- fleet/tmp/pdfs`.
- `Principal Engineering/` **is not a git repository**, so `registry/services/llm-gateway` is tracked
  by nothing and unreachable from any clone. That package also has **no build script**, while its
  `exports` map points at four `.js` files nothing can produce. Phase 0.
- `fleet-rs`: **364 of 570 commits** are an unattended loop committing `verify RED, findings kept,
  code not committed`. Four of seven "active" days contain zero real commits. Six task tags each
  retried ~73× and never gave up.
- `fleet-merge::remove()` returns `Err(NotFound)` for an already-removed worktree, breaking the
  documented idempotence contract. Needs an owner ruling.
- `fleet-memory/src/gate_check.rs:23` leaks via `Box::leak` per distinct category. Bounded by the
  no-daemon tenet today; a real bug the day anything long-running calls it.
- selfcheck's D2 detector false-positives on `_selftest.sh`'s deliberate bad-`mktemp` **fixture**;
  it is a pure line-match and does not honour justification comments.
