# BLUEPRINT: `<crate-name>`

> Copy this file to `docs/blueprints/<crate-name>/BLUEPRINT.md` before writing any code for a new
> crate. This is fleet's own per-crate build spec (AGENTS.md's "docs/blueprints/<crate>/
> BLUEPRINT.md" reference) — not the product/business deep-dive format in the external
> `blueprints/` repo (read-only there; those are L8 *examples* to learn rigor from, per the
> owner's standing instruction, never templates to copy verbatim). What's borrowed from that
> rigor: no naked claims, an adversarial pass before calling anything done, killed alternatives
> named, a real failure story instead of "we handle errors."

## 1. What this crate owns, and what it explicitly does not

<!-- One paragraph. Name the boundary crate this touches on either side (who calls it, what it
     calls). A crate whose boundary can't be stated in one paragraph is scoped too broadly. -->

## 2. Reuse decision (C1/L2 — record before writing a line)

- [ ] Searched the workspace (`grep`/`search_graph`) for an existing crate/module already owning
      this capability.
- [ ] Searched the stdlib and already-vendored dependencies (`Cargo.lock`) before adding a new one.
- Decision: **install** / **extract** / **build-new** — state which, and why the other two were
  rejected.

## 3. Contract (write this before any implementation)

The public interface (types + function signatures), and the failing acceptance test(s) that pin
it. Real code, not prose:

```rust
// interface sketch
```

- [ ] Illegal states are unrepresentable (no `Option<T>` standing in for "hasn't happened yet" next
      to a separate bool that could disagree with it).
- [ ] Clock/RNG/IO injected via a port trait — nothing in the pure logic reads `SystemTime::now()`
      or touches a filesystem/network directly.
- [ ] Money/counts/hashes are integers or fixed strings, never floats.

## 4. The adversarial pass (handle what §3 didn't enumerate)

Walk every input through: **empty · null · wrong type · huge · negative · duplicate · concurrent ·
unicode · already-exists · partial failure.** For each one that applies to this crate's actual
inputs, name the behavior (typed error, or a specific handled case) — not "shouldn't happen."

| Case | Behavior |
|---|---|
| empty | |
| concurrent | |
| partial failure | |

## 5. Killed alternatives

At least one real alternative design considered and rejected, with the actual reason (cost,
complexity, a property it couldn't provide) — not "we chose the obvious approach."

## 6. Failure story

How does this crate actually break in production, and what does the caller see when it does? A
crate with no answer here hasn't been thought through past the happy path.

## 7. Definition of done

- [ ] Unit tests for the pure logic (inject fakes for every port).
- [ ] Integration test(s) driving the real adapter, not the fake, for at least the one path a
      human or another crate actually calls.
- [ ] Mutation testing (`cargo mutants -p <crate>`, scoped to changed files) where the crate's own
      dependency graph makes it cheap; if it isn't (a binary crate with many path deps), say so
      explicitly instead of skipping silently — see `docs/LLD/LLD-META-L8-ADDENDUM-4.md` for a
      worked example of naming that cost rather than hiding it.
- [ ] `cargo clippy -p <crate> --all-targets --no-deps -- -D warnings` clean on every file this
      blueprint's work touched (pre-existing failures elsewhere are not this blueprint's to fix —
      name them, don't absorb them).
- [ ] No new file over ~80 lines without a one-line reason.
- [ ] Registry entry updated if this crate is new (`registry/services/REGISTRY.md` or
      `registry/features/REGISTRY.md`, per the layer).

## 8. Evidence

Paste the real command + real output for every checked box above, red included. A checked box
with no pasted command is a claim, not a measurement.
