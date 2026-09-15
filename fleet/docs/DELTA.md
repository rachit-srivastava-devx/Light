# Implementation deltas from LLD

## 2026-09-15 — ingest remains partial

`docs/blueprints-next/ingest/BLUEPRINT.md` specifies a durable inbox/store
transaction, refusal receipts, authenticated source metadata, cursor-after-
commit, and a `100_000` string-leaf bound. The current `crates/ingest` crate
implements normalization plus a strict authenticated path and a store-backed
`SqlDurableInbox` slice. The slice
atomically persists scrubbed payloads, taint, redaction metadata, attachment
references, authenticated actor metadata, deduplication outcomes, and refusal
rows, and restart tests prove the rows survive reopening SQLite. The cursor is
persisted transactionally and read back only after commit. The public
legacy `normalize` helper remains compatible with authored acceptance tests, but
durable acceptance rejects events produced by it; production callers use
`normalize_authenticated` or `ingest`, whose normalization failures write a
refusal first. Cursor and version positions are now bounded; late cursors do not
regress checkpoints, and stale object versions are retained but marked
non-applicable. The local CLI task path now enters strict authenticated ingest
before `RunStart`. Canonical connector-envelope validation, payload-reference
persistence, an idempotent ingest migration ledger, SQLite busy-timeout
handling, and a taint-bearing metadata-only control reference now have
executable proof. It is still not production-complete: provider-specific
signature/scope adapters and the downstream control consumer that enforces
`injection_taint` are separate node responsibilities and remain unwired, while
shared repository gates have unrelated baseline failures. This is an
implementation boundary, not an LLD correction. Node `ingest` remains
`in-progress` until the required authority decision for that cross-node edge is
recorded; no other node is being started in this pass.

The implementation retains `MAX_JSON_LEAVES = 10_000` although the draft LLD
states `100_000`: the authored `json_too_complex_refuses` acceptance test
requires refusal at the lower bound. The stricter cap is safer for local
resource protection; changing the LLD or acceptance contract requires an ADR.

`dev.sh` now treats terminal clearing as optional because `cargo watch --clear`
fails in environments without terminfo. The existing shell test still asserts
that bare mode must not start a watcher, which conflicts with the current
documented background-warm behavior. The test failure is recorded and the
acceptance test was not modified.

## 2026-09-15 — verify remains an explicit implementation delta

`docs/blueprints-next/verify/BLUEPRINT.md` describes a canonical
`verify::verify` API with a real secret scanner and exact candidate authority.
The live CLI and pipeline now enter that API through the production adapter,
preserving the resolved gate list, parser, probe, timeout, and typed exit
semantics. Candidate validation, BLAKE3 integrity evidence, content-bound
no-git tree hashing, acceptance hashing of resolved gate assets, and a real
`GitleaksFindingsProvider` are implemented. Remaining deltas are trusted
configuration provenance, legacy digest migration, and final executable proof
of the CLI receipt/no-HEAD pipeline paths. The durable secret-scan
denominator/receipt contract and scanner wait-path fix are implemented; current
test execution is blocked by the host's unaccepted Xcode license, not by a Rust
compile error. These remain open Node `verify` completion items, not changes to
the LLD authority.
