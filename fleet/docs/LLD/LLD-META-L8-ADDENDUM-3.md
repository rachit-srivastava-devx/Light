# Fleet LLD Meta-L8 Addendum 3 — OS-correctness closure

Date: 2026-09-13
Status: design closure draft; implementation and production proof are still absent. Builds on
[`LLD-META-L8-ADDENDUM.md`](LLD-META-L8-ADDENDUM.md) (A-H) and
[`LLD-META-L8-ADDENDUM-2.md`](LLD-META-L8-ADDENDUM-2.md) (I-P); does not reopen either.

A review comparing this design's concurrency and recovery mechanisms against standard operating-
systems and database correctness literature (ARIES write-ahead logging, seL4's formal-verification
model, backpressure/flow-control design, and the Coffman conditions for deadlock) found that
several places state a *mechanism* with confidence but never state the *correctness property* the
mechanism is supposed to guarantee — and one place where two different, individually-sound
mechanisms are used without checking whether they compose. Five items, closed below.

## Q. Lease expansion must not reintroduce hold-and-wait — owner: `control`

§8 avoids deadlock by acquiring every resource for a lease atomically upfront (tokens, RAM, CPU,
disk, worktree, in one transaction) before launch — this eliminates hold-and-wait, one of the four
Coffman conditions, by construction. But §8's "dynamic discovery outside the lease stops the worker
for lease expansion and re-evaluation" describes a worker that already holds its original
reservation, blocking for more — which *is* hold-and-wait, reintroduced through the one path the
atomic-upfront design didn't cover.

```rust
struct LeaseExpansionRequest { lease_id: String, additional_resources: ResourceDelta, discovered_at: String }
enum ExpansionOutcome { Granted { new_lease_id: String, new_generation: u64 }, Denied { reason: String }, Deferred }
```

Expansion is never "hold current + wait for more." On discovery, the worker's current reservation
is released back to the scheduler in the same transaction that requests the enlarged one — an
atomic *replace*, not an atomic *add*. If the enlarged reservation cannot be granted immediately,
the outcome is `Denied` (the worker is stopped and the node returns to `READY` to re-enter normal
admission with the corrected resource estimate) or `Deferred` under the same wait-queue discipline
as first admission — never a worker parked holding partial resources indefinitely. This makes lease
expansion use the *same* deadlock-avoidance discipline as initial admission, rather than a second,
uninspected one.

**Composition with the multi-repo saga (Addendum 1-C):** that saga uses a *different* proven
technique — sorted repo-ID lock ordering, breaking circular-wait instead of hold-and-wait. A run
that holds both a single-repo lease (atomic-upfront) and participates in a multi-repo `ChangeSet`
(ordered locking) never acquires the changeset's repo locks *while* holding an expansion-pending
lease state, and vice versa — the two disciplines are composed by forbidding a lease from being in
`Deferred` expansion state and a saga participant simultaneously, not by proving the general case
safe.

Required tests: an expansion request never leaves the prior reservation held past the same
transaction; a denied expansion returns the node to `READY`, never a stuck `RUNNING`; a lease in
`Deferred` expansion cannot be admitted into a `ChangeSet`'s lock-acquisition order; a generated
adversarial interleaving of expansion requests and saga lock acquisition never deadlocks (bounded
random interleaving, not just the one scenario named above).

## R. Idempotent event replay — owner: `store` (extends §5's transaction pattern)

§5's transaction pattern ("append event → update projection → ... → commit") and N08's "checkpoint
+ bounded replay" state a latency target for recovery, never the correctness property recovery
actually depends on: that replaying the event log — from a checkpoint, from the start, twice, or
interrupted midway and rerun — converges to the *same* projection state every time. This is exactly
what ARIES's LSN-comparison redo rule provides and what is currently unstated here.

```rust
struct ProjectionApply { entity_key: String, applied_seq: u64 }
```

Every projection row carries the `seq` of the last event it applied. Replay reapplies events in
`seq` order and, for each, compares the event's `seq` against the target row's current
`applied_seq`: skip if `event.seq <= applied_seq` (already applied), apply and advance
`applied_seq` otherwise. This is checked per entity, not globally, so two projections at different
replay progress during a partial crash remain individually correct. Recovery's 30-second SLA (N08)
is a performance target *on top of* this; it is not the mechanism that makes replay safe.

Required tests: replaying the full event log twice in a row produces byte-identical projection
state; replaying from an arbitrary interrupted midpoint produces the same end state as replaying
from the start; an out-of-order redelivery of an already-applied event is a no-op, not a duplicate
effect.

## S. fd-3 channel overflow policy — owner: `fleet-worker`

§5 states the bound ("maximum 1 MiB/frame, 8 MiB queued per worker") but never the policy for what
happens at that bound — block, drop, or error are three different correctness properties (per
TCP's receive-window and the LMAX Disruptor's producer-wait design), and leaving the choice
unstated means it is whatever the current code happens to do.

```rust
enum QueueFullPolicy { BlockProducer, ProtocolFault }
```

The policy is `BlockProducer`, never silent drop: a worker whose observation channel is at the
8 MiB cap blocks on its next write until the parent drains it, the same backpressure contract as a
full TCP window. A worker that cannot tolerate blocking (a native CLI with its own internal
buffering) and overflows anyway produces a `ProtocolFault` receipt (§5's existing protocol-fault
category for sequence gaps and duplicate terminal frames covers this uniformly) — never a silently
dropped frame, which would corrupt the frame-sequence invariant §5 already relies on elsewhere.

Required tests: a worker producing faster than the parent drains observably blocks rather than
loses frames; a fault-injected worker that cannot block produces a `ProtocolFault` receipt with the
exact sequence number where the gap occurred; no test path ever shows a frame silently absent from
the observation stream with no corresponding fault record.

## T. State-machine safety/liveness must be model-checked, not example-tested — owner: `control`

Addendum 1-D's 16-state run lifecycle currently specifies correctness as "one acceptance test for
every legal row" — this tests that enumerated transitions behave as documented, but proves nothing
about the *reachable state space* those transitions generate together. Testing every edge of a
graph is not the same claim as proving a property holds at every node reachable by any path through
it; this is the distinction that separates ordinary test suites from formally verified systems
(seL4's proof is over all reachable kernel states, not a sampled transition list).

Named properties this addendum requires checked, at minimum, by an exhaustive or bounded-model-
checked search over the reachable state space (a lightweight explicit-state search over the finite
state/event alphabet in Addendum 1-D is sufficient; a full TLA+ specification is not required but
may be used):

- **Safety:** no reachable state has two active leases holding overlapping write-sets (the
  Conflict predicate in §8 must hold as a global invariant, not just a per-admission check).
- **Safety:** `COMPLETED`, `FAILED`, `CANCELLED` have no outgoing transitions in the reachable graph.
- **Liveness:** every path from `PAUSING` reaches `PAUSED` or is preempted by `CANCELLED` — no path
  loops in `PAUSING` indefinitely.
- **Liveness:** `WAITING_QUOTA` does not hot-loop (Addendum 1's own requirement) — checked as a
  reachability property (no cycle through `WAITING_QUOTA` with zero elapsed wait), not asserted by
  prose alone.

Required tests: the model-checked properties above are checked as an automated build step against
the transition table, not a one-time manual proof; any change to Addendum 1-D's table re-runs the
check before merge; the per-row acceptance tests remain required in addition, not instead.

## U. Event schema version-skew across live consumers — owner: `fleet-events`, `store`

N15 covers the store reading old event rows across one schema migration; it says nothing about
`route`, `control`, and `offline` — the live consumers of the event stream — encountering a mix of
schema versions *during* a rolling deployment, when some processes have migrated and some have not.

```rust
struct ConsumerCompatibility { consumer: String, min_schema_version: u64, max_schema_version: u64 }
```

Every consumer declares the schema-version range it can read. A migration that would produce an
event outside any currently-registered consumer's range is refused (a typed refusal, not a crash or
silent misparse) until all consumers have migrated; this is checked at the same admission boundary
N15's migration test already exercises, extended from "the store can still read old rows" to "every
live reader can still read the events actually being produced right now."

Required tests: a migration is refused while any registered consumer's range excludes the new
schema version; a consumer reading an event outside its declared range fails closed with a typed
refusal, never a best-effort partial parse; the compatibility check runs before the migration
commits, not after.

## Closure gate

Same terms as Addenda 1 and 2: normative once a blueprint links these contracts, `EDGE-TYPES.md`
names the new payloads (`LeaseExpansionRequest`, `ExpansionOutcome`, `ProjectionApply`,
`QueueFullPolicy`, `ConsumerCompatibility`), and a different-model review finds no unresolved P0/P1
contradiction. Until then, LLD.md plus Addenda 1 and 2 remain the recoverable baseline.
