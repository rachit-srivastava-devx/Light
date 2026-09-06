# ADR 0016 — Promote `SessionMeter` reservation semantics upstream

**Status:** recommended; registry implementation pending. **Date:** 2026-08-28.
**Registry decision:** extract the working primitive; do not replace it yet.

## Context

`@pe/cost-control-plane` exposes monthly `record()`/`getLedger()`/`isWithinBudget()`. That is
post-hoc accounting and a racy boolean check, not the product's atomic pre-spend invariant.
`backend/relay-py/src/orb_relay/cost/meter.py` is load-bearing and already implements
`reserve_remaining()` -> `settle()`/`release()` with tenant, user, and session identity.

Adding three interface methods without atomic adapters would make the registry claim stronger than
the code again. Deleting `meter.py` first would remove the only working guard.

## Decision

Promote the algorithm and contract upstream, then migrate the relay only after the registry's T0
memory adapter and durable adapter pass the same concurrency/idempotency suite:

- `reserve`: atomic check-and-hold keyed by tenant/user/session plus idempotency key; returns an
  opaque reservation ID and held integer-paise amount.
- `settle`: exactly-once conversion of a hold to actual attributed spend; actual <= held; releases
  the unused balance.
- `release`: idempotent return of an unsettled hold.
- all money crosses JSON boundaries as integer-paise decimal strings; no float currency.
- concurrent reservations cannot oversubscribe either the session cap or tenant ceiling; expiry
  has an explicit clock and audit event rather than relying on process death.

Keep `SessionMeter` in place until the upstream port, memory adapter, durable adapter, contract tests,
and relay composition exist. The extraction trigger has already fired conceptually (C2: cost
reservation is not product-specific); the operational migration gate is a passing atomic contract
suite plus a real relay caller.
