You are the BUILDER. WRITE CODE NOW. Do not investigate the environment — it is already proven:

  * `keel/Cargo.toml` EXISTS with deps blake3, serde, serde_json, clap(derive), libc, fs2, time.
  * `cd keel && cargo build` ALREADY SUCCEEDS (verified, 47s). crates.io IS reachable. `cargo add` is fine.
  * Do NOT spend turns checking for offline crates, b3sum, flock, or platform details. All resolved.
  * macOS arm64. `b3sum` may be absent — that is fine, the test tolerates it.

YOUR ONLY JOB: make `bash tests/acceptance/p0.sh` print `0 failed`.
Read `tests/acceptance/p0.sh` FIRST — it is the spec. **YOU MAY NOT EDIT IT.**
Read `AGENTS.md` (10 hard rules) and `contracts/*.json` (wire formats).

Write `keel/src/*.rs` — split into modules as you like. Implement exactly these commands:

  fleet run --task <T> --repo <PATH> --agent <stub|env-probe>
  fleet attest verify <ID>
  fleet ledger verify | append --event <E> --body <JSON> | count | dump

Semantics (the test asserts every one of these):
 1. empty/whitespace --task -> write a `refusal` receipt FIRST, then exit 7.
 2. spawn the agent in its own process group (setsid), stdin=/dev/null, env scrubbed of
    FLEET_STATE / ledger / socket paths (allowlist PATH,HOME,LANG only), fd 3 = socketpair(SEQPACKET).
    On drop: SIGTERM the process group, SIGKILL after 5s.
    `stub`      -> appends a line to <repo>/main.rs, writes {"schema_version":"1.0","kind":"done","body":{}} on fd 3.
    `env-probe` -> writes its own env into the submission body.
    (Implement these as internal functions re-invoked via `fleet __agent <name>`; no external binaries.)
 3. FREEZE: `git -C <repo> diff` -> blake3 hex -> write bytes to $FLEET_STATE/artifacts/<hex>, chmod 0444.
    The id MUST equal blake3 of that file's exact bytes. Empty diff + exit 0 -> exit 6 NO_WORK_LANDED.
 4. Write $FLEET_STATE/attestations/<hex>.json as an in-toto Statement:
      _type "https://in-toto.io/Statement/v1"
      subject[0].digest.blake3 == <hex>
      predicateType "https://fleet.local/DeliveryAttestation/v1"
      predicate.elements MUST contain key "oracle_independence"
 5. print a line containing `artifact=<hex>`
 6. `attest verify <id>`: re-hash the artifact, compare to subject digest; mismatch -> exit 8; ok -> 0.
 7. LEDGER at $FLEET_STATE/ledger/chain.jsonl, JSONL, per contracts/receipt.v1.json.
    hash = blake3(prev_hash + canonical_json(row minus "hash")); first prev_hash = "GENESIS".
    ts_wall and actor are stamped BY YOU, never accepted from input.
    ** 20 CONCURRENT `fleet ledger append` PROCESSES must yield: no rows lost, chain verifies, and
       NO TWO ROWS SHARING prev_hash. Hold an exclusive fs2 file lock across the ENTIRE
       read-last-hash -> compute -> append sequence. Reading last_hash outside the lock is THE bug. **
    `count` prints an integer. `dump` prints the raw JSONL.

Create $FLEET_STATE subdirs as needed. Never panic on a missing dir — create it.

ITERATE: run `bash tests/acceptance/p0.sh` yourself, read failures, fix, repeat until 0 failed.
Then paste the final real output. If you believe an assertion is wrong, DO NOT edit it —
append your objection to docs/objections/<your-name>.md (NEVER docs/DELTA.md — lead-owned) and make the rest pass.

## IMPORTANT — file creation
`keel/src/main.rs` DOES NOT EXIST. Create it fresh, plus any modules you want
(`keel/src/ledger.rs`, `keel/src/run.rs`, etc.). Do NOT emit a patch that deletes and recreates the
same path in one operation — create new files directly. A previous run corrupted itself that way.
