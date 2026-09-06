# Light — voice-native build loop (Orb × Fleet), one independent root

Built per `blueprints/Speed-of-Thought-L8-Deep-Dive/` (the LLD artifact, `README.md`,
`ORB-AND-FLEET-DELTA.md`, `18-THE-LIGHT-APP-AND-UI.md`, `PHASES-TO-USABLE.md`) and decision **D15**
(2026-09-07): one monorepo root, top-level, **not** inside `company/products/` — fully independent,
no dependencies outside this directory.

## Layout

```
Light/
├── orb/                    copied from company/products/adhd-focus-orb (minus macos/OrbMac)
├── apps/macos/              copied from company/products/adhd-focus-orb/macos/OrbMac
├── fleet/                   copied from fleet-rs/
│   └── registry-reference/  copied from fleet/registry/ — SDLC-primitive shell scripts
│                             (intake·gate·ledger·statemachine·memory·ratchet·roles·budget·
│                             metering·monitoring·telemetry·scan·watch·isolate·kmap·context·
│                             crew·reputation / dispatch·sdlc·verify·review·measure) — reference
│                             source for Light/fleet to absorb the /task workflow SHAPE from
│                             (D1), not itself part of Light's own registry.
├── registry/{features,services,modules}/  NEW, local to this initiative (scaffold only —
│                             populated by extraction as P3 features land; not the estate's
│                             own top-level registry/, no cross-reuse between the two, D15 §8)
├── FEATURES.md               the atomic feature list, ordered by time-to-visible-output
├── STATUS.md                 generated — do not hand-edit (status/render.sh)
└── status/                   one .status file per feature; see status/CONVENTION.md
```

## How this was built (provenance, so nobody re-derives it by surprise)

- **Copy, not move.** The originals (`company/products/adhd-focus-orb`, `fleet-rs`,
  `fleet/registry`) are untouched — this is the D15-mandated rollback path.
- **File manifest = `git ls-files -co --exclude-standard`** in each source repo, not a hand-rolled
  exclude list. This means: everything git already tracks or would track (untracked-but-not-ignored)
  came across; everything `.gitignore` already excludes (node_modules, .venv, target, Pods,
  DerivedData, build caches) did not — including the one landmine this approach deliberately
  avoids: a *bare* `build/` exclude would have silently dropped the real, contract-mandated source
  path `apps/mobile/src/build/` (this exact mistake is already documented in
  `FLEET-LEARNINGS.md`/`ORB-AND-FLEET-DELTA.md` from the upstream repo's own `.gitignore` history —
  git's own tracked-file list sidesteps it entirely because the upstream negation fix is already
  merged there).
- **One fresh git history, not three nested ones.** `Light/` is `git init`'d fresh at this root with
  a single initial commit, rather than preserving three independent `.git` histories nested inside
  one tree. Rationale: D15 calls this "one monorepo root," and a real per-feature **worktree-lane**
  workflow (`git worktree add` per feature, per the build plan) needs one coherent repo to branch
  from — three nested `.git` dirs would make that unworkable. Full history for any file is still one
  `git log` away in the untouched original at its old path.
- **Regenerable artifacts excluded, deliberately, not an oversight.** node_modules/.venv/target/Pods
  etc. total ~17GB combined across the two source repos and are neither portable (symlink depth in
  particular — see `FLEET-LEARNINGS.md`, "the node_modules symlinks are baked at a fixed relative
  depth") nor meaningfully "the codebase." Each sub-project needs its own fresh
  install/`cargo build`/`swift build` here — that is real, tracked work (see `FEATURES.md`'s
  baseline-verification line), not assumed away.

## Reading order for anyone landing here

1. This file → `FEATURES.md` → `STATUS.md`.
2. `blueprints/Speed-of-Thought-L8-Deep-Dive/README.md` + `ORB-AND-FLEET-DELTA.md` (the LLD and the
   honesty ledger this build follows).
3. `FLEET-LEARNINGS.md` at the repo root — append a dated entry there when you finish a unit; read it
   before starting one (gotchas from the prior build session still apply here: shared-stash races,
   worktree-depth-relative symlinks, concurrent-agent file races).
