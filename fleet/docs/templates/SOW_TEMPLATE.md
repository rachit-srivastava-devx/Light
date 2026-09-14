<!--
Fleet's planning-intake template (goal item 7). Copy this into `--text` for `fleet sow`. The
six `## ` headings, the `request:` line, and a measurable threshold below are not stylistic --
they are exactly what `crates/plan/src/intake/sow.rs::validate_sow_text` checks structurally.
A SOW missing any of them is refused before it ever reaches planning (that refusal is the point:
it's what turns the vague-task problem named in `docs/LLD/LLD-META-L8-ADDENDUM-4.md`'s item W
into a typed, exit-7 refusal instead of a silently-accepted guess).

`crates/plan/tests/verifies_this_template/main.rs` proves this exact file validates cleanly --
if you edit this file, that test will tell you which check you broke.
-->

request: <one sentence — what is being asked for, in the requester's own words>

## Request restatement

<!-- Paraphrase the request line above in your own words. If this section says the same thing
     as `request:` with no new information, the requester and the builder haven't actually
     confirmed they agree on scope yet. -->

## Built for

<!-- Who consumes the result, and in what context. A crate? A human running a CLI command? -->

## Must do

<!-- The concrete, checkable behaviors this delivers. Bullet list, not prose. -->

## Explicitly will not do

<!-- Named out-of-scope items. "Everything else" is not an answer -- name at least the
     adjacent thing someone would reasonably assume is included but isn't. -->

## Done when

<!-- The observable condition that ends this SOW. Not "when it works" -- what a person or a CI
     job actually checks. -->

## Acceptance threshold

<!-- A measurable number: a percentage, "p95", or "exit 0". Example: "cargo test passes, exit 0,
     and the new gate's denominator is >0 on the first real run." -->
