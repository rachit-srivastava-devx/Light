# Per-repo gate commands — `.fleet/gates.toml`

`fleet run`, `fleet gate` and `fleet oracle` run the committed gate table
(`crates/fleet-verify/src/registry.rs`). Its defaults are Rust defaults: the unit-tests gate
shells out to `cargo test --workspace`. On a repo that is not a cargo workspace that gate probed
`cargo`, found nothing applicable, and printed

```
SKIP gate unit tests -- unit tests unavailable
```

which is a gate that examined zero inputs. A repo says what its gate commands **are** by dropping
a `.fleet/gates.toml` next to the code.

## Location

`<repo>/.fleet/gates.toml` — the same `.fleet/` directory a repo already uses for
`agents.toml` and `skills.toml`. Discovered from the `--repo` you pass, never from the `fleet`
process's own cwd.

**No file means no change.** With no `.fleet/gates.toml` the table is byte-identical to the
committed one, so nothing about an existing Rust repo moves.

## Shape

One table per gate, keyed by the gate's **registry id** (the id printed in `PASS gate <id>`):

```toml
# A Node/TypeScript repo: unit tests come from npm, not cargo.
[gates."unit tests"]
command = ["npm", "run", "test:unit"]
probe   = "npm"
```

| Key | Required | Meaning |
|---|---|---|
| `command` | yes | argv, executed with the repo as its cwd. Must be non-empty. |
| `probe` | no | the tool that must resolve before the gate runs. Defaults to the gate's committed probe — override it whenever the command changes toolchain (`cargo` → `npm`). |

Every current id: `"unit tests"`, `"mutants"`, `"semgrep"`, `"trivy"`, `"recur"`, `"detectors"`,
`"policy"`, `"corpus"`.

## What a repo may not do

A repo may **replace a known gate's command**. It may not add a gate, delete a gate, or downgrade
one from `Required` to `Advisory`. There is deliberately no `enabled = false`:
[AGENTS.md](../AGENTS.md) rule 6 is that a check which examined zero inputs fails, and an opt-out
switch is exactly the check that is cheaper to fake than to satisfy. Saying *what your unit-test
command is* is supported; saying *you have no unit tests* is not.

Two things are hard errors (exit **3**, environment fault) rather than silent no-ops:

* an id that is not in the registry — a typo like `[gates."unit-tests"]` must not quietly leave
  the gate on `cargo test`;
* an unparseable file, an unknown key, or an empty `command`.

## The denominator still has to be published

A gate's verdict is `{checked, total}`, parsed from its own stdout by that gate's registry parser
(`crates/fleet-verify/src/parsers.rs`). The unit-tests parser reads libtest's own line:

```
test result: ok. 42 passed; 0 failed; 0 ignored
```

So a replacement command has to end up printing a line the parser recognises — `jest` and
`vitest` reporters can be pointed at that format, or wrap the runner in a small script that
prints it. A command that exits 0 while publishing nothing is `FAIL … Unparseable`, never a pass.

## Checking it

```sh
fleet gate --id "unit tests" --repo /path/to/node-repo
fleet run --repo /path/to/node-repo --task "..." --json | jq '.gates'
```

`--json` carries every gate's `id`, `verdict`, `checked`, `total`, `detail` and `env_fault`, so
CI can assert on the table rather than scraping the human output.
