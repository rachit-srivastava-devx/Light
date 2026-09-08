# Adding a new crate (blueprint → build)

> **Migration status:** the original 16-crate migration is complete (see `MIGRATION-PLAN.md`'s
> header). This file is standing guidance for the day a NEW crate needs to be added to this
> workspace — it merges what were two separate mid-migration briefs (`_AGENT-BRIEF.md` for writing
> the blueprint, `_BUILD-BRIEF.md` for implementing it) into one, since they serve the same
> two-phase job for the same reader. Follow `PLAYBOOK.md`'s rules throughout both phases.

Repo root: `/Users/rachitsrivastava/youtube/Principal Engineering/Light`. The Cargo workspace and
everything below lives under `fleet/` — `cd` into `<repo>/fleet` first. All `crates/…`, `src/`
paths below are relative to `fleet/`.

## Phase 1 — write the blueprint

You are producing `<your-crate>/BLUEPRINT.md` and nothing else. Do not modify fleet source; do not
touch other crates' blueprints.

**Read first, in order:**
1. `PLAYBOOK.md` — how you work (AI-native SDLC; spec-before-code; no self-grading).
2. `_TEMPLATE.md` — the exact 12-section template. Fill every `<...>`, delete every `> guidance`
   line. **Hard rule lives here: no source file may exceed 80 lines** — your §8 must decompose into
   small one-responsibility files, `lib.rs` a thin hub.
3. `fleet-router/BLUEPRINT.md` and `fleet-types/BLUEPRINT.md` — two Opus-approved exemplars. Match
   their rigor: §3 is real `cargo check`-able Rust with typed error enums; §5 cites real
   `file:line` evidence for anything lifted from existing code; §6 fills the full edge-case table.
4. `MIGRATION-PLAN.md` §3 — the crate roster, DAG, and the canonical sibling-dependency rule
   (advisory deps go through a port trait wired in `src/`, never a Cargo path-dep on a sibling
   unless the DAG names a hard compile edge). The plan wins on crate boundary/DAG; the blueprint
   wins on implementation detail.

**Shared vocabulary is fixed — cite it, don't redefine it.** `fleet-types` owns `Role`,
`TaskId`/`NodeId`/`LaneId`, `Tokens` (integer minor-units), `ExitCode`, `LifecycleState`, and the
`Receipt`/`Attestation` wire types. Depend on those by name (`use fleet_types::...`). If you need a
shared type that isn't there, don't invent a local copy — flag it for lead review.

**Ground every claim in real source.** If you're lifting logic from existing code, open the real
file, read it, and cite real line ranges in §5 — never assume a whole file is liftable (the
`fleet-router` blueprint found `route.rs` was only ~⅓ pure; check before you claim). For genuinely
new code, write "greenfield" and name the exact chosen library/crate + version + one-line why.

**Return, after writing the file:** the file path; the crate's public API surface (the fn/trait/type
names in §3) as a compact list; any divergence from `MIGRATION-PLAN.md` (a reuse claim that didn't
hold, a missing type owner, an edge the DAG doesn't allow) for the lead to adjudicate.

## Phase 2 — build the crate from its accepted blueprint

This is a BUILD task — write code, don't re-audit the spec. Follow the l8-code skill and
`PLAYBOOK.md`.

**The #1 rule: trust the blueprint, write early, compile often.** `<your-crate>/BLUEPRINT.md` is
the already-reviewed, accepted spec. Implement its §3 public API and §8 file layout directly. Do
not grep the rest of the tree to re-verify types the blueprint already states — a prior agent on
this migration spent 34 minutes doing exactly that and wrote nothing (see `MIGRATION-PLAN.md` §7's
teach-back log). Only open another file if the blueprint is genuinely ambiguous on one specific
point. If you are still only reading after ~10 minutes, stop and start writing.

Work in this order, writing to disk immediately:
1. Fill `crates/<crate>/Cargo.toml` deps (per §7) + `src/lib.rs` (thin `pub mod`/`pub use` hub) +
   ONE small module. Run `cargo check -p <crate>` now; confirm it compiles.
2. Add remaining modules per §8 one group at a time, `cargo check -p <crate>` after each. Never let
   more than one module accumulate uncompiled.
3. Write the §9 tests. Run them.

**Boundaries (do not cross):**
- Edit only `crates/<your-crate>/`. Never touch the root `Cargo.toml`, `src/`, or another crate.
- Sibling ADVISORY deps: define the port trait in your own crate and let `src/` wire the real
  adapter later. Do not path-depend on a sibling unless `MIGRATION-PLAN.md`'s DAG section names a
  hard compile edge for you.
- Add dependencies to your own `Cargo.toml`; verify a crate is alive (recent releases, not yanked)
  before pinning it.

**Hard rules:** every source file ≤ 80 lines. Typed error enums (`thiserror`), never a bare
`String`/`bool`/`Option` standing in for an error. No float for tokens/counts. Inject clock/RNG/IO
(no ambient reads in logic). Tests write only under a tempdir. Nothing stubbed with
`unimplemented!()` in the public API — if truly blocked on one detail, minimally stub it, keep the
crate compiling, and name the stub in your return.

**Verify (run for real; paste actual output, red included):**
```bash
cargo test -p <crate> --all-targets
cargo clippy -p <crate> --all-targets -- -D warnings
find crates/<crate> -name '*.rs' -exec wc -l {} + | grep -v ' total$' | awk '$1>80{print; f=1} END{exit f}'
```
Do not report success unless all three are green. Do not run `cargo mutants` yourself if any other
agent might be building concurrently in the same workspace — concurrent `cargo mutants` runs
collide on the shared `mutants.out`/`target` and hang (this stalled 5 agents during the original
migration, see `MIGRATION-PLAN.md` §7). A lead/reviewer does mutation review by hand instead — one
mutation reproduced, confirm a test kills it.

**Return:** files created (path + line count); the real verify output (test denominator `N/N, 0
skipped`, clippy, the ≤80 gate); anything stubbed, deviated on, or that didn't hold, for review
before the crate is considered done.
