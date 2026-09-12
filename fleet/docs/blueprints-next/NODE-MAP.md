# Fleet LLD node map

This is the one-to-one coverage index. The 32 rows below must each have exactly one directory and
one `BLUEPRINT.md`. The LLD JSON remains the topology authority; this table is the implementation
navigation aid.

| # | id | label | tag | planned package | primary LLD section | current evidence seam |
|---:|---|---|---|---|---|---|
| 1 | `user_cli` | User / CLI | external | `crates/user-cli` | §21 | `src/cli`, `src/interactive`, `src/dispatch` |
| 2 | `connectors` | GitHub / Gmail | deterministic | `crates/connectors` | §16 | no complete connector runtime; event adapter traits exist |
| 3 | `ingest` | Event Ingest | deterministic | `crates/ingest` | §6, §16 | `crates/fleet-events` ingest/guard |
| 4 | `store` | SQLite WAL Store | deterministic | `crates/store` | §5, §16 | `crates/fleet-store` ledger/memory stores |
| 5 | `control` | Controller | deterministic | `crates/control` | §5, §8, §16 | `src/pipeline`, `crates/fleet-lifecycle` |
| 6 | `intent` | Intent Agent | model,gated | `crates/intent` | §6 | `src/pipeline/classify_stage.rs`, intent types |
| 7 | `route` | Route Admission | deterministic | `crates/route` | §6, §9, §17 | `crates/fleet-router`, `src/dispatch/route_cmd.rs` |
| 8 | `model_catalog` | Model / Adapter Catalog | deterministic | `crates/model-catalog` | §9, §19 | `crates/fleet-crew` capability probes; live catalog is incomplete |
| 9 | `scan` | Ambiguity Scan | deterministic | `crates/fleet-scan` (edit-in-place) | §6 | `crates/fleet-scan` |
| 10 | `probe_business` | Business-Context Probe | model,ungated | `crates/probe-business` | §6 | partial pipeline/SOW probes |
| 11 | `probe_tech` | Technical-Context Probe | model,ungated | `crates/probe-tech` | §6 | repo/context commands, no complete node |
| 12 | `probe_learn` | Learning-Retrieval Probe | model,ungated | `crates/probe-learn` | §6, §11 | `crates/fleet-memory` retrieval |
| 13 | `probe_research` | Research Probe | model,ungated | `crates/probe-research` | §6, §22 | external research port only |
| 14 | `questions` | Question Merger | deterministic | `crates/questions` | §6, §7 | partial clarification/ambiguity paths |
| 15 | `dag` | Workflow DAG | deterministic | `crates/dag` | §6, §8 | `src/pipeline/graph.rs`, `stages.rs` |
| 16 | `knowledge` | Repo Memory + Standards | deterministic | `crates/knowledge` | §10, §11, §12 | `crates/fleet-context`, `crates/fleet-memory` |
| 17 | `planner` | Module Planner | model,ungated | `crates/planner` | §7, §8 | `crates/fleet-plan`, `src/dispatch/plan_cmd.rs` |
| 18 | `context` | Context Compiler | deterministic | `crates/context` | §10 | `crates/fleet-context` |
| 19 | `plan_review` | Reviewer Gate | model,gated | `crates/plan-review` | §7, §18 | partial review contracts in `crates/fleet-plan` |
| 20 | `ready` | Ready Contract | deterministic | `crates/ready` | §7, §13 | `crates/fleet-plan` readiness gates |
| 21 | `next_plan` | Next-Module Planner | model,ungated | `crates/next-plan` | §7, §8 | no complete overlap-safe implementation |
| 22 | `builder` | Builder Agent | model,gated | `crates/builder` | §3, §12, §14 | `crates/fleet-worker`, `crates/fleet-crew` |
| 23 | `review` | Code Review | model,gated | `crates/review` | §14, §18 | `src/dispatch/adjudicate_cmd.rs`, verifier routes |
| 24 | `verify` | Verify + Secret Scan | deterministic | `crates/verify` | §13, §18 | `crates/fleet-verify`, `src/dispatch/verify_cmd.rs` |
| 25 | `candidate` | Learning Candidate | deterministic | `crates/candidate` | §11, §19 | `crates/fleet-memory`, `crates/fleet-plan` teach |
| 26 | `integrate` | Integration Merge | deterministic | `crates/integrate` | §14, §15 | `crates/fleet-merge` |
| 27 | `offline` | Offline Evaluator | deterministic | `crates/offline` | §18, §19 | no complete paired evaluator |
| 28 | `post` | Post-Merge Verify | deterministic | `crates/post` | §14, §18 | partial verify/receipt paths |
| 29 | `approval` | Publication Approval | deterministic | `crates/approval` | §1, §13, §21 | no complete grant-bound publication path |
| 30 | `notify` | Notification | deterministic | `crates/notify` | §16, §20, §21 | `crates/fleet-stream` sinks |
| 31 | `broker` | External Broker | deterministic | `crates/broker` | §13, §16, §21 | no complete external-effect broker |
| 32 | `rollback` | Guarded Rollback | deterministic | `crates/rollback` | §14, §15 | `crates/fleet-merge`, rollback CLI tests |

## Node review rule

If a row cannot name a real source seam, it is `greenfield`; if a source seam does not satisfy the
LLD contract, the blueprint records the gap and the implementation task. Existing crate names are
not permission to merge responsibilities or to claim the LLD node already exists.

