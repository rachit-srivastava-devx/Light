# Code modification and retirement policy

This documentation migration does not silently delete or rewrite runtime code. The blueprints
decide the later code action from evidence.

## Current disposition

| Area | Current disposition | Required evidence before action |
|---|---|---|
| `crates/fleet-*` | Extract/reference candidates; keep until the corresponding node crate has equivalent public behavior | exact source-line reuse map, focused tests, contract tests, real-binary proof |
| `src/` | Composition-root candidate; business logic is moved only when a node crate owns it and `src/` can wire it through a port | dependency graph, no duplicate authority, end-to-end binary test |
| `keel/` | Historical/legacy implementation; not the authority for the proposed 32-node runtime | explicit replacement proof and a clean migration decision |
| JSON contracts under `crates/fleet-types/contracts/` | Protected | ADR before any edit, then schema parity and migration tests |
| `docs/DELTA.md` | Existing user-owned dirty work; preserve | separate authorized delta task |
| `docs/blueprints/` | Legacy blueprint estate, explicitly in scope for deletion after review | 32/32 replacement coverage and independent P0/P1 review clean |

## Delete-versus-modify rule

1. Preserve existing behavior until a new node crate proves the same or intentionally changed
   contract through differential tests.
2. Modify code when it is reachable product behavior needed by an accepted node contract and the
   change can preserve unrelated dirty work.
3. Delete code only after graph-based inbound references, tests, scripts, docs, and runtime entry
   points show it is unreachable or replaced. Move load-bearing assets into the owning crate first.
4. If implementation and LLD disagree, record the mismatch in `docs/DELTA.md`; do not force code to
   match a flawed document.

The present task owns blueprint replacement only. It does not authorize a runtime rewrite, contract
edit, branch reset, remote publication, or deletion of source directories.

