# Disk full

## At 02:00

Treat exit code `3` as an environment fault. Stop launches and check free space and inodes on the filesystem containing `FLEET_STATE`; do not delete ledger, attestations, or artifacts by hand.

After space is available, run `fleet state verify`. If it reports a mismatch, stop and follow the restore procedure from a known-good snapshot. If verification succeeds, retry the failed operation and record the original command, exit code, free-space reading, and verification output.

Never use GC to make room for a referenced artifact. Run `fleet state gc --older-than <dur> --dry-run` first; only an operator who understands the candidate list may repeat it with `--yes`.
