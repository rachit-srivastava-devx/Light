# Restore procedure

## At 02:00

1. Stop new fleet launches or isolate the host.
2. Preserve the current `FLEET_STATE` directory; do not edit or remove it.
3. Select a snapshot whose `manifest.json` and `ledger/chain.jsonl` are known-good.
4. Run `fleet state restore --from <backup>`. The command verifies every file checksum and the ledger chain before swapping state; exit `8` means nothing was replaced.
5. Run `fleet state verify` and require non-zero denominators for the chain, artifact, and attestation sweeps.
6. Resume writes only after verification succeeds; retain the pre-restore state and command output for incident review.

If restore fails with exit `8`, quarantine that snapshot and try an older verified snapshot. Never copy individual ledger or attestation files into a live state directory.
