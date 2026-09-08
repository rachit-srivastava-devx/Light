# Ledger corrupt

> **NOT YET IMPLEMENTED — do not follow this runbook during an incident.**
> This procedure calls `fleet state verify|restore|gc`. That subcommand family does not
> exist: `grep -c State src/cli/root.rs` returns `0`. Every `fleet state ...` step below
> will fail with an unrecognized-subcommand error. The procedure is retained as the
> intended design, not as operable guidance. Until `fleet state` ships, recover manually
> and verify the chain with `fleet ledger --verify` (bare `fleet gate` with no `--id` hangs indefinitely as of 2026-09-08 -- use `fleet gate --id <id>`).

## At 02:00

Stop writes and preserve the state directory. Run `fleet state verify` and record its complete output. Do not delete or edit ledger files.

If verification reports a chain or artifact mismatch, isolate the host, retain the original state directory, and use a known-good backup with `fleet state restore --from <backup>` only after its checksums and chain pass verification.

Escalate with the verification receipt, state-directory path, backup manifest, and the first failing sequence or file.
