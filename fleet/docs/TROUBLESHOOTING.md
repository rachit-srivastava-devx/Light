# Troubleshooting

Run the first command in the matching row before changing state. Preserve its output when escalating.

| Symptom | First command | Cause | Fix |
|---|---|---|---|
| A gate is red | `bash verify.sh` | A prerequisite, input set, or invariant failed; zero-input checks are not valid passes | Read `var/verify.log`, fix the first reported cause, then rerun the same command |
| Ledger is corrupt | `FLEET_STATE="${FLEET_STATE:-$HOME/.local/state/fleet}" fleet ledger verify` | Truncated write, manual edit, or concurrent writer | Stop identified Fleet processes, copy `ledger/chain.jsonl` for evidence, restore the last valid record, then verify again |
| Worker exits 0 with no work | `FLEET_STATE="${FLEET_STATE:-$HOME/.local/state/fleet}" fleet doctor` | Empty input, wrong state directory, or no landed git diff | Confirm the repository has a committed baseline and the worker changed a tracked checkout; a result with `checked=0` is not an attestation |
| Orphaned processes remain | `pgrep -af 'fleet|worker'` | The parent died before reaping a child or a timeout cleanup path was interrupted | Stop only the identified Fleet processes, inspect the ledger, and retry |
| `$FLEET_STATE` is missing | `printf 'FLEET_STATE=%s\n' "${FLEET_STATE:-$HOME/.local/state/fleet}"` | The variable is unset or points at a removed directory | `export FLEET_STATE="$HOME/.local/state/fleet"; mkdir -p "$FLEET_STATE"; fleet doctor` |
| `command not found` after install | `printf '%s\n' "$PATH"` | `~/.local/bin` is not on the current shell's `PATH` | Run the exact export printed by `install.sh`, then start a new shell or source the shell startup file |
| Gatekeeper/quarantine blocks the built binary | `xattr -l "$HOME/.local/bin/fleet"` | macOS attached a quarantine attribute to a locally built or copied binary | Confirm the binary came from this worktree, then run `xattr -d com.apple.quarantine "$HOME/.local/bin/fleet"` and retry |

## Exit codes

`0` is success; `3` is an environment fault; `6` is an invariant violation; `7` is a refusal; `8` is a verification mismatch. A checked/total denominator is part of verification output; `checked=0` is not a valid gate pass.
