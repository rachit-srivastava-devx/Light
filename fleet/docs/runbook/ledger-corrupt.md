# Ledger corrupt

## At 02:00

Stop writes and preserve the state directory. Run `fleet state verify` and record its complete output. Do not delete or edit ledger files.

If verification reports a chain or artifact mismatch, isolate the host, retain the original state directory, and use a known-good backup with `fleet state restore --from <backup>` only after its checksums and chain pass verification.

Escalate with the verification receipt, state-directory path, backup manifest, and the first failing sequence or file.
