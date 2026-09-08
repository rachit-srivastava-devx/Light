# Restore procedure

> **NOT YET IMPLEMENTED — do not follow this runbook during an incident.**
> This procedure calls `fleet state verify|restore|gc`. That subcommand family does not
> exist: `grep -c State src/cli/root.rs` returns `0`. Every `fleet state ...` step below
> will fail with an unrecognized-subcommand error. The procedure is retained as the
> intended design, not as operable guidance. Until `fleet state` ships, recover manually
> and verify the chain with `fleet ledger --verify` (bare `fleet gate` with no `--id` hangs indefinitely as of 2026-09-08 -- use `fleet gate --id <id>`).

## At 02:00

1. Stop new fleet launches or isolate the host.
2. Preserve the current `FLEET_STATE_DIR` directory; do not edit or remove it.
3. Select a snapshot whose `manifest.json` and `ledger/chain.jsonl` are known-good.
4. Run `fleet state restore --from <backup>`. The command verifies every file checksum and the ledger chain before swapping state; exit `8` means nothing was replaced.
5. Run `fleet state verify` and require non-zero denominators for the chain, artifact, and attestation sweeps.
6. Resume writes only after verification succeeds; retain the pre-restore state and command output for incident review.

If restore fails with exit `8`, quarantine that snapshot and try an older verified snapshot. Never copy individual ledger or attestation files into a live state directory.
