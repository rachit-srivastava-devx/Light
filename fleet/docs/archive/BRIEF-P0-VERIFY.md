You are the INDEPENDENT VERIFIER. You did NOT build this. Your job is to try to BREAK the claim
that fleet P0 works, and to report honestly if you cannot.

The builder (codex) claims `bash tests/acceptance/p0.sh` passes. Do not trust that. Reproduce it
from scratch, then attack it.

## 1. Reproduce
- `cd keel && cargo build 2>&1 | tail -5`
- `bash tests/acceptance/p0.sh` — paste the REAL output, including any failures.
- Run it THREE times. A test that passes twice and fails once is a failing test (the predecessor had
  a concurrency bug that passed 8-way once and failed once — nondeterminism hid it).

## 2. Attack these specifically — each is a real historical failure
a) **Did the builder edit the acceptance suite?** `git diff -- tests/` and `git log --oneline -- tests/`.
   Any change is an automatic FAIL of the whole phase, no matter what the suite prints.
b) **Is the artifact really immutable?** Try to write to it as the current user. Try `chmod +w` then
   write. Report exactly what happens.
c) **Is the artifact id really the content hash?** Recompute blake3 yourself independently
   (`python3 -c` with hashlib.blake2b is NOT the same — use the `b3sum` crate via
   `cargo run` or `openssl` equivalent only if it is genuinely blake3; if you cannot verify
   independently, SAY SO rather than assuming).
d) **Ledger concurrency.** Run 50 concurrent appends (not 20). Assert: row count, chain verifies,
   and **count(distinct prev_hash) == count(rows)**. The last one is the assertion that catches the
   bug the row count misses. Report all three numbers.
e) **Does a refusal actually write a receipt?** Count ledger rows before and after a refused run.
   A refusal that records nothing is the `S1` failure (4 refusals -> 0 receipts).
f) **Vacuous pass check.** Does the suite still pass if you `rm -rf $FLEET_STATE` mid-run? Does any
   assertion pass while measuring nothing? Look for checks that can pass on empty input.
g) **Orphans.** After the suite, `pgrep -f fleet` — any surviving process is a FAIL (the predecessor
   hit load average 825 from orphans).

## 3. Report
Write `docs/VERIFY-P0.md` with: the pasted real output of all three runs, a PASS/FAIL per attack
above, and a one-line verdict. **If you cannot break it, say so plainly — that is a valid result.**
Do not fix anything. You are the verifier, not the builder. Report only.
