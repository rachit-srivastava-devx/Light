# ADR-0001 — lane-status.v1 is a projected object, not a receipt body

- **Status:** Accepted (2026-09-02)
- **Deciders:** Owner + Independent verifier (adversarial review lane)
- **Supersedes:** the original `contracts/lane-status.v1.json` authors-clause reading of
  `ledger_ref` (which was unreachable to the emitter)
- **Replaces/enables:** `contracts/lane-status.v1.json` rewritten to a projection contract,
  plus a `lane_status` receipt event documented in `contracts/receipt.v1.json`.

## Context

`fleet console` (S2) renders a live `LANES` view that is meant to tail the activity ledger in
real time. The schema `contracts/lane-status.v1.json` declared `ledger_ref` as a **required**
field of the `lane_status` receipt body. But the emitter could never fill it: `append_receipt`
hashes the body before the envelope hash exists, and the lane record knows its own seq/hash only
after the receipt has been appended. An independent adversarial reviewer found **0 of 7** emitted
`lane_status` bodies contained `ledger_ref` — the schema's own requirements were impossible to
satisfy, so every real stream of lane receipts was invalid against its own contract, and the
console's LANES view had nothing valid to project.

A second, deeper defect surfaced during verification: a plain `swarm dispatch --task … --repo …
--agent stub` never emitted a `lane_status` receipt at all (the lane was only defaulted on the
`--role` five-lane path), so the console showed `LANES · 0 tracked` no matter how long it stayed
open. Live-ness of the LANES view was therefore unobservable from the reviewer's standard
reproduction.

## Decision

1. **`lane-status.v1` is a projection, not a receipt body.** Each LANES row is assembled by the
   viewer at render/validation time from two sources:
   - the receipt **body** (authored by the emitter: `schema_version`, `lane_id`, `role`, `state`,
     `agent`), and
   - immutable **envelope stamps** appended by the ledger after this receipt was sequenced
     (`ts_wall`, `actor`, `ledger_ref{seq, hash}`).
   The contract's `required` list names the union — including `ts_wall`, `actor`, `ledger_ref` —
   and the description explicitly states this object is the projected view, not the authored
   body. `fleet contract lane-status validate` re-derives these stamps from the envelope and
   verifies each projected row; a zero-input run (`checked=0`) is a failure (`ExitCode 8`).

2. **The receipt envelope is the immutable source of sequence and hash.** `ledger_ref` is never a
   field a writer fills; it is stamped. This closes the self-reference that the authors-clause
   reading created.

3. **Every dispatch owns a lane.** Even a flagless/`--role`-free `swarm dispatch` defaults to the
   `builder` lane, so any dispatch writes a real `queued → running → passed/failed/refused`
   status stream. `fleet console` tails the ledger (`LedgerTail`, new-byte polling every 250 ms)
   and re-renders the LANES header live — the reviewer's reproduction now shows
   `LANES · 0 tracked` → `LANES · 1 tracked` while the console stays open.

4. **`contracts/receipt.v1.json`** already enumerated `lane_status` in its event enum; no change
   needed there. `fleet contract lane-status validate` enforces the projection against that
   event's receipts.

## Consequences

- **Positive.** The console's LANES view is genuinely live and validated against a schema its
  emitter can satisfy. Any dispatched task leaves a verifiable lane trail. A missing `ledger_ref`
  in a row now means the projection step or validator is broken, not "the emitter forgot".
- **Negative.** Rows rendered before this change that (rightly) lacked envelope stamps are
  invalid against the new projection contract; the validator correctly rejects them rather than
  guessing seq/hash.
- **Risk.** The console tail treats a rewritten/truncated ledger by resetting its read offset; a
  ledger replaced mid-session re-reads from byte 0 (covered by a regression test). Gaps in the
  ledger sequence still show as absence, never as fabricated rows, per the no-fabrication rule.
- **Regain.** This ADR satisfies the contracts governance rule ("never edit `contracts/*.json`
  without an ADR"), which previously blocked any fix.