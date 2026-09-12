# Meta-L8 review of the Fleet LLD and node blueprints

Date: 2026-09-12  
Status: **FAIL / implementation hold**

This review is the decision record for the blueprint replacement. It is intentionally stricter
than a document-quality review: a blueprint is not ready if an implementation agent must invent an
authority boundary, wire type, state transition, command, dependency version, or proof denominator.
The command-level evidence for this decision is recorded in `VERIFICATION.md`.
The implementation-neutral closure draft in `docs/LLD/LLD-META-L8-ADDENDUM.md` is now propagated
to the affected staging blueprints and registry. It is not implemented, and the post-propagation
set has not yet received a fresh different-model review.

## What Fleet is actually building

Fleet is a local Rust controller for evidence-backed agentic software delivery:

```text
request/provider event -> normalize/deduplicate -> typed intent -> route
-> bounded ambiguity/context -> versioned DAG/plan -> leased worker
-> independent review -> deterministic verification -> local Git integration
-> post-merge verification -> explicit approval -> optional external effect
```

Models propose intent, plans, edits, reviews, and lessons. The parent controller owns durable state,
permissions, budgets, worker leases, evidence, merge/rollback, and promotion. The product reason is
provenance: a model completion, green unit test, or provider acknowledgement is not by itself proof
that Fleet's requested outcome is safe or complete.

## Verified structural result

- 32 architecture components exist in `docs/LLD/lld-full-detail.architecture.json`.
- 32 node directories exist under this staging directory, with one `BLUEPRINT.md` each.
- `EDGE-TYPES.md` records 43 JSON connections.
- All 32 node documents contain the 14 shared sections, node identity, Definition of Done,
  mutation material, and denominator language.
- These are documentation/design artifacts. Current package names, source seams, dependency rows,
  and planned commands are not runtime proof.

## P0 findings — block implementation

1. **Configuration trust root was missing from the original LLD.** The addendum assigns this to
   `control` with `store` persistence, but the trust-root loader, signature/pin, atomic reload,
   revocation, and value-provenance behavior remain unimplemented and unverified.

2. **Lease-scoped skills/MCP mediation remains unverified.** The addendum defines versioned
   `ToolRequest`/`ToolResult`, capability-bundle and tool-list digests, credential-handle lifetime,
   revoke/cancel behavior, and fd-3 routing, but no implementation or real-child proof exists.

3. **Multi-repository saga authority remains unverified.** The addendum assigns preparation to
   `integrate`, compensation to `rollback`, scoping to `approval`, effect execution to `broker`,
   and persistence to `store`; durable rows, readback, restart fencing, and compensation proof do
   not yet exist.

4. **Lifecycle/governance ownership is now assigned but not implemented.** `control` is the sole
   reducer/admission authority; `fleet-lifecycle` and `fleet-govern` are extraction inputs only.
   The sixteen-state table is in the addendum, but runtime replay, grants, quotas, retries, and
   precedence tests remain missing.

## P1 findings — block the affected implementation wave

- Run lifecycle now lists 16 states and a draft legal `(state,event,state)` table, but lacks runtime
  guard, durable event, replay result, and precedence evidence for pause/cancel/lease expiry/unknown effects.
- Cost accounting is now specified by Addendum §E (`UsageObservation`, unique source keys, late
  corrections, provider aggregate allocation, and `reserved = settled + released + held`), but
  runtime/provider billing proof is absent.
- GitHub/Gmail identity, secret custody, webhook/poll verification, cursor, and native-to-envelope
  mapping are now specified by Addendum §F, but current source uses legacy bearer/IMAP seams and
  compliant runtime proof is absent.
- Multiple LLD edge payloads were unresolved or contradictory. The staging registry now closes
  them as `RequirementInput`, `CandidateLesson`, `SourceManifest`, and forwarded `IntentSpec`,
  with `WorkflowSelection`/`ScanDecision` made an explicit alias. Runtime compatibility tests are
  still absent.
- The real-binary commands required by many blueprints are not current `fleet` subcommands, and the
  4B protocol forbids the implementation agent from adding composition wiring. Each command needs
  an owner, fixture schema, state-directory isolation, exit-code assertion, and binary provenance.

## P2 findings — close before calling the plan mature

- `verify` still has an `f64` coverage parser despite the repository's no-float rule; use integer
  covered/total line counts and checked cross-multiplication.
- Mutation rows are not uniformly bound to declared files/functions and qualified test IDs; a
  mutation run can otherwise discover zero targets and look green.
- The 80-line source/test rule needs more file decomposition in broad nodes such as `verify` and
  `connectors`.
- Dependency snippets use semver ranges while calling themselves pinned; exact lockfile version,
  checksum/license, smoke output, and limitation must be recorded per adopted package.
- Some named test and Definition-of-Done denominators drift across sections. The denominator is part
  of the acceptance contract and cannot be inferred from `test result: ok`.

## Independent review inputs

- Non-Astra Luna LLD completeness review: `docs/blueprints-next/_reviews/lld-completeness-luna.md`.
- Non-Astra Luna 4B executability review: `docs/blueprints-next/_reviews/blueprint-executability-luna.md`.
- Non-Astra Luna verification/anti-stub review: `docs/blueprints-next/_reviews/verification-antistub-luna.md`.
- Non-Astra Terra meta-L8 review: `docs/blueprints-next/_reviews/meta-l8-terra.md`.
- Codex CLI `0.149.0`, model `gpt-5.6-luna`, read-only review before the final propagation edits:
  **FAIL**; it independently reproduced the configuration, capability mediation, saga, lifecycle,
  cost, connector, edge payload, decomposition, anti-stub, and adoption findings above. It is not
  treated as a fresh sign-off on the final propagated set.
- Claude CLI with `--model sonnet`, read-only consult: **unavailable**; it exited without a review
  because its OAuth session was expired and could not be refreshed. No Claude completion is treated
  as evidence.

## Gate decision

**Do not claim implementation readiness or delete the legacy blueprint estate yet.** The 32-node
replacement now has broad structural coverage and explicit design ownership for the former P0
omissions, but the LLD is itself partial, runtime proof is absent, and the final propagated set
still lacks a fresh different-model review. The next authorized work is implementation-neutral
re-review, then runtime proof of the addendum contracts.
