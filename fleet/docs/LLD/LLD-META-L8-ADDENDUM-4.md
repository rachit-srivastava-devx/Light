# Fleet LLD Meta-L8 Addendum 4 — real gaps found by dogfooding, not by analysis

Date: 2026-09-14
Status: design-gap record. Builds on [`LLD-META-L8-ADDENDUM.md`](LLD-META-L8-ADDENDUM.md) (A-H),
[`LLD-META-L8-ADDENDUM-2.md`](LLD-META-L8-ADDENDUM-2.md) (I-P), and
[`LLD-META-L8-ADDENDUM-3.md`](LLD-META-L8-ADDENDUM-3.md) (Q-U); does not reopen any of them.

Addenda 1-3 found gaps by comparing the design against outside literature (ARIES, seL4, Coffman).
This one found its gaps a different way: by actually running `./target/debug/fleet run` against
real scratch repos and reading the real output, per the owner's standing instruction to perfect one
implemented node at a time and keep nothing where the flow itself is failing. Two of the three
items below are FR-trace omissions (§24 never asked for the behavior); one is an implementation
divergence from an already-stated design intent. No new component boxes are added to the
architecture diagram by this addendum -- all three live inside existing components (`router`,
`scan`, `verify`/pipeline stages), so `lld-full-detail.html`/`architecture.json` are unchanged.

## V. Non-git / local-directory execution mode — owner: `verify`, `src/pipeline`

§24's FR1-46 has no requirement for operating without git. `ensure_git_worktree` (called from every
`run`/`swarm`/`gate`/`oracle` intake) unconditionally required a real git worktree, so fleet could
never be pointed at a plain local project at all -- directly contradicting the owner's stated
intent ("fleet can work on local or non git repos as well"). Confirmed real, not hypothetical: `fleet
run --repo <plain dir, no .git> --task "..."` refused at intake, exit 3, before any other stage ran.

Closed at the implementation level for `run` only: an opt-in `--no-git` flag skips just the
git-worktree check at intake; `Merge` -- the one stage on this path that genuinely needs git -- is
recorded `"skip"`, never folded into `"pass"`; the machine receipt carries `repo_mode: "git" |
"no-git"` so a consumer can't mistake a skipped merge for one that happened. `swarm`/`gate`/`oracle`
intake, `crates/merge`'s worktree isolation, and `crates/builder`'s spawn/change-detect are
unchanged and still git-required.

**Open design question this addendum does NOT resolve:** should `swarm`/`gate`/`oracle` eventually
get the same opt-in, and does concurrent local-mode multi-agent work need a non-git equivalent of
worktree isolation (plain-directory copy-on-write, say)? §23's Increment C ("bounded concurrency")
assumed git worktrees throughout for lease/lock semantics -- a non-git concurrent mode is new scope,
not covered by extending `run`'s intake alone.

Required tests (done): `src/tests/run_non_git_repo/` -- 6/6, includes a regression pin that the
flag stays opt-in, a mixed-multi-repo case, and a real resume-after-kill case (verified
independently). FR trace should gain an FR47 row pointing here.

## W. Classification stage computes a discarded artifact — owner: `router`, `scan`

`src/pipeline/classify_stage.rs` calls `route::decide(None, ...)` -- role is hardcoded `None` on
*every* invocation, so `route::decide`'s five filter stages report `checked:0/total:5` unconditionally,
regardless of task content. This reads exactly like the "ambiguity scan" §6 describes ("checks a
fixed rule, not a vibe, before asking") but isn't one: it never varies with the task, so it can never
distinguish a clear task from an ambiguous one. The stage's real product -- a computed `TaskClass` --
exists in code but is discarded before reaching `Decision`, which carries no class field at all.

Found by direct dogfooding (`fleet run --task "add a hello function"` against a real scratch repo:
`classification` came back structurally identical -- `role: "invalid"`, 0 candidates at every
stage -- regardless of what the task text said). Not yet fixed; named here so a future node
doesn't have to rediscover it by the same trial. Two honest paths forward: wire `TaskClass` through
to a real role-selection input, or update §6 to state plainly that `Classify` is advisory-only
today and name what closes that gap.

## X. Router quota bootstrap has two entrypoints with diverging defaults — owner: `router`, `route_cmd`

`src/dispatch/run/run_cmd.rs::healthy_runtime()` exists specifically so a fresh CLI invocation gets
"an unmeasured-but-generous quota window and no cooldown" rather than being refused before any real
usage is ever measured -- a documented, deliberate bootstrap default. `fleet route`
(`src/dispatch/ops/route_cmd.rs::default_runtime()`) builds the *same* `RuntimeState` type for the
same router a second, independent way: empty `remaining` map, and `capable` populated by candidate
**id** where `filter_capability` reads `.adapter` -- so `fleet route --role builder` (and every
other role) refuses at stage 3/4 on a machine with real `claude` and `codex` installed, while `fleet
run` does not hit the same wall for the same underlying router. Confirmed live: `fleet route --role
builder|verifier|lead` all refuse, in this environment, with both CLIs genuinely on `PATH`.

Not yet fixed (filed separately: task `task_ea962afc`). Recorded here because it's a real
design-consistency gap between two adapters of one router concept, not because reading the code
alone would have found it -- it took running both commands against the same fresh state to see the
divergence.

## What this addendum does not claim

None of V/W/X required rewriting any part of the finalized design in §1-22; each is either a
missing FR row or an implementation that hasn't yet caught up to a design intent already stated
elsewhere in this document. Nothing here licenses treating §1-22 as wrong.
