# L8 review gate for the 32 node blueprints

The lead reviewer owns this gate. A Sonnet/model completion message is an input, not approval.

## Coverage and traceability

- [ ] Exactly 32 node directories exist, matching `NODE-MAP.md` and the LLD JSON.
- [ ] Every blueprint names its exact LLD section, node tag, incoming edges, outgoing edges, and
  current source evidence or an explicit `greenfield` status.
- [ ] Every LLD edge has at least one typed payload contract and one integration test plan.
- [ ] Every FR1–FR46 row in LLD §24 maps to one or more node blueprints and a required observation.
- [ ] No blueprint claims implementation, live provider authentication, remote CI, production
  reliability, or external publication without that evidence.

## Engineering quality

- [ ] Responsibility is one-node-wide; sibling advice uses a port, not a hidden dependency cycle.
- [ ] `src/` is composition/wiring only. Effects have a parent-owned broker, receipt, grant, and
  idempotency key.
- [ ] Stable libraries/protocols are selected before custom mechanisms. Version, license, source,
  smoke test, and adoption limitation are recorded.
- [ ] Errors are typed; integers/fixed strings represent counts, tokens, money, sizes, and hashes;
  unknown values remain unknown with a reason.
- [ ] Clock, RNG, IO, subprocess, network, and model calls are injectable or explicitly isolated.
- [ ] Each proposed source/test file is ≤80 lines and has one responsibility.

## Actually-working and anti-stub evidence

- [ ] Tests call the public API and assert observable state/output, not only `is_ok()` or no panic.
- [ ] Each effectful or CLI node has at least one test using the real product binary or real adopted
  tool; a fake child may supplement but cannot be the only proof.
- [ ] A constant-return, empty-success, skipped-input, dropped-receipt, and wrong-payload mutation
  are each named and killed where applicable.
- [ ] Hidden tests are controller-authored and invisible to the proposing worker where the node is
  model-facing; property generators come from declared invariants and record a fixed seed.
- [ ] Contract tests exercise callers/callees; differential tests compare old/new behavior when a
  refactor or behavior-preserving claim is made.
- [ ] Required gates publish `checked,total`, fail on zero input, and fail on `checked < total`.

## Mutation and independent review

- [ ] Mutation targets are scoped to changed lines or the node's critical predicates.
- [ ] Default floor is `caught/total >= 75%` with the raw denominator; safety/authority predicates
  require `>=80%` unless the reviewer records a measured calibration reason.
- [ ] The independent reviewer re-derives one invariant, reads one hidden-risk path, and manually
  applies one mutation that a named test kills.
- [ ] A different model from the drafting model reviews the entire replacement set. Its findings
  are stored in `_reviews/` and unresolved P0/P1 findings block deletion of legacy blueprints.

## Definition of done for this documentation change

The replacement is done only when all 32 blueprints are complete, research references are present,
the cross-node map and implementation order are internally consistent, the independent review has
no unresolved P0/P1 finding, and repository verification records the real commands and failures.
This gate does not claim that the proposed Fleet runtime is implemented.

