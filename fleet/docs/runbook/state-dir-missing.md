# State directory missing

## At 02:00

Confirm `FLEET_STATE` is set to the intended absolute path and that the service account can create and write its `ledger`, `artifacts`, and `attestations` directories. Do not point it at a new empty directory to bypass an integrity failure.

If the original directory was deleted, stop writes, restore from the newest verified snapshot with `fleet state restore --from <backup>`, then run `fleet state verify`. A missing or empty state is not a successful verification; escalate if no verified snapshot exists.

Record the configured path, ownership/mode, disk status, restore manifest, and final verification output.
