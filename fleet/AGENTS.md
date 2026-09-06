# AGENTS.md — operating rules for anything working in this repo

Blueprint: `../blueprints/Fleet-L8-Deep-Dive/`. Where code and blueprint disagree, **record the
disagreement in `docs/DELTA.md`** — the blueprint may be wrong and implementation is evidence.

## Hard rules (violating one is a rejected change)

1. **NEVER edit `tests/acceptance/*`.** The lead authored it before implementation. Making it pass by
   changing it is the failure this whole system exists to prevent.
2. **NEVER edit `contracts/*.json`** without an ADR in `docs/adr/`.
3. **`$?` after a pipe reads the WRONG command.** Do not do it. (Violated 6× in the predecessor,
   twice more in zsh where `PIPESTATUS` silently expands empty.)
4. **`mktemp -d` for any path later `rm -rf`'d** — never derive it from a content digest.
   (Two concurrent runs collided and wedged the machine for 40 minutes, twice.)
5. **A worker gets NO ledger path, NO state dir, NO socket path in its environment.** Its only channel
   out is fd 3. The parent stamps timestamp, actor, resolved model — the worker has no such field.
6. **A gate/check that examined zero inputs FAILS.** Never passes. (A gate read 0 inputs and passed;
   every CI run after it was meaningless.)
7. **Exit codes are typed:** `0` ok · `3` environment fault (missing tool/interpreter) · `6` invariant
   violation · `7` refusal (ambiguous/empty input) · `8` verification mismatch. An environment fault
   must never be reported as an agent failure.
8. **Every refusal writes a receipt before exiting.** (4 refusals once produced 0 receipts.)
9. **Money/hashes/counts are integers or fixed strings. No floats. No `unwrap_or(0)`** — absent is
   absent, never zero. (`Number(null)===0` printed 23 fabricated zeros.)
10. **Publish the denominator.** A verdict carries `{checked,total}`; `checked==0` is a failure.

## Verify before claiming
Run `bash tests/acceptance/p0.sh` and paste the real output including failures. A claim with no
reproducing command is not a measurement.

## Worktrees live inside this repo — never beside it

Agent worktrees go in `.worktrees/<name>` (gitignored). They must **never** be created as siblings
of `fleet-rs/`. A previous session left 38 sibling worktrees totalling **11 GB** in the parent
directory, mixed in with unrelated projects. Create them with:

    git worktree add -b <branch> .worktrees/<name> HEAD

and set `CARGO_TARGET_DIR` to an **absolute, repo-external** shared path so N worktrees do not each
build the crate from scratch — three parallel lanes cost 1.8 GB of duplicate artifacts and serialised
on CPU contention. The path must be absolute (not `$PWD`-relative) to avoid the bug where an agent
exports it from the wrong directory, and must live outside the repo tree so the M2 file-count
detector does not trip on build artifacts.

    export CARGO_TARGET_DIR="$HOME/.cache/fleet-rs-target-shared"

Remove them with `git worktree remove --force .worktrees/<name>` when the lane merges.
