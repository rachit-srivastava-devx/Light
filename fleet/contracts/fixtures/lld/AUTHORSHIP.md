# Fixture authorship (F02 lane contract §7.1, carried forward from blueprint `03` §3.3)

Rule (unchanged from the U1-era file this replaces): "the bad fixture must be written by a
different unit's builder than the one who writes the gate. A self-authored bad fixture is
*unverified*, and unverified is reported, not assumed."

Moved from `orb/apps/mobile/src/build/fixtures/AUTHORSHIP.md` (F02: schema and fixture ownership
moved fleet-ward, F02 lane contract §3.1/§5.5).

| fixture | authored by | status |
|---|---|---|
| `complete_module.json` | U1 (`sot-p0/lld-v1-schema`), adapted (`schema_version` added, §5.1) by the F02 builder | independent of the gate (F06 not yet built) |
| `one_line_freeze.json` | U1 (`sot-p0/lld-v1-schema`), unchanged | independent of the gate (F06 not yet built) |
| `one_alternative.json` | specified by the F02 lead in the lane contract §7.1 (exact required property: `grain:"leaf"`, one `alternatives` entry, must fail on path `alternatives`); the surrounding module content (purpose/interface/guarantees/failure_story text) was instantiated by the F02 builder against that specification | **honest disclosure, not self-authored in the U1/U3 sense**: the F02 builder is not the gate's author (F06 does not exist in this tree yet), so the independence rule's *purpose* (a bad fixture's failure mode must not be chosen by whoever implements the check it exercises) holds; the *exact prose* was chosen by the same person who wrote the validators it exercises, which the table above discloses rather than hides |
| `complete_freeze.json` | specified by the F02 lead (§7.1: must validate OK in all 3; `freeze.content_hash` must equal the recomputed hash of its own `module_brief`); instantiated by the F02 builder, `content_hash` computed mechanically (not by hand) via a throwaway canonical-JSON+SHA-256 script and independently re-verified against the file after writing (see the F02 builder's final report for the exact recomputation) | same disclosure as `one_alternative.json` above |
| `forged_freeze.json` | specified by the F02 lead (§7.1: `stamped_by: "orb:lld-ready"`, must fail only on path `freeze.stamped_by`); instantiated by the F02 builder as `complete_freeze.json` with exactly that one field changed (`content_hash` deliberately left correct, so the fixture does not also fail on a hash mismatch) | same disclosure as above |
| `numeric_trap.json` | specified by the F02 lead (§7.1/§6.3: a `module_brief` with `depth_evidence.score.ratio: 1.0` grafted on; hashing must refuse in all 3, identically); instantiated by the F02 builder from `complete_module.json`'s content plus the grafted field | same disclosure as above |
| (no new fixtures) | — | F06 added none. `complete_module.json` and `one_line_freeze.json` were authored by U1 before any gate existed and are used **unmodified** as this gate's control and bad fixtures. F06's builder verified byte-identity against the merge base rather than asserting it. This is the full independence bar, not the one-step-short disclosure the four F02-era fixtures carry. |
| `hash_forged_freeze.json` | specified by the F07 lead (lane contract §7.1: a byte-copy of `complete_freeze.json` with exactly one hex digit of `freeze.content_hash` flipped, nothing else; required to pass `lld::validate_lld_v1` and `fleet gate lld-ready` on its `module_brief` unchanged, and to be refused only by the new `fleet sow --lld` hash check); instantiated by the F07 builder via a throwaway Python script (`json.loads` the original, flip the first hex character after `sha256:` to the next value cycling `0-9a-f`, replace that exact substring in the raw file text — never re-serialize the JSON, so formatting stays byte-identical elsewhere) and independently verified by `diff` against `complete_freeze.json`: exactly one line differs, exactly one character within it differs, same byte count | same disclosure as the F02-era rows above — the F07 builder is not the author of the `sow --lld` hash-check guard it exercises (that guard did not exist before this lane), so the independence rule's purpose holds; the exact mechanism (flip one hex digit) was specified by the lead, the builder's freedom was limited to which digit and mechanical execution |

**Why the four new fixtures are not "self-authored/unverified" in the way U3's early fixtures
were:** U3's problem (see the old file's own history) was that the *same person* who wrote
`LldReadyGate.ts` also invented what should make a fixture fail, with nobody else checking the
failure mode was genuine rather than tuned to the gate's own blind spots. Here, the *lead* (a
different role, a different pass over the contract, writing before any implementation existed —
T1 in this estate's SDLC gate model) specified each fixture's required pass/fail behavior in the
lane contract *before* the builder wrote a line of `lld.rs`, `lld-v1.ts`, or `lld_schemas.py`. The
builder's freedom was limited to prose (module names, purposes) and mechanical construction
(computing a real hash), not to choosing which property must hold. This is disclosed, not asserted
as equivalent to full independent authorship — an independent verifier should treat it as one step
short of U1's original bar (a different human/session with no stake in the validator's design) and
form their own judgment.
