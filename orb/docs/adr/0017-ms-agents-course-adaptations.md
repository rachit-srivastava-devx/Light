# ADR 0017 — Adaptations from Microsoft's "Building AI Agents From Zero to Production"

**Status:** recommended; build work pending. **Date:** 2026-09-01.
**Registry decision:** install (CI/eval discipline only) — no architecture change.

## Context

Compared this product's build against Microsoft's course
[`Building-AI-Agents-From-Zero-To-Production`](https://github.com/microsoft/Building-AI-Agents-From-Zero-To-Production).
The course teaches building an **agentic** system: an LLM/agent framework picks which specialist
agent, tool, or route to use, and evaluations exist to catch it picking badly (LLM-as-judge
relevance/groundedness/tool-call-accuracy, layered with observability and smoke tests).

This product's control path is the opposite by design: `AGENTS.md` invariant #2 and
`docs/adr/LESSONS.md` (L1, L3, L7) exist specifically to keep an LLM from ever choosing state,
route, tool, or completion. The course's agent-framework pattern (Lessons 2, 6, 7 — handoff
orchestration, Toolbox tool-picking, A2A agent composition) is therefore **not adoptable** without
violating that invariant, and this ADR does not recommend it.

What the course gets right, independent of that architectural difference, is the **discipline
around shipping and evaluating** an LLM-touching system: a three-layer quality model (observability
always-on / smoke test every deploy / evaluation on a schedule), and treating tool/credential
governance as a single choke point. Both map cleanly onto gaps already identified in this repo's
own eval/cost/observability audit (2026-09-01 gap analysis: ~26-31 weeks of gap in that plane, the
single largest of the three planes audited), specifically the "no CI at all" and "eval samples are
synthetic expansions of a handful of real seeds" findings.

## Decision

Adopt, as build work against the existing blueprint (no blueprint doc text changes):

1. **Three-layer eval discipline**, matching course Lesson 3's model:
   - Observability: already real (`observability/metrics.py`) — no change needed.
   - Smoke test: new, cheap, fail-fast check run on every merge (deploy-time "is it reachable and
     does it follow its basic contract" — course Lesson 4's smoke-test CI gate). Distinct from and
     much cheaper than the full eval run.
   - Scheduled/pre-release eval: wire `eval/gates.py` and the golden sets into an actual CI job
     (`.github/`) that runs nightly or pre-release, not only on manual invocation.
   - This directly targets the audit's top-ranked gap ("No `.github/` directory exists at all").

2. **Real sample sizes for eval gold sets**, not synthetic expansion of 6-20 real cases to hit the
   blueprint's N (200/300/500). Either shrink the blueprint's stated N to what a real labeled set
   can support at this stage, or invest in growing the real corpus — do not keep expanding a
   handful of seeds and calling it N.

3. **LLM-as-judge pattern for genuinely subjective checks** (course Lesson 3's relevance/
   groundedness evaluators) — applicable to this product's empathy-appropriateness (≥95% target,
   `06-EVALS-AND-TESTING.md`) and context-pack groundedness checks, using a schema-locked judge
   call the same way the atomizer is schema-locked. This is evaluation tooling, not a control-path
   change, so it does not touch invariant #2.

## Explicitly not adopted

- **Agent framework / handoff orchestration / tool-picking** (course Lessons 2, 6, 7): would let an
  LLM choose route or tool, violating invariant #2 and repeating exactly the mistake `LESSONS.md`
  L1/L3/L7 already logged once.
- **Toolbox centralized tool governance** (course Lesson 6): moot today — the cognitive layer does
  not call agent-picked external tools. Revisit only if/when a tool-calling surface (e.g. calendar,
  GitHub integration) is added to the crew; until then `gateway-sidecar` (C9, "one model door") is
  the equivalent choke point for the one thing this product does call externally (the model
  provider).
- **Hosted Agent / Capability Host storage model** (course Lesson 5): not applicable — this product
  already owns its data store from day one (C6, `conversation_store.py`), so there is no
  "basic-managed-storage to sovereign-storage" migration to make.
- **A2A cross-org agent composition** (course Lesson 7): not applicable at the 20K-user single-app
  rung this product targets (blueprint `08-COST-MODEL.md`, honest-rung principle).

## Consequence

No `blueprints/ADHD-Focus-Orb-L8-Deep-Dive/` file is edited by this ADR. Build agents picking up the
CI/eval gap work from the 2026-09-01 audit should treat items 1-3 above as the adaptation brief;
everything under "explicitly not adopted" should not be reintroduced as a "modernization" later
without a new ADR that confronts invariant #2 directly.
