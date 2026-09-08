# State directory missing

> **NOT YET IMPLEMENTED — do not follow this runbook during an incident.**
> This procedure calls `fleet state verify|restore|gc`. That subcommand family does not
> exist: `grep -c State src/cli/root.rs` returns `0`. Every `fleet state ...` step below
> will fail with an unrecognized-subcommand error. The procedure is retained as the
> intended design, not as operable guidance. Until `fleet state` ships, recover manually
> and verify the chain with `fleet ledger --verify` (bare `fleet gate` with no `--id` hangs indefinitely as of 2026-09-08 -- use `fleet gate --id <id>`).

## At 02:00

Confirm `FLEET_STATE_DIR` is set to the intended absolute path and that the service account can create and write its `ledger`, `artifacts`, and `attestations` directories. Do not point it at a new empty directory to bypass an integrity failure.

If the original directory was deleted, stop writes, restore from the newest verified snapshot with `fleet state restore --from <backup>`, then run `fleet state verify`. A missing or empty state is not a successful verification; escalate if no verified snapshot exists.

Record the configured path, ownership/mode, disk status, restore manifest, and final verification output.
