# Quickstart

Executed top to bottom from a clean `/tmp` scratch dir on 2026-09-08 against
`target/debug/fleet`. Every command below and its output is real — see
`docs/USING-FLEET.md` for the full command reference and the defects found along the way, and
`docs/DX-AUDIT.md` for the adversarial sweep this is built on.

**Honest framing:** the full "plan a task, run it, get an attestation" workflow the product aims
for does not exist end to end today (3 of 28 commands needed for it — `adjudicate`, `attest`,
`pr` — are unimplemented stubs, and no verified link exists between an accepted SOW and `run`
proceeding). This is the shortest path to *real, working* output, not a tour of a finished product.

## 1. Build and point at a scratch state dir

```bash
git clone <this-repo> && cd fleet
cargo build
export FLEET_STATE_DIR=/tmp/fleet-quickstart-state   # NOT "FLEET_STATE" -- see USING-FLEET.md
mkdir -p "$FLEET_STATE_DIR"
export PATH="$PWD/target/debug:$PATH"
```

## 2. Confirm the environment

```
$ fleet doctor
cargo: found
git: found
...
decision: allow (concurrency_cap=1)
EXIT:0
```

## 3. Make a scratch git repo to point commands at

`fleet` refuses to act on a non-repo path, and several commands (`run`, `swarm`, `agents`,
`rollback`) need one:

```bash
mkdir -p /tmp/fleet-quickstart-repo && cd /tmp/fleet-quickstart-repo
git init -q && git commit --allow-empty -q -m init
```

## 4. Run a task

```
$ fleet run --repo /tmp/fleet-quickstart-repo --task "add a version flag" < /dev/null
fleet: run: stage Event starting
...
fleet: verify: running `cargo test --workspace` (budget ...)
...
fleet: Verify("gate(s) failed: unit tests: NonZeroExit(101); mutants: NonZeroExit(1); ...")
EXIT:7
```
Exit 7, refused — `run` executes the *target repo's own* verify gates
(`cargo test`, `cargo mutants`, `semgrep`, ...), and an empty scratch repo has no `Cargo.toml`, so
those gates fail honestly. That is the correct, expected outcome for a throwaway repo, not a bug.
Give it real up-to-40s of budget: `docs/DX-AUDIT.md` recorded this same command hanging
indefinitely; retested for this rewrite it now completes in ~8s, but treat anything under
`timeout 60` as the safety margin, not `timeout 15`.

## 5. Check what got recorded

```
$ fleet ledger
rows: 1
EXIT:0
$ fleet ledger --verify
verified: 1/1
EXIT:0
```
Not `fleet ledger verify` (that's a clap error, exit 2) — the real flag is `--verify`.
`fleet status` will **not** show this task; it reports host capacity, not a task rollup (a known,
undisclosed defect — see `USING-FLEET.md`).

## 6. Where it stops

- `fleet sow`/`fleet plan` work (see `USING-FLEET.md` §3-4 for the real, undocumented-in-`--help`
  vocabulary each expects) but nothing observed here links a SOW to `run` proceeding or refusing.
- `fleet adjudicate`, `fleet attest`, `fleet pr` are unimplemented stubs (exit 3, every input).
  There is no way to get from the receipt above to an "attested" artifact today.
- `fleet meter` and `fleet agents` need one-time setup (a seeded `meter.json` row, or a valid
  built-in agent id) not mentioned in `--help` — see `USING-FLEET.md` for verified recipes.

That is the whole honest shortest path: build, point at state, run one task against a scratch
repo, see it refuse for a real reason, confirm the refusal landed in the ledger. Everything past
that point in the product's aim (dispatch, adjudicate, attest, PR) is either unverified or
unbuilt as of this writing.
