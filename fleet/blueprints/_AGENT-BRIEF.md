# Blueprint-author brief (read this, then write ONE crate's BLUEPRINT.md)

You are a Sonnet author producing `blueprints/<your-crate>/BLUEPRINT.md`. Repo root:
"/Users/rachitsrivastava/youtube/Principal Engineering/Light". Write ONLY that one file. Do not modify
any fleet source. Do not touch other crates' blueprints.

## Read first, in order
1. `blueprints/PLAYBOOK.md` — how you work (AI-native SDLC; spec-before-code; no self-grading).
2. `blueprints/_TEMPLATE.md` — the exact 12-section template. Fill every `<...>`, delete every
   `> guidance` line. **HARD RULE lives here: no source file may exceed 80 lines** — your §8 must
   decompose into small one-responsibility files, each with an approx line count, `lib.rs` a thin hub.
3. `blueprints/fleet-router/BLUEPRINT.md` and `blueprints/fleet-types/BLUEPRINT.md` — two
   Opus-approved exemplars. Match their rigor: §3 is real `cargo check`-able Rust with typed error
   enums; §5 cites real `file:line` evidence from fleet source; §6 fills the full edge-case table.
4. `blueprints/MIGRATION-PLAN.md` — find YOUR row in §3 (build branch + reuse evidence), the DAG
   position (your imports/imported-by edges), and the global constraints. The plan wins on crate
   boundary/DAG; your blueprint wins on implementation detail.

## The shared vocabulary is fixed — cite it, don't redefine it
`fleet-types` owns `Role`, `TaskId`/`NodeId`/`LaneId`, `Tokens` (integer minor-units), `ExitCode`,
`LifecycleState`, and the `Receipt`/`Attestation` wire types. Read `blueprints/fleet-types/BLUEPRINT.md`
§3 and depend on those by name (`use fleet_types::...`). If you need a shared type that isn't there,
DON'T invent a local copy — flag it in your return notes for Opus to adjudicate (same as the
router/types authors did).

## Ground every claim in real source
For extract/refactor/partial crates: open the fleet files named in your §3 row, read them, and cite
real line ranges in §5. For build-new crates: write "greenfield" and name the exact chosen
library/crate + version + one-line why (prefer something already in `fleet/keel/fleet/Cargo.lock`;
for genuinely new deps, verify the crate exists and is maintained). Never assume a whole file is
liftable — the router author found route.rs was only ~⅓ pure; check before you claim.

## DAG discipline
Your §1 edges must obey `types → {lifecycle, store} → {siblings} → src/`. A sibling→sibling edge must
be minimal and named; if you need an edge the DAG doesn't allow, that's a divergence — flag it, don't
silently add it.

## Return (after writing the file)
1. the file path;
2. the crate's public API surface (the fn/trait/type names in your §3) in a compact list;
3. any divergence from MIGRATION-PLAN (a reuse claim that didn't hold, a missing type owner, an edge
   the DAG doesn't allow) — Opus adjudicates these, so be explicit.
