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

**Use [`docs/QUICKSTART.md`](docs/QUICKSTART.md), not the snippet below.** The flags and
subcommand shapes drift as the CLI evolves, and a quickstart that isn't re-verified against a real
build goes stale silently — `docs/QUICKSTART.md` and [`docs/USING-FLEET.md`](docs/USING-FLEET.md)
are re-run against a real `target/debug/fleet` and updated when they diverge; this file is not.
(A previous revision of this section showed `fleet sow --task ... | sow accept --id`, `fleet run
--agent stub`, `fleet ledger verify`, and `fleet attest verify <id>` — none of that shape exists
in the current CLI; see `docs/QUICKSTART.md` for the real one.)

## Commands

Run `fleet help` for the full command list and runnable examples. Shell completion is generated with `fleet completions zsh`, `fleet completions bash`, or `fleet completions fish`.

To gate a repo that is not a cargo workspace, give it a `.fleet/gates.toml` naming its own gate
commands (a Node repo maps the unit-tests gate to `npm run test:unit`) — see
[docs/GATES-CONFIG.md](docs/GATES-CONFIG.md). With no such file the committed Rust defaults are
used unchanged.

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
