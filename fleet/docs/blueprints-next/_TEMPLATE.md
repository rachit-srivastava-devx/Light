# BLUEPRINT — `<node-id>`

This file is a complete implementation contract for one LLD node. Delete all guidance comments and
replace every placeholder before marking the node `blueprint-done`.

## 1. Identity and LLD path

- Node id / label / tag: `<exact JSON id> / <label> / deterministic|model,gated|model,ungated`
- LLD authority: `docs/LLD/LLD.md §<section>` and `docs/LLD/lld-full-detail.architecture.json:<id>`
- Why this node exists: `<business/system outcome in one sentence>`
- Incoming edges: `<source nodes and payloads>`
- Outgoing edges: `<destination nodes and payloads>`
- Build status: `extract | partial | greenfield | delta`

## 2. Responsibility and non-goals

**Owns:** `<decisions, state, computation, or effect that only this node may own>`

**Does not own:**

- `<concern>` — owned by `<node>`
- `<concern>` — owned by `<node>`

## 3. Boundary and authority

Describe the trust zone, input validation, output authority, durable boundary, and whether the
node is pure, port-driven, or effectful. State exactly what a model can propose and what it cannot
authorize. Name the parent-mediated effect, if any.

## 4. Crate/package layout

```text
crates/<node-id>/
  Cargo.toml or pyproject.toml
  src/ or <package>/
    lib.rs / __init__.py       # thin exports only, ≤80 lines
    <single-responsibility>.rs # ≤80 lines
  tests/
    <behavior>.rs              # ≤80 lines
```

Every file gets a one-line purpose and approximate line count. `src/` may only wire the node to
siblings through named ports or hard edges listed in `NODE-MAP.md`.

## 5. Public API contract

Show real compilable Rust signatures or real Python protocol/type signatures. Include doc comments
for preconditions, postconditions, ownership, and panic-never behavior. Every error is typed.

```rust
// or Python typing.Protocol / dataclasses when this node is an adapter package
```

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `<type>` | `<rule>` | `<state>` | `<typed error; 0/3/6/7/8>` |

State clock, randomness, filesystem, network, subprocess, and model ports are injected. State the
serialization/version/hash rules and whether the node is `Send + Sync`, single-writer, or process
isolated.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `<path or package>` | `<current primary source>` | `<call site>` | `<reason>` | `<smoke/live gate>` |

For extracted logic cite exact source lines. For greenfield work name the adopted primitive first.
No dependency is considered adopted merely because it appears in a manifest.

## 8. Behavior matrix

For every public operation, state behavior for empty, missing/null, wrong type, huge input,
negative number, duplicate, concurrent call, Unicode, already-exists, partial I/O failure,
timeout/cancellation, stale revision, and unavailable dependency. Write `n/a — <reason>` where a
dimension cannot apply.

### `<operation>`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | `<typed refusal or result>` |
| huge / negative | `<bounded behavior>` |
| duplicate / concurrent | `<idempotency or conflict>` |
| partial failure / timeout | `<receipt, retry, or refusal>` |
| stale / unavailable | `<versioned outcome>` |

## 9. Tiny implementation steps

Each step changes one bounded thing and has an immediate check. A 4B agent follows this list without
asking a design question.

1. `<create manifest/type>` → `<cargo check / import check>`
2. `<implement pure predicate/port>` → `<named unit test>`
3. `<wire one dependency>` → `<named integration test>`
4. `<add failure path>` → `<named refusal/receipt test>`
5. `<run the real command or composition path>` → `<observed output>`

## 10. Test matrix

**Unit tests:** `<named test>` — `<assertion and fixture source>`

**Integration/contract tests:** `<named test>` — `<real public path and downstream contract>`

**Hidden tests:** `<controller-authored case never shown to worker>` — `<failure caught>`

**Property tests:** `<invariant>` — `<generator, fixed seed, minimum cases>` or `not applicable: ...`

**Differential tests:** `<old/new or reference comparison>` — `<allowed divergence>` or `not applicable`

**Real-binary/effect test:** `<command or executable>` — `<observable receipt/side effect>` or `n/a`

## 11. Mutation targets and anti-stub proof

| Mutant or fake implementation | Test that must fail | Why this proves behavior |
|---|---|---|
| `<flip boundary/drop branch/return constant>` | `<test>` | `<observable property>` |

At least one mutation must be reproduced manually by the independent reviewer. Tests must fail for
a constant-return stub, empty success, skipped input set, fake child binary, missing receipt, and
wrong downstream payload wherever those failure modes apply.

## 12. Verification recipe and denominators

```bash
<exact commands from repo root>
```

Expected evidence: `<tests checked>/<tests total>, 0 skipped`; clippy/lint `<summary>`; mutation
`<caught>/<total> >= <floor>`; gate/input `checked=<n>, total=<n>` with `n>0`; real binary output
`<receipt/trace/exit code>`. Environment faults are not agent failures.

## 13. Definition of done

`<node-id>` is done only when:

- `<all public operations have behavior and typed failures>`
- `<all required tests and mutation targets pass with nonzero denominators>`
- `<real composition or binary path proves the LLD edge>`
- `<no source/test file exceeds 80 lines>`
- `<library smoke test and version/license evidence are recorded>`
- `<different-model reviewer re-derives one invariant and kills one mutation>`

## 14. Failure stories and review questions

**Failure → Cause → Fix:** `<real or anticipated operator failure>`

**Review questions this blueprint answers:**

1. `<what could be stubbed or falsely green?>`
2. `<what happens under the highest-risk failure?>`
3. `<why this boundary/library and not the obvious alternative?>`

