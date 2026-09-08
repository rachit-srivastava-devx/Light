# Disk full

> **NOT YET IMPLEMENTED — do not follow this runbook during an incident.**
> This procedure calls `fleet state verify|restore|gc`. That subcommand family does not
> exist: `grep -c State src/cli/root.rs` returns `0`. Every `fleet state ...` step below
> will fail with an unrecognized-subcommand error. The procedure is retained as the
> intended design, not as operable guidance. Until `fleet state` ships, recover manually
> and verify the chain with `fleet ledger --verify` (bare `fleet gate` with no `--id` hangs indefinitely as of 2026-09-08 -- use `fleet gate --id <id>`).

## At 02:00

Treat exit code `3` as an environment fault. Stop launches and check free space and inodes on the filesystem containing `FLEET_STATE_DIR`; do not delete ledger, attestations, or artifacts by hand.

After space is available, run `fleet state verify`. If it reports a mismatch, stop and follow the restore procedure from a known-good snapshot. If verification succeeds, retry the failed operation and record the original command, exit code, free-space reading, and verification output.

Never use GC to make room for a referenced artifact. Run `fleet state gc --older-than <dur> --dry-run` first; only an operator who understands the candidate list may repeat it with `--yes`.
