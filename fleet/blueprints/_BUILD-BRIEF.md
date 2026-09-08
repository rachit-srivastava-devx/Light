# Build-agent brief (implement ONE crate from its blueprint)

Repo root: "/Users/rachitsrivastava/youtube/Principal Engineering/Light". **The Cargo workspace and
everything below live under `fleet/` — `cd` into `<repo>/fleet` first.** All `crates/…`, `src/`, and
`blueprints/…` paths in this brief are relative to `fleet/`. You implement one crate's real code.
Follow the l8-code skill and `blueprints/PLAYBOOK.md`. This is a BUILD task — write code, don't
re-audit the spec.

## The #1 rule: trust the blueprint, write early, compile often
`blueprints/<your-crate>/BLUEPRINT.md` is the ALREADY-REVIEWED, Opus-approved spec. Implement §3's
public API and §8's file layout DIRECTLY. **Do NOT grep fleet source to re-verify types** — a prior
agent wasted 34 min doing that and wrote nothing. Only open a fleet file if the blueprint is genuinely
ambiguous on one specific point. If you are still only reading after ~10 minutes, STOP and start writing.

Work in this order, writing to disk immediately:
1. Fill `crates/<crate>/Cargo.toml` deps (per §7) + `src/lib.rs` (thin `pub mod`/`pub use` hub) + ONE
   small module. Run `cargo check -p <crate>` NOW; confirm it compiles.
2. Add remaining modules per §8 ONE GROUP AT A TIME, `cargo check -p <crate>` after each. Never let
   more than one module accumulate uncompiled.
3. Write the §9 tests. Run them.

## Boundaries (do not cross)
- Edit ONLY `crates/<your-crate>/`. NEVER touch the root `Cargo.toml`, `src/`, or another crate — the
  workspace is already scaffolded green with stubs; your crate is a member and `cargo check -p <crate>`
  works as-is.
- `fleet-types` is real and DONE — depend on it: `fleet-types = { path = "../fleet-types" }`.
- Sibling ADVISORY deps: define the port trait in YOUR crate (the blueprint shows it) and let `src/`
  wire the real adapter later. Do NOT path-depend on a sibling unless MIGRATION-PLAN §DAG names a hard
  compile edge for you (only `fleet-worker → fleet-merge` today).
- Add deps to your OWN Cargo.toml; prefer versions already in `fleet/keel/fleet/Cargo.lock`. If the
  blueprint flags a risky/dead dep (e.g. `ort`, `rank-fusion`), follow its resolution (defer behind a
  port / hand-roll), don't add it.

## Hard rules
- Every source file ≤ 80 lines. Typed error enums (thiserror). No float for tokens/counts. Inject
  clock/RNG/IO (no ambient reads in logic). Tests write only under a tempdir.
- Fold in any reconciliation the blueprint or MIGRATION-PLAN §7 names for your crate.
- Nothing stubbed with `unimplemented!()` in the public API — implement it. If truly blocked on one
  detail, minimally stub it, keep the crate compiling, and NAME it in your return.

## Verify (run for real; paste actual output — red included)
```
cargo test -p <crate> --all-targets
cargo clippy -p <crate> --all-targets -- -D warnings
find crates/<crate> -name '*.rs' -exec wc -l {} + | grep -v ' total$' | awk '$1>80{print; f=1} END{exit f}'
```
Do NOT report success unless all three are green. **Do NOT run `cargo mutants`** — concurrent mutants
runs collide on the shared `mutants.out`/`target` in this one workspace and hang (that stalled 5
agents). Opus does mutation review by hand. Your job ends at green tests + clippy + ≤80.

## Return
1. files created (path + line count); 2. the REAL verify output (test denominator `N/N, 0 skipped`,
clippy, wc-l gate, mutants if available); 3. anything you stubbed, deviated on, or that didn't hold —
Opus adjudicates and reviews before the crate is DONE.
