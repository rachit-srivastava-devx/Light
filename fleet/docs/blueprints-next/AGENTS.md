# Fleet node-blueprint operating contract

These documents are the implementation authority for the proposed 32-node system in
`docs/LLD/LLD.md`. They are design artifacts, not proof that the runtime already exists.

## Authority and scope

1. `docs/LLD/LLD.md`, `docs/LLD/LLD-META-L8-ADDENDUM.md`, and
  `docs/LLD/lld-full-detail.architecture.json` define the product, node names, edges, contracts,
  budgets, and trust boundaries. The addendum only closes the four explicitly named authority
  omissions; it does not turn design into runtime proof.
2. Current Rust/Python source and tests define what can honestly be extracted. A blueprint must
   label missing or contradictory implementation as `greenfield`, `partial`, or `delta`; it must
   never turn a planned behavior into an observed one.
3. `docs/DELTA.md` records implementation-versus-design disagreements. Do not edit it while writing
   these blueprints unless the user separately authorizes a code/design delta.
4. External L8 guidance is evidence for the review method, not authority over this repository.

The replacement denominator is 32 node directories, one `BLUEPRINT.md` per LLD component. The
existing `docs/blueprints/` set is legacy input only and is deleted only after this replacement is
complete and reviewed.

## Non-negotiable design rules

- One node owns one responsibility. Cross-node behavior follows the LLD edge path exactly; do not
  create a convenient god crate or hide a workflow edge inside `src/`.
- Each node is planned as `crates/<node>/` (Rust) or `crates/<node>/` with an explicitly marked
  Python package for the adapter lane. `src/` is the composition root and wiring layer, not a
  second business-logic home.
- Prefer a stable, maintained library or provider protocol. Never implement hashing, OAuth,
  terminal emulation, a queue, a workflow engine, a vector index, a sandbox, or a model protocol
  from scratch when an adopted primitive satisfies the boundary. Every choice needs a current
  primary-source citation, version/commit, license, smoke command, and a stated limitation.
- A dependency declaration is not adoption evidence. A real call site, real smoke command, and
  observed output are required before an implementation claim.
- No float for money, tokens, counts, budgets, sizes, scores used as thresholds, or hashes. Use
  checked integers or fixed strings. Unknown is `None` plus a reason, never zero.
- Every fallible public operation has a typed error. No `unwrap`, `expect`, `panic`, bare `bool`,
  or `Option` may hide a production failure.
- A gate that examined zero inputs fails. Every verdict publishes `{checked,total}` and requires
  `checked > 0` and `checked == total` for pass.
- A worker receives no ledger path, state directory, socket path, credential, actor, timestamp,
  resolved model, approval, or budget-settlement authority. The only worker result channel is the
  inherited fd 3 protocol described by the LLD.
- Source files and test files are planned at 80 lines or fewer. `lib.rs`/`mod.rs` is a thin hub.
- Acceptance tests are lead-owned. An implementation agent may add a new test only when the
  blueprint explicitly names it; it may not weaken or edit an existing acceptance test.

## Evidence required in every node blueprint

The writer must complete every section of `_TEMPLATE.md`: exact LLD path and topology; ownership
and non-goals; public API; data invariants; edge-case behavior; source reuse map; package choices;
tiny implementation steps; unit, integration, property, contract, hidden, differential, and
mutation tests as applicable; anti-stub proof; commands with denominators; and definition of done.

`model` nodes propose only. `deterministic` nodes own the acceptance decision or effect. `model,
gated` nodes require an independent reviewer and a deterministic gate before their output has an
effect. `model, ungated` nodes can produce only bounded questions or context, never a direct side
effect.

## Low-parameter agent protocol

An implementation agent may read exactly one node blueprint plus the referenced contract/library
docs. It must execute the numbered steps in order, write one small file, compile/check, then write
the next. It must stop on an unresolved ambiguity, typed contract mismatch, missing dependency,
or zero-input gate; it must not guess. The blueprint is incomplete if a 4B agent would need to ask
which file, type, edge, command, or failure behavior is intended.
