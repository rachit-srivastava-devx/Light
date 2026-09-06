# fleet

> **What it is built on:** [`docs/DEPENDENCIES.md`](docs/DEPENDENCIES.md) — every crate,
> package, binary and lane, what each is for, and which adopted tools were withdrawn.
>
> **New here? Read [`docs/USING-FLEET.md`](docs/USING-FLEET.md)** — every command, a real
> end-to-end walkthrough with actual output, and the six exit codes that are the whole contract.

fleet is a local macOS CLI for running a worker, freezing its git diff, and recording a receipt-backed attestation. It keeps state under `$FLEET_STATE` (default `~/.local/state/fleet`) and uses the user's own local CLI credentials. It is tamper-evident and single-user: there is no server, daemon, database, or cloud service.

## Install

macOS only. From this worktree:

```sh
./install.sh
```

The installer probes `cargo`, `rustc`, and `git` with `--version`, builds a release binary, installs it to `~/.local/bin`, initializes `$FLEET_STATE` with mode `700`, and prints exact undo instructions. Use `./install.sh --check` for a no-change preflight or `./install.sh --uninstall` to remove the binary and generated zsh completion while preserving receipts.

## 60-second quickstart

After installation, this is a copy-pasteable run that creates real work in a disposable git repository and ends by verifying the resulting attestation:

```sh
set -eu
demo="$(mktemp -d)"
trap 'rm -rf "$demo"' EXIT
cd "$demo"
git init -q
git config user.email fleet@example.invalid
git config user.name fleet-demo
printf 'before\n' > main.rs
git add main.rs && git commit -qm initial

# fleet plans before it executes: `run` refuses a task with no accepted SOW.
# The SOW needs atomic leaves, cited challenges and two named alternatives.
task='make the demo change
leaves:
- change main.rs and exit 0 | acceptance: the diff is non-empty and applies
challenges:
- the change may collide with existing content | citation: A1
alternatives:
- append: add a line at the end | tradeoff: no context awareness
- rewrite: replace the file | tradeoff: discards existing content
estimates:
- 5 minutes
edge cases:
- the file is empty'

# `sow` exits 9 (SOW_READY_AWAITING_REVIEW) and prints the id on stderr.
sow="$(fleet sow --task "$task" 2>&1 >/dev/null | grep -oE 'id=[0-9a-f]+' | cut -d= -f2 | head -1)"
fleet sow accept --id "$sow"

# The final stdout line is the bare `artifact=<id>`; take that one, not the progress lines.
artifact="$(fleet run --task "$task" --repo "$demo" --agent stub | grep -oE '^artifact=[0-9a-f]{64}$' | tail -1 | cut -d= -f2)"
fleet ledger verify
fleet attest verify "$artifact"
fleet status
```

To skip the planning gate in scripts and CI, set `FLEET_SOW_BYPASS=1` — it is recorded in the
receipt, so a run without a plan stays distinguishable from one with a plan.


The final line must print `verified artifact=<64-character-id>`.

## Commands

Run `fleet help` for the full command list and runnable examples. Shell completion is generated with `fleet completions zsh`, `fleet completions bash`, or `fleet completions fish`.

## Exit codes

| Code | Meaning |
|---:|---|
| 0 | Success |
| 3 | Environment fault: missing tool, state, or usable local resource |
| 6 | Invariant violation: the local workflow could not safely proceed |
| 7 | Refusal: ambiguous, empty, or unsupported input |
| 8 | Verification mismatch: evidence does not match its claim |

## Limitations

The P0 attestation is STRUCTURE, NOT TRUST until the O2 holdout oracle lands. A repository outcome is not a user outcome; the operator is the root of trust. The design provides SLSA L3 non-forgeability against the WORKER and L1 against the OPERATOR. Receipts and artifacts are tamper-evident, not tamper-proof. P0 is local to one developer account and one Mac; it does not provide multi-user authorization, remote durability, or independent oracle verification.

## Troubleshooting

See [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) for symptom-first recovery commands.
