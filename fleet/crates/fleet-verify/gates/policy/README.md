# Fleet policy-as-data

These gates are Rego policies intended for evaluation by `conftest` (and later
by the in-process Rust Rego interpreter). Each policy has both a `.bad.json`
fixture that must be rejected and a `.good.json` control that must be accepted.

## `measured_nothing.rego`

Rejects any verdict with `checked == 0`. The denominator is published so a
reported pass proves that at least one input was actually examined.

**fake_cost:** The cheapest way to make this green is to change `checked` from
`0` to `1` without examining an input. That is not the correct fix; the producer
must run the check over a real input and publish the resulting `checked/total`
denominator.

## `attest.rego`

Requires all eight delivery elements under `predicate.elements`, including the
non-droppable `oracle_independence` element: `sow`, `blind_suite`,
`independent_verification`, `adequacy`, `blast_radius`, `rollback`, `cost`, and
`oracle_independence`.

**fake_cost:** The cheapest way to make this green is to add an empty
`oracle_independence` object. That is not the correct fix; the element must
record distinct O1/O2 authors, unreachable paths, both hashes, and the named
discriminator verdict.

## `arch.rego`

Rejects more than one `main.rs`. It also rejects a package whose runtime
dependencies do not include `blake3` when tree evidence reports a hand-rolled
permutation. The policy expects `main_rs`, `packages`, and `tree` evidence;
`runtime_deps` is accepted as an alias for `runtime_dependencies`.

**fake_cost:** The cheapest way to make this green is to delete or rename the
second entry point, hide the permutation from the tree evidence, or add an
unused `blake3` dependency. None is the correct fix. The correct fix is one
runtime entry point and use of the real `blake3` crate with the hand-rolled
permutation removed.

## Running the fixtures

Run `./policy/run.sh`. It checks that every policy has both fixture directions,
then expects each bad fixture to fail and each good control to pass. A missing
`conftest` is an environment fault and returns exit code `3`; it is never
treated as a passing verification.
