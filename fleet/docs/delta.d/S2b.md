# S2b — Unblock commits: fix `swarm`'s real N* red

Backlog item: `handover/BACKLOG.md`, `## [ ] S2b — Unblock commits: the pre-commit hook can't land
anything while swarm/corpus are red`.

## Step 1 — is it real or flaky?

Ran `bash tests/acceptance/swarm.sh` against the pre-fix tree (uncontended, checked `ps aux` for
other `verify.sh`/`corpus`/`cargo` processes first — none running). Confirmed the same 8 failing
`N*` lines named in `docs/delta.d/S2.md` and `handover/BACKLOG.md`'s S2b item: `N1`, `N3`, `N4`,
`N8`, and four `N9` cases. Real, not flaky — this is a hard code path, not load-dependent.

## Diagnosis

`fleet plan` (`keel/fleet/src/main.rs`, `plan_command`) calls `route::for_plan(role,
&state_dir()?)?` for `Intent::Change` and `Intent::Diagnose` prompts, propagating any `Err` with
`?`. `route::for_plan` calls `crate::meter::planning_snapshot_at(state)?`
(`keel/fleet/src/meter.rs:267`), which returns a hard `Err(EXIT_ENVIRONMENT)` (after printing an
actionable "no persisted lanes and FLEET_METER_WINDOWS is unset" message) whenever no meter state
exists yet and `FLEET_METER_WINDOWS` is unset — exactly the state of a fresh `FLEET_STATE`
directory, which is what `tests/acceptance/swarm.sh`'s `N*` block uses (a fresh
`FLEET_STATE="$(mktemp -d ...)"` at the top of the script, no `FLEET_METER_WINDOWS` set for that
block).

That `?` aborted `plan_command` entirely: no `commands: N planned (denominator: N)` line, no
numbered `fleet ...` lines, no `fleet sow` line — just the meter's error message and a non-zero
exit code. Every one of the 8 failing assertions asserts something from the part of the output
that this abort skipped:

- `N1` — "add a --version flag" (Change intent) needs `swarm dispatch` in the text and rc=0.
- `N3`/`N4` — "what broke" (Diagnose intent) needs >=2 numbered `fleet ` lines and the word
  `denominator`.
- `N8` — "add a --version flag" needs `fleet sow` in the text (D41's plan-names-every-gate
  contract).
- `N9` — four ordinary implementation phrasings (Change intent) need `fleet plan` to exit 0.

`N2` (LedgerVerify) and `N5`/`N6`/`N7` (the unmatched-prompt refusal path) were unaffected because
those intents never call `route::for_plan` at all (`LedgerVerify` and `MeterShow` skip routing;
the refusal path in `intent::classify` never reaches routing either).

This is not `intent.rs`'s classification table (B4's territory) — every one of these prompts
already classifies to the correct intent (confirmed directly with `fleet plan "<prompt>"` before
the fix: `intent: implement a change` / `intent: diagnose what broke` printed correctly, then the
command aborted on the very next call). The bug is in `plan_command`'s own handling of
`route::for_plan`'s `Err` case, which is a narrower defect than B4's "intent coverage" scope and
does not touch the intent table, the verb list, or the `tests/fixtures/plan-prompts.txt` work B4
owns.

## Fix

`keel/fleet/src/main.rs`, `plan_command`: instead of `route::for_plan(...)?` (propagate-and-abort),
`match` on the `Result`. The existing `Ok(routed)` arm is unchanged (still prints the routed lane
or a route-emptied refusal, exactly as before). The new `Err(_)` arm prints an `UNAVAILABLE`
routing line (the underlying environment fault was already printed by `meter::planning_snapshot_at`
via its own `eprintln!`) and lets `plan_command` continue on to print the command list, exactly the
same "a plan is not an execution" principle already applied one branch above it for an ordinary
route refusal. Only `fleet run`/`fleet swarm dispatch` may actually refuse; `fleet plan` never did
before this bug and does not now.

No changes to `route.rs` (S5's territory, untouched — confirmed via `git diff route.rs` showing
zero lines from this session) or `crew/crew/adapters/` (also untouched). No changes to
`intent.rs`'s verb table — "make me a sandwich" was not touched and still refuses (see below).

## Proof: before / after

Before (pre-fix binary, fresh `FLEET_STATE`, `FLEET_SOW_BYPASS=1`, no `FLEET_METER_WINDOWS`):

```
$ fleet plan "add a --version flag"
intent: implement a change
agent: builder
skills: rust
fleet meter: no persisted lanes and FLEET_METER_WINDOWS is unset.
  ...
rc=3
```
(no "swarm dispatch" text, no "fleet sow" line, no commands list — N1, N8 both fail; rc=3 not 0)

After (fixed binary, same environment):

```
$ fleet plan "add a --version flag"
intent: implement a change
agent: builder
skills: rust
fleet meter: no persisted lanes and FLEET_METER_WINDOWS is unset.
  ...
routed lane: UNAVAILABLE — routing could not be evaluated (see the environment fault printed above).
  fix before running: resolve the fault above, then re-run `fleet plan` to see the routed lane.
commands: 3 planned (denominator: 3)
  1. fleet sow
  2. fleet sow accept
  3. fleet swarm dispatch
note: plan only — nothing was executed. Run the commands above, or use the REPL to confirm.
rc=0
```

`fleet plan "what broke"` after the fix: 2 numbered `fleet ` lines (`fleet doctor`, `fleet ratchet
show`), `commands: 2 planned (denominator: 2)` present, rc=0 — satisfies N3/N4.

All four N9 phrasings ("handle division by zero", "support UTF-8 filenames", "validate the config
on load", "delete the dead retry path") now exit 0.

Control case unaffected — "make me a sandwich" still refuses:

```
$ fleet plan "make me a sandwich"; echo rc=$?
fleet: no committed intent matches: "make me a sandwich"
  closest intents (3 candidates):
    ...
rc=7
```

### `bash tests/acceptance/swarm.sh` — before / after, run twice for stability

Before (pre-fix, reproduced live this session via `git apply -R` on this session's own
`main.rs` patch, rebuild, run — not assumed from prior docs): **45 passed, 8 failed** — `N1`, `N3`,
`N4`, `N8`, and four separate `N9` FAIL lines (one per phrasing).

After (post-fix), run twice consecutively, uncontended:

Run 1:
```
== 50 passed, 0 failed ==
```

Run 2 (stability check):
```
== 50 passed, 0 failed ==
```

Identical, clean, `0 failed` both times — real, not a flaky pass.

### `cargo test -p fleet`

`test result: ok. 95 passed; 0 failed; 1 ignored` (unit/snapshot suite) plus the
`illegal_lifecycle_transitions_do_not_compile` trybuild case: `test result: ok. 1 passed; 0
failed`. No regressions from this change.

### `FLEET_MUTANTS=0 bash verify.sh` (uncontended — confirmed no other `verify.sh`/`corpus`/`cargo`
processes running before and during the run)

```
== fleet verify ==
  .... fmt                         ok   fmt
  .... clippy -D warn              ok   clippy -D warn
  .... unit tests                  ok   unit tests
  .... acceptance builds           ok   acceptance builds
  .... cargo-deny                  ok   cargo-deny
  .... cargo-audit                 ok   cargo-audit
  .... secrets                     ok   secrets
  .... acceptance                  ok   acceptance
  .... readme                      ok   readme
  .... swarm                       ok   swarm
  .... policy                      ok   policy
  .... recur                       ok   recur
  .... semgrep                     ok   semgrep
  .... trivy                       ok   trivy
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  .... attest-smoke                ok   attest-smoke
  .... pytest                      ok   pytest
  .... detectors                   ok   detectors
  .... corpus                      FAIL corpus                     (see var/verify.log)
-- 17 passed, 1 failed, 1 skipped (denominator: 19 stages) --
```

`swarm` moved from FAIL to `ok` — that is this task's bar, and it is met. **`corpus` is still
red** — `var/verify.log` shows the same pattern already tracked as `B13`: 25 `TIMEOUT <detector>.sh
exceeded 30s -- treated as FAILED` lines (mass detector timeouts, not assertion failures),
`DENOMINATOR checked=34 total=34 excluded=69 caught=28`. This session never touched
`tests/corpus/`, `bin/corpus.sh`, or any detector script, and per this task's own instructions,
`corpus`/B13 is explicitly NOT this item's job to fix — forcing a timeout-prone gate green without
fixing the actual timeout cause would be exactly the "a check cheaper to fake than to satisfy will
be faked" violation `fleet/PRINCIPLES.md` warns against.

## Did a real `git commit` succeed?

**No — still blocked, by `corpus`/B13, not by `swarm`.** `.githooks/pre-commit` runs `verify.sh`
unconditionally; with `corpus` still red, `verify.sh` exits 6 (its own gate wrapper turns that into
`git commit` rc=1) and the hook refuses the commit regardless of which files are staged. Attempted
a real `git commit` (not `--no-verify`) staging this session's own changed file
(`keel/fleet/src/main.rs`), this delta doc, and the `handover/BACKLOG.md` update — plus S2's own
files that were already sitting staged from the prior session (this repo has one shared staging
area; there was no clean way to separate S2's already-staged diff to `main.rs` from this session's
new diff to the same file without an interactive/partial-stage operation this role is not to use).
The hook ran the full suite (real, ~17+ minutes uncontended) and blocked the commit on `corpus`,
exactly as predicted: `git log` still shows `db3b5d5` at `HEAD` after the attempt, and `git status`
still shows every file staged, untouched. Confirmed empirically, not assumed — see
`handover/PROGRESS.md`'s S2b line and the raw commit-attempt output below.

```
== fleet verify ==
  .... fmt                         ok   fmt
  .... clippy -D warn              ok   clippy -D warn
  .... unit tests                  ok   unit tests
  .... acceptance builds           ok   acceptance builds
  .... cargo-deny                  ok   cargo-deny
  .... cargo-audit                 ok   cargo-audit
  .... secrets                     ok   secrets
  .... acceptance                  ok   acceptance
  .... readme                      ok   readme
  .... swarm                       ok   swarm
  .... policy                      ok   policy
  .... recur                       ok   recur
  .... semgrep                     ok   semgrep
  .... trivy                       ok   trivy
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  .... attest-smoke                ok   attest-smoke
  .... pytest                      ok   pytest
  .... detectors                   ok   detectors
  .... corpus                      FAIL corpus                     (see var/verify.log)
-- 17 passed, 1 failed, 1 skipped (denominator: 19 stages) --
COMMIT_EXIT=1
```

`git log --oneline -1` after the attempt: `db3b5d5 B4: verify RED, findings kept, code not
committed` — unchanged, confirming nothing landed.

## Bottom line

- `swarm`'s `N*` red was real (confirmed stable across repeated runs), diagnosed to a genuine
  narrow bug in `plan_command`'s error handling (not B4's intent-table scope, not S5's route.rs),
  and fixed with a minimal, principled change: don't let an environment-fault `Err` from the
  router abort the whole plan, same as an ordinary route refusal already didn't.
- `swarm` is now real-green, proven with pasted before/after output run twice for stability, plus
  the full unit/snapshot/trybuild suite still passing.
- `corpus`/B13 remains red and remains blocking a real commit — that is B13's separate, already-
  tracked problem, reported honestly rather than forced green.
