# BLUEPRINT — `<crate-name>`

> **Fill every `<...>` and every `> guidance` line, then delete the guidance lines.** The bar:
> a competent human OR a 4B model must be able to implement this crate correctly from this file
> alone, with no other context — no re-reading fleet source, no asking a follow-up question. If a
> reader would have to guess, the blueprint is not done. See `fleet-router/BLUEPRINT.md`
> for a fully-worked exemplar of this exact template.
>
> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `<crate-name>`
- **One-line purpose:** `<what this crate does, one sentence, no marketing words>`
- **Build branch:** `<extract | refactor | partial | build-new>` — from MIGRATION-PLAN §3.
  > `extract` = lift working code near-as-is. `refactor` = lift + reshape out of a god-file.
  > `partial` = some exists in fleet, finish the rest. `build-new` = absent in fleet, greenfield.
- **Imports (this crate depends on):** `<crate names only, e.g. fleet-types>` — or `none`.
- **Imported by (depends on this crate):** `<crate names only>` — or `none yet`.
  > Both edge lists must be consistent with the one-directional DAG in MIGRATION-PLAN §3
  > (`types → store → {siblings} → src/`). If this blueprint needs an edge not in that DAG, that is
  > a divergence — flag it in the notes column of MIGRATION-PLAN §5, don't silently add the edge.

## 2. Responsibility & non-goals

> One paragraph: what this crate **owns** — the decisions/state/computation nothing else may
> duplicate. Then a bullet list of what it must explicitly **NOT** do (the seam other crates sit
> behind). Every non-goal should name which *other* crate owns that concern instead.

**Owns:** `<...>`

**Non-goals (the seam):**
- `<must not do X — that's <other-crate>'s job>`
- `<...>`

## 3. Public API contract

> Real, compilable Rust. Every `pub` fn/trait/struct/enum, each with a doc comment stating its
> contract (preconditions, postconditions, panics-never). Every fallible operation returns a typed
> error enum — never `String`, never `anyhow::Error`, never a bare `bool`/`Option` standing in for
> an error. This code block IS the spec; a reviewer should be able to `cargo check` it in isolation
> (modulo bodies) to know the crate compiles to the intended shape.

```rust
<paste the real signatures here>
```

## 4. Data model & invariants

> List every type that crosses the public boundary (or is load-bearing internally) and the
> invariant it enforces — the illegal state it makes unrepresentable. Explicit rules:
> - **Money/precision:** integers only (smallest unit — cents, milli-tokens, etc.), never `f32`/`f64`.
> - **Time:** never call `SystemTime::now()`/`Instant::now()` inside logic — accept a clock via
>   parameter or trait so tests can freeze it. Name where in this crate the clock is injected.
> - **Randomness:** same rule — injected `Rng`, never ambient/thread-local, if the crate needs any.
> - **IO/filesystem/network/subprocess:** name every boundary where this crate touches the outside
>   world, and whether that boundary is injected (a trait/port the caller supplies, mockable in
>   tests) or direct (a smell — justify it or move it to the caller).

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `<Type>` | `<...>` | `<...>` |

**Clock/RNG/IO injection points:** `<name each, or "none — this crate is pure">`

## 5. Reuse map

> For `extract`/`refactor`/`partial`: cite the exact fleet file, function name, and line range (or
> line anchor) for every piece of logic being lifted, and say precisely what changes vs what stays
> byte-for-byte. Evidence, not vibes — a reviewer should be able to open the fleet file at that line
> and see what you saw. For `build-new`: write "greenfield" and name the chosen library/crate and
> why (one line — cost/maturity/license).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `<path:Lstart-Lend>` | `<...>` | `<yes/no>` | `<...>` |

## 6. Behavior spec

> Per public fn, step-by-step, and for every input dimension below say explicitly what happens —
> "handled: <behavior>" or "typed error: <Variant>". Never leave a case unaddressed; a blank cell
> reads as a silent-wrong-output bug in review. Not every fn takes every input shape (e.g. a pure
> in-memory fn has no "IO partial-failure" case) — write "n/a: <why>" rather than omitting the row.

### `fn <name>(...)`

| Input dimension | Behavior |
|---|---|
| empty | `<...>` |
| null / `None` | `<...>` |
| wrong-type (if caller can misuse at a boundary, e.g. deserialize) | `<...>` |
| huge (size/count at 100x-1000x expected) | `<...>` |
| negative (if numeric) | `<...>` |
| duplicate (if identity-bearing) | `<...>` |
| concurrent (if shared state) | `<...>` |
| unicode / non-ASCII | `<...>` |
| already-exists (if creates something) | `<...>` |
| partial-failure (if multi-step / IO) | `<...>` |

*(repeat the table per public fn)*

## 7. Dependencies

> Exact crate name + pinned version + one-line why. Check fleet's `Cargo.lock` first (path:
> `fleet/keel/fleet/Cargo.lock` or the relevant workspace lock) and prefer a version already
> resolved there — a second version of the same crate in the workspace is a build-time and
> supply-chain cost with no benefit.

| Crate | Version | Why |
|---|---|---|
| `<crate>` | `<x.y.z>` | `<...>` |

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines** (`.rs`, `.py`, any code file — blank lines and
> comments count). This forces one responsibility per file. Decompose accordingly: `lib.rs` is a
> thin module-declaration + re-export hub only; each type, each trait impl, each logical unit gets
> its own small file. If a module would exceed 80 lines, split it (e.g. `admit.rs` / `settle.rs`
> instead of one `budget.rs`). Show the FULL tree below with a one-line responsibility per file and
> an approximate line count — the reviewer will reject any file projected over 80. Test files follow
> the same cap; split a large integration test into focused files.

```
crates/<crate-name>/
  Cargo.toml
  src/
    lib.rs            # ~15 — module decls + re-exports only
    <unit_a>.rs       # ~NN — <single responsibility>
    <unit_b>.rs       # ~NN — <single responsibility>
  tests/
    <behavior_a>.rs   # ~NN — <one behavior area>
```

`Cargo.toml` sketch:
```toml
[package]
name = "<crate-name>"
version = "0.1.0"
edition = "2021"

[dependencies]
<from §7>

[dev-dependencies]
<test-only deps, e.g. proptest>
```

## 9. Test plan

> Named tests (behavioral names — `rejects_expired_reservation`, never `test_1`), each stated as a
> concrete assertion, not a vague intention. Cover: unit tests (one per behavior-spec row that isn't
> trivially covered by another), integration tests (crate's public API end-to-end), mutation targets
> (name the specific mutant a test must kill — e.g. "flipping `>=` to `>` in the quota check at
> §3's `admit()` must fail `test_name`"), and property tests where the domain has an algebraic
> invariant (e.g. "route is deterministic across N reorderings of a stable input").

**Unit tests:**
- `<test_name>` — asserts `<...>`

**Integration tests:**
- `<test_name>` — asserts `<...>`

**Mutation-testing targets (what `cargo mutants` must NOT survive):**
- `<mutant, e.g. "boundary flip in X">` — killed by `<test_name>`

**Property tests (if apt):**
- `<property>` — via `proptest`/`quickcheck`, `<N>` cases minimum

## 10. Verification recipe

> The exact shell commands, run from `crates/<crate-name>/`, with the expected pass signal and the
> denominator to publish (e.g. "47/47 tests, 0 skipped" — never just "tests pass").

```bash
cargo test -p <crate-name> --all-targets
cargo clippy -p <crate-name> --all-targets -- -D warnings
cargo mutants -p <crate-name>
```
Expected: `<N>/<N> tests pass, 0 skipped` · clippy clean, 0 warnings · mutants caught ≥ `<floor>%`
(publish `<caught>/<total>`, not a percentage alone).

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum — none swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test code.
- [ ] Clock/RNG/IO are injected (trait or parameter), never read ambient inside logic.
- [ ] Thread-safety documented: is this crate's state `Send`/`Sync`? Single-writer? Lock-free? Say which and why.
- [ ] No float used for money, tokens, or any precision-sensitive count.
- [ ] No self-grading: the crate's own tests don't certify its own correctness as the sole gate — the verification recipe's denominator is published, and mutation testing is run, not just unit tests.
- [ ] The verify command's pass/fail denominator (`x/y`) is stated in this file and will be stated again in the PR — not just "green".
- [ ] Tests that touch the filesystem write only under a `tempdir()`/`TempDir`, never the repo tree or `$HOME`.
- [ ] Every non-goal in §2 is actually absent from the code (no scope creep into a sibling crate's job).
- [ ] **No source file exceeds 80 lines** (verified: `find src tests -name '*.rs' | xargs wc -l` — every file ≤ 80). `lib.rs` is a thin hub, not a dumping ground.

## 12. Definition of Done

> The single crossable line for this crate. Must include: §10's exact commands all pass with a
> published denominator, §11 fully checked, the registry (`registry/services/REGISTRY.md` or
> `registry/features/REGISTRY.md` per C1/L2) updated, and Opus review (MIGRATION-PLAN §6 step 4)
> completed — re-deriving the contract and reproducing one mutation by hand, not trusting the
> checkbox.

`<crate-name>` is DONE when: `<...>`
