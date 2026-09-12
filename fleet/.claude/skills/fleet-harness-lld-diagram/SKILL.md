---
name: fleet-harness-lld-diagram
description: Standing rule for the Fleet Coding Harness low-level system design diagram — it must always be regenerated at full component detail (every component in LLD.md §3/§4), never simplified to a high-level sketch. Use whenever asked to create, update, or regenerate the Fleet harness's LLD/system-graph diagram, or when LLD.md's architecture changes and the diagram needs to catch up.
---

# Fleet harness LLD diagram — always full detail

## The rule

Owner correction, 2026-09-11: a first pass at this diagram used ~17 invented, high-level
components and was rejected — "half of the details are not there." The accepted version
matches `LLD.md` §3 (mermaid flow) and §4 (responsibility table)
almost 1:1, at 32 components. **Every future regeneration of this diagram must stay at that
component-level detail or greater — never regress to a simplified sketch, even if a request
just says "make a diagram" or "update the graph."** If `LLD.md` gains a new mechanism (a new
crate, a new gate, a new cross-cutting system), the diagram is stale until it's added here too.

`LLD.md` is the source of truth. Do not invent parallel component names — pull them from §3/§4
directly. If a mechanism in `LLD.md` doesn't fit legibly as its own node, fold it into an
existing node's `sublabel` (as done for the two-tier secret scan inside `verify`, and the
80/90/95% retention pressure tiers inside `store`) rather than dropping it silently.

## Where things live

- Source of truth: `docs/LLD/LLD.md` (moved 2026-09-11: root → `docs/LLD/`, the owner wanted the
  many `lld-full-detail.*` companion files out of the repo root, not loose at top level) — §3
  architecture, §4 responsibility table, §11 retention, §13 secret-scan tiers, §16 multi-day/quota,
  §17 tokenomics, §19 self-opt safety, plus dated **Gap closure** paragraphs in
  §6/§7/§9/§12/§16/§17/§18/§19 closing rigor gaps a multi-agent review found (undefined terms like
  "material" or "demonstrated performance" used as real admission gates)
- Working spec (edit this, don't rebuild from scratch): `docs/LLD/lld-full-detail.architecture.json`
- Delivered artifact: `docs/LLD/lld-full-detail.html`
- Pre-existing, separate, simpler overview diagram — leave alone unless asked, and NOT part of
  this `docs/LLD/` move (it stays in its original location):
  `docs/design/fleet-harness/system-graph.html` / `system-graph-v2.json` (this is `LLD.md`'s own
  "overview" graph, referenced in §3 via a `../design/fleet-harness/` relative link; the
  full-detail diagram is the companion, not a replacement)

## Current component set (32), so a future session doesn't re-derive it

Main spine (top to bottom): User/CLI + GitHub/Gmail connectors → Event Ingest → SQLite WAL
Store → Controller → Intent Agent → Route Admission (+ Model/Adapter Catalog) → Ambiguity Scan
→ 4 parallel probes (business-context, technical-context, learning-retrieval, research) →
Question Merger → Workflow DAG → Module Planner (+ Context Compiler / Repo Memory+Standards
feeding it) → Reviewer Gate → Ready Contract → Builder Agent (+ Next-Module Planner in
parallel) → Code Review → Verify+Secret Scan → Integration Merge → Post-Merge Verify →
Publication Approval → External Broker / Notification.

Feedback loops (dashed, routed around the spine, not through it): Question Merger → back to
User; Reviewer Gate → back to User (pre-build walkthrough); Publication Approval → back to User
(PR + explanation); Workflow DAG → Ready Contract directly (skip-planning bypass at risk tier
low); failed Post-Merge Verify → Guarded Rollback → requeues into the Workflow DAG; Verify →
Learning Candidate → Offline Evaluator → promotes back into Repo Memory.

Every node carries a `tag`: `deterministic`, `model, gated` (a model proposes, an independent
check or deterministic gate catches disagreement before effect), or `model, ungated` (trusted
directly — only the 4 probes, whose output only ever produces questions/context, never an
effect). A legend card explains this. Package/mechanism references are folded into `sublabel`
where `LLD.md` actually names one (SQLite WAL, Git/MCP, ACP) — `sources` (archify's evidence-link
field) was tried and reverted: it requires a pinned, committed, public GitHub revision to verify
against, which `LLD.md` and the diagram spec don't have while they're still untracked.

Not in the diagram, by deliberate choice, named in a card instead ("Not drawn as nodes here"):
quota failover (a runtime retry concern, §16), the multi-repo ChangeSet saga states (§15), and
the 15-state pause/resume machine (§16) — each would need its own long-distance edges through an
already-tight layout for marginal legibility gain. The FR1–46 trace (§24) and the four trust
zones (§3) are also not packed in. Add any of these as separate views or a linked table if asked
— don't cram them into this graph and re-break legibility.

## How to regenerate (archify, learned the hard way this session)

1. Edit `lld-full-detail.architecture.json` directly (add/change components and connections
   to match whatever changed in `LLD.md`). Don't start over.
2. **Build vertically, not horizontally.** A wide diagram (many nodes per row) fails archify's
   1440px desktop-readability check hard — going from ~3080px viewBox width to a ~1500px tall,
   narrow layout took the projected text size from ~2px (fail) to ~5.5–6px (pass), with the
   same component count. Height has no equivalent hard ceiling; width does.
3. Target `meta.quality_profile: "standard"`, not `"showcase"`. A diagram this information-dense
   cannot clear showcase's absolute font-size floor without gutting content — standard's 9/9
   structural checks (crossings, corridors, label clearance, orthogonality) are the real bar and
   are fully achievable; showcase's extra desktop-readability floor is a legibility nice-to-have,
   not a structural correctness one.
4. Iterate using the raw renderer first, not the CLI's `--json` mode — it hides the actual
   diagnostic text behind a fail-closed boundary:
   ```bash
   cd ~/.claude/skills/archify
   ARCHIFY_QUALITY_PROFILE=standard node ./renderers/architecture/render-architecture.mjs \
     <path to docs/LLD/lld-full-detail.architecture.json> /tmp/preview.html
   ```
   Fix exactly what each diagnostic names (usually: align exact center-x for anything meant to
   be a pure vertical edge; add `labelDy`/`labelAt` per the tool's own suggested fix when a label
   overlaps a node; route any edge that must travel far back up the spine through a dedicated
   side corridor with explicit `fromSide`/`toSide`/`via`, never let it cross through unrelated
   nodes). Repeat until it exits 0.
5. Once clean, run the real acceptance commands and overwrite both files in `docs/LLD/`:
   ```bash
   node bin/archify.mjs validate architecture <spec.json> --quality standard --json
   node bin/archify.mjs deliver architecture <spec.json> docs/LLD/lld-full-detail.html --quality standard --json
   node bin/archify.mjs visual-check docs/LLD/lld-full-detail.html --json
   ```
   `visual-check` will report a viewport-overflow "fail" purely from the diagram's height — that
   is expected for a tall pan/zoom diagram, not a real defect; judge it from the actual PNG
   screenshots it produces, not the pass/fail flag alone.
