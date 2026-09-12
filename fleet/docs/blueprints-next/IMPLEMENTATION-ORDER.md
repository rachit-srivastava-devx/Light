# Implementation order and low-parameter execution protocol

The order follows the LLD graph and minimizes rework. A node may be drafted in parallel, but code
must be built only after its incoming contracts are accepted.

## Waves

1. **Authority foundation:** `store`, `control`, `route`, `model-catalog`.
2. **Ingress and intent:** `connectors`, `ingest`, `intent`, `scan`, `probe_business`, `probe_tech`,
   `probe_learn`, `probe_research`, `questions`.
3. **Plan and context:** `dag`, `knowledge`, `context`, `planner`, `plan_review`, `ready`.
4. **Execution and evidence:** `builder`, `next_plan`, `review`, `verify`.
5. **Effects and learning:** `integrate`, `candidate`, `offline`, `post`, `approval`, `notify`,
   `broker`, `rollback`.
6. **Composition:** `user_cli`, then end-to-end `src/` wiring.

## One 4B-agent task

1. Read one node blueprint and its named LLD section.
2. Create only the manifest and thin module hub.
3. Run `cargo check -p <node>` or the named Python import check.
4. Add one pure type/predicate and its named test.
5. Run the focused test before adding another module.
6. Add the injected port or adopted library call site.
7. Run the integration test that proves one incoming/outgoing LLD edge.
8. Add each refusal/timeout/duplicate path from the behavior matrix.
9. Run lint, file-size, and denominator checks.
10. Run the real binary/effect test if the node has an effect boundary.
11. Stop and write a typed refusal if a dependency, contract, or input set is missing.
12. Return changed files, commands, real output, checked/total, mutations, and unresolved gaps.

The agent must not edit acceptance tests, contracts without an ADR, sibling crates, `src/` wiring,
or the blueprint to make a failing implementation look green.

## Fixture naming contract

Blueprint commands use concrete fixture paths so an implementation agent does not invent inputs:
`tests/fixtures/blueprint-<node-id>/...`. The first implementation step for a node creates the
named fixture from the blueprint's JSON shape, commits it to that node's test fixture area, and
publishes its digest. A missing fixture is an environment fault or blueprint blocker; it is never a
reason to replace the command with a fake success.

## Proof ladder

| Level | Evidence | What it proves | What it cannot prove |
|---|---|---|---|
| L0 | type/check | the proposed shape compiles | behavior |
| L1 | focused unit/property tests | local rules over declared inputs | composition or real effects |
| L2 | integration/contract tests | caller/callee compatibility | external provider availability |
| L3 | mutation/hidden/differential tests | test strength and unintended change | a wrong intent |
| L4 | real binary/effect trace | reachable product behavior | multi-day or production reliability |
| L5 | independent model review + hand mutation | separation of duties | universal correctness |

`DONE` requires the levels named in the node blueprint; no level is silently substituted by prose.
