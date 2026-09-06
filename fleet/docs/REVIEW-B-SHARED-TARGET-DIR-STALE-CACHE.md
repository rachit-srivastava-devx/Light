# Adversarial review — B-SHARED-TARGET-DIR-STALE-CACHE

**Verdict: REJECT**

The central technical defect is real and independently reproduced, but this deliverable is not an
acceptable evidence artifact. The named acceptance contract does not exist in `handover/BACKLOG.md`,
the only claimed repair command is incomplete and passes vacuously when executed literally, the
historical result has no published denominator, and the document is stale against the repository's
current D34/M3/merge-lane state.

## Contract actually found

The requested contract was item `B-SHARED-TARGET-DIR-STALE-CACHE` in
`handover/BACKLOG.md`. That file has no such item: the B-series present is B1 through B14, followed
by S5. Therefore there are **0 acceptance criteria available to check**. An absent contract is not
an implicit pass.

## What was claimed

1. Two removed worktrees shared `target-shared`; the cached test executable retained a deleted
   compile-time `CARGO_MANIFEST_DIR`.
2. Five named tests then failed with registry exit 3.
3. Touching `fleet/src/agent.rs` forced a rebuild and made the tests pass.
4. The root cause was not fixed; this fragment was left as a scoped backlog item.

## Commands and probes actually run

| Command / probe | Exit | Observation |
|---|---:|---|
| `bash -c 'export CARGO_TARGET_DIR="$PWD/target-shared"'` | 0 | Exact cited export is syntactically valid and emits no evidence by itself. |
| `git worktree remove --force ...` | NOT RUN | Explicit reviewer constraint: do not run git. The deliverable provides historical prose, not replayable worktree names that still exist. |
| `touch fleet/src/agent.rs && cargo test ...` | NOT RUN | `touch` would edit a prohibited source file, and `...` is not a complete test command. |
| `CARGO_TARGET_DIR="$PWD/target-shared" cargo test --manifest-path keel/Cargo.toml '...' --quiet` | 0 | **Vacuous green:** every test executable reported `running 0 tests`; 8, 95, and 1 tests were filtered out. |
| `CARGO_TARGET_DIR="$PWD/target-shared" cargo test --manifest-path keel/Cargo.toml --quiet` | 0 | Current repaired cache: 104 passed, 0 failed, 1 ignored across the reported test executables. This does not reproduce the historical stale state. |
| Five exact named test filters from the deliverable, against `target-shared` | 0 each | **5 passed / 5 checked / 5 total**; each named test actually ran once. |
| Disposable two-copy real-crate reproduction below | first 0, second 101 | First test passed; after moving the source worktree away, the same test failed at `intent.rs:282` after `Registry::load_default()` returned `Err(3)`. |
| `bash tests/corpus/M3.sh` | 0 | Printed nothing: no checked/total denominator and no identity of the binary examined. |
| `bash bin/detector-integrity.sh` | 0 | `105 detectors match the manifest (denominator: 105)`; this checks hashes/presence, not M3's stale-cache semantics. |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | **RED:** 16 passed, 2 failed, 1 skipped, denominator 19 stages. Failures: `swarm` and `corpus`. |

The independent stale-cache probe used two copies of the real `keel` workspace, not a toy crate and
not git:

```bash
review_tmp=$(mktemp -d)
mkdir -p "$review_tmp/.worktrees/test-fanout" "$review_tmp/main" "$review_tmp/shared"
rsync -a --exclude target keel/ "$review_tmp/.worktrees/test-fanout/keel/"
rsync -a --exclude target keel/ "$review_tmp/main/keel/"
cp -p agents.toml skills.toml "$review_tmp/.worktrees/test-fanout/"
cp -p agents.toml skills.toml "$review_tmp/main/"
case_name='intent::tests::every_intent_route_selects_declared_resolved_skills'
CARGO_TARGET_DIR="$review_tmp/shared" cargo test \
  --manifest-path "$review_tmp/.worktrees/test-fanout/keel/Cargo.toml" \
  -p fleet "$case_name" --quiet
/bin/mv "$review_tmp/.worktrees/test-fanout" "$review_tmp/removed-worktree"
CARGO_TARGET_DIR="$review_tmp/shared" cargo test \
  --manifest-path "$review_tmp/main/keel/Cargo.toml" \
  -p fleet "$case_name" --quiet
```

Observed denominator:

```text
REPRO_FIRST_BUILD_EXIT=0
REPRO_EMBEDDED_WORKTREE_PATHS_BEFORE_MOVE=24
REPRO_AFTER_WORKTREE_MOVE_EXIT=101
REPRO_EMBEDDED_DEAD_PATHS_AFTER_MAIN_RUN=19
REPRO_DENOMINATOR checked=1 total=1 reproduced=1
```

The failure was real:

```text
intent::tests::every_intent_route_selects_declared_resolved_skills --- FAILED
called `Result::unwrap()` on an `Err` value: 3
test result: FAILED. 0 passed; 1 failed
```

## Independent verifier — real output

```text
== fleet verify ==
  ok   fmt
  ok   clippy -D warn
  ok   unit tests
  ok   acceptance builds
  ok   cargo-deny
  ok   cargo-audit
  ok   secrets
  ok   acceptance
  ok   readme
  FAIL swarm                      (see var/verify.log)
  ok   policy
  ok   recur
  ok   semgrep
  ok   trivy
  SKIPPED WITH A REASON mutants   (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 16 passed, 2 failed, 1 skipped (denominator: 19 stages) --
FULL_VERIFY_EXIT=6
```

`swarm` reported **45 passed, 8 failed**. The failures were N1, N3, N4, N8, and
four N9 cases. `corpus` reported:

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=28
```

That arithmetic reconciles: 25 timed-out detectors plus M2, M6, and M7 equals 28 caught; six more
detectors passed, giving 34 checked. The manifest's 105 entries include `_selftest.sh` and
`run.sh`; the runner excludes those two, so 34 checked + 69 excluded = 103 runnable detector files.

## Findings

### F1 — The acceptance contract is absent (blocking, confidence 10/10)

`handover/BACKLOG.md` contains no matching item or acceptance criteria. The deliverable's final
claim that it was left as a scoped backlog item is false in the current tree. No reviewer can
derive ACCEPT from zero criteria.

### F2 — The claimed proof command is vacuous (blocking, confidence 10/10)

`cargo test ...` is an ellipsis, not a reproducible command. Cargo accepts the literal string as a
test filter, executes zero tests, and exits 0. The document records neither the exact command nor
its exit code/output, so the historical "passed immediately after" claim cannot be audited.

### F3 — The deliverable omits its denominator (blocking, confidence 10/10)

The list does contain exactly five test names, so `5 = 5` adds up. But the document never publishes
`{checked,total}` for the historical failing run or the post-touch run. It also does not say whether
all five actually executed after the rebuild. Current evidence is 5/5; historical evidence remains
unpublished.

### F4 — The write-up is stale against current implementation evidence (blocking, confidence 10/10)

`docs/DELTA.md` already records this as D34, `tests/corpus/M3.sh` exists, and
`bin/merge-lane.sh:25-32` attempts cache invalidation. The deliverable still says the fragment is
the write-up and the fix is merely a future backlog item. It neither reports these later controls
nor reviews whether they close the root defect.

### F5 — The existing detector can green-check the wrong artifact (blocking, confidence 10/10)

M3 hardcodes `keel/target/debug/fleet`. The documented worktree workflow exports
`CARGO_TARGET_DIR="$PWD/target-shared"`, whose binary is `target-shared/debug/fleet`. With that
environment active, the build and M3 examine different artifacts. M3 also silently exits 0 with no
denominator; if its hardcoded binary is absent it exits 77 and is excluded. This does not prove the
shared target is safe.

### F6 — The claimed structural invalidation can silently do nothing (confidence 9/10)

`bin/merge-lane.sh` runs `cargo clean` without binding it to the documented shared target path and
then discards every error with `2>/dev/null || true` before printing "invalidated". Unless the
operator happened to export the same `CARGO_TARGET_DIR` into the merge process, it cleans the
default target instead. Even when cleaning fails, the script reports success.

### F7 — The scope is wider than the two named registry sites (confidence 10/10)

There are **8** `env!("CARGO_MANIFEST_DIR")` occurrences in **6** Rust source files: `agent.rs`,
`skills.rs`, `main.rs`, `graph.rs`, `sow.rs`, and `route.rs`. Registry loading reproduced the defect,
but freelane, crew, SOW, routing, and test-source path resolution can retain the same dead worktree
root. A fix scoped only to `agents.toml`/`skills.toml` leaves the same failure class elsewhere.

## Exactly what must change before re-review

1. Add or restore `B-SHARED-TARGET-DIR-STALE-CACHE` in `handover/BACKLOG.md` with explicit,
   falsifiable acceptance criteria. Include a published denominator and positive/negative controls.
2. Replace `cargo test ...` with complete replayable commands, working directory, environment,
   exact test filters, exit codes, and pasted before/after output for all five named tests.
3. Reconcile the deliverable with D34, M3, and `merge-lane.sh`: state what is implemented, what is
   only mitigation, and what remains unfixed. Do not call an absent backlog item scoped work.
4. Make the guard inspect the actual active shared artifact (or every candidate fleet artifact),
   fail when zero relevant artifacts are checked, publish `{checked,total}`, and mutation-test both
   stale and clean binaries. Cache invalidation must target the exact shared directory and must not
   print success after a failed clean.
5. Cover all eight compile-time manifest-root consumers or document a deliberate boundary with a
   test per excluded consumer; then rerun `FLEET_MUTANTS=0 bash verify.sh` and record the real final
   stage denominator, including any red.
