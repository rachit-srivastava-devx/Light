# Adversarial review: ANTHROPIC-RESEARCH-2026-08-30

**Verdict: REJECT**

Reviewed deliverable: `docs/delta.d/ANTHROPIC-RESEARCH-2026-08-30.md`  
Required contract: item `ANTHROPIC-RESEARCH-2026-08-30` in `handover/BACKLOG.md`  
Review date: 2026-08-30

The deliverable cannot be accepted as written. The stated backlog contract does not exist; three of
five findings are materially wrong or overclaimed; and the independent verifier is red with typed exit
`6`. The strongest defects are not stylistic: the runtime MCP tool count is wrong, the “early victory”
property is not mechanically enforced system-wide, and the research evidence is not reproducible from
the document.

## Contract check

I searched the complete 405-line backlog, not only headings:

```text
$ rg -n -i 'ANTHROPIC-RESEARCH-2026-08-30|anthropic|research' handover/BACKLOG.md
EXIT_CODE=1
```

There is no item with the required ID and no Anthropic/research item. Therefore the acceptance criteria
named in the review brief are absent. A deliverable cannot satisfy an unavailable contract, and no
`done`/`partial`/`blocked` status exists to reconcile.

## Claim ledger

| # | What was claimed | What I observed | Classification |
|---:|---|---|---|
| 1 | Dynamic Workflows provides worktree-isolated parallel subagents; its default cap matches `min(16, cores-2)` | Dynamic Workflows does provide up to 16 concurrent agents and 1,000 total, with fewer agents on lower-core machines. Worktree isolation is separately opt-in by prompt or `isolation: worktree`. The blueprint explicitly derives **14**, not 16, on its 16-core reference host. | **PARTIAL / OVERSTATED** |
| 2 | SDK `AgentDefinition` partially overlaps `worker-payload.v1` but does not replace the cross-provider subprocess/digest contract | Official SDK fields overlap the proposed payload. The blueprint additionally requires launcher-owned skill bytes/digests, MCP config, sandbox, auth reference, resolved model and whole-payload digest. | **CONFIRMED** |
| 3 | The early-victory problem has “no gap”; exact detector hashes, denominators throughout `verify.sh`, and `measured_nothing` mechanically solve it | Hash integrity is green, but it proves file identity, not complete E2E execution. `verify.sh` publishes a stage denominator only; its stage wrapper accepts any command that exits 0. The Rego fixture rejects `checked==0`, but a fresh real `fleet status --json` returns `checked:0,total:0,empty:true` with exit 0. Full swarm and corpus stages are red. | **CONTRADICTED** |
| 4 | `fleet mcp` exposes only `impact` and `lesson_recall`; no outbound capability exists | The compiled CLI and a full JSON-RPC `tools/list` both expose **6** tools: `file_read`, `file_write`, `file_list`, `impact`, `lesson_recall`, `ledger_read`. `mcp.rs` has no direct HTTP client, but that narrow grep does not prove full worker containment: the real Claude/Codex adapters launch networked provider CLIs and do not pass the blueprint’s sandbox/no-ambient/MCP flags. | **CONTRADICTED** |
| 5 | Blueprint pricing matches current Anthropic pricing; Dreaming is memory curation, not a deterministic gate | The listed Opus 5, Sonnet 5 and Haiku 4.5 prices match Anthropic’s official pricing page. Anthropic describes Dreaming as scheduled session/memory review and curation, not a deterministic enforcement gate. | **CONFIRMED** |

**Finding denominator:** 5 total = 2 confirmed + 1 partial/overstated + 2 contradicted.

Official sources independently checked:

- [Dynamic Workflows documentation](https://code.claude.com/docs/en/workflows) — 16 concurrent maximum,
  fewer on machines with fewer cores, 4,096 work items, 1,000 agents total.
- [Claude Code worktrees documentation](https://code.claude.com/docs/en/worktrees) — subagent worktree
  isolation is enabled explicitly by prompt or `isolation: worktree`.
- [Effective harnesses for long-running agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents)
  — the early-victory failure is declaring completion after partial/unit or curl checks instead of real
  end-to-end use.
- [How we contain Claude](https://www.anthropic.com/engineering/how-we-contain-claude) — the containment
  claim concerns the complete agent environment and allowed egress, not whether one Rust source file
  imports an HTTP client.
- [Agent SDK Python reference](https://platform.claude.com/docs/en/agent-sdk/python) and
  [Claude pricing](https://platform.claude.com/docs/en/about-claude/pricing).
- [Building with Claude: managed agents](https://claude.com/blog/building-with-claude-managed-agents)
  — Dreaming reviews sessions and curates memory.

## Commands actually run

All commands were run from the quoted repository path. No Git command was run.

| Command / property | Exit | Actual observation |
|---|---:|---|
| `bash bin/detector-integrity.sh` | 0 | `105 detectors match the manifest (denominator: 105)` |
| `cd tests/corpus && shasum -a 256 --status -c MANIFEST.sha256` | 0 | `checked=105`, live files `=105` |
| `bash policy/run.sh` | 0 | 3 passed, 0 failed; denominator 3 policies |
| `conftest test ... measured_nothing.bad.json` | 1 | Expected rejection: 1 test, 1 failure |
| `conftest test ... measured_nothing.good.json` | 0 | Expected acceptance: 1 passed |
| Fresh-state `fleet status --json` | **0** | `checked:0,total:0,empty:true`; vacuous real-product success |
| Compiled `fleet mcp manifest 'keel/**'` | 0 | Returned 6 tools, not 2 |
| Bare compiled `fleet mcp` | 7 | Typed refusal with usage |
| Full MCP stdio `initialize` → `tools/list` → `tools/call impact` | 0 | Runtime listed the same 6 tools; `impact` returned `status:ok` and a receipt |
| `claude --version`; `claude --help` | 0 / 0 | `2.1.247`; blueprint-named flags exist in the installed CLI |
| `codex --version`; `codex exec --help` | 0 / 0 | `0.149.0`; blueprint-named flags exist in the installed CLI |
| Instantiate adapters and print `command("probe")` | 0 | Claude: `claude -p --model opus probe`; Codex: `codex exec --model codex probe` — no sandbox/MCP/no-ambient flags |
| `bash tests/acceptance/p0.sh` | 0 | 34 passed, 0 failed |
| `FLEET_MUTANTS=0 bash verify.sh` | **6** | 16 passed, 2 failed, 1 skipped; denominator 19 stages |

The direct P0 result was:

```text
== P0 acceptance ==
  ok   A1 fleet run exits 0
  ok   A2 run prints an artifact id
  ok   B1 artifact exists in the store
  ok   B2 artifact mode is 0444
  ok   B3 artifact is not writable
tests/acceptance/p0.sh: line 53: /var/folders/_x/39gv9nf93lbfh7p1fvzl1nn80000gn/T//fleet-p0.3Z7Vhp/artifacts/0094d2237b98a12d0c45d811d904ae86d3b73f181c3f460e2f71cb297942bded: Permission denied
  ok   B4 artifact id == its content hash (independent oracle)
  ok   C1 attest verify exits 0
  ok   C2 attestation written
  ok   C3 in-toto shape: subject digest == artifact id, predicateType correct
  ok   C4 tampered attestation -> exit 8
  ok   D1 no ledger path leaked into a worker env record
  ok   E1 ledger chain verifies
  ok   E2 chain valid after 20 concurrent appends
  ok   E3 no rows lost (count=29)
  ok   E4 no prev_hash reuse (THE assertion the row count misses)
  ok   F1 empty task refused with exit 7
  ok   F2 the refusal WROTE A RECEIPT (S1: 4 refusals -> 0 receipts)
  ok   G1 blake3 is a RUNTIME dependency, not hand-rolled
  ok   G2 no hand-rolled hash permutation in the tree
  ok   G3 exactly one implementation (A4: one implementation per concept)
  ok   H:--help exits 0 with real output (3679 bytes)
  ok   H:--version exits 0 with real output (82 bytes)
  ok   H:doctor exits 0 with real output (401 bytes)
  ok   H:bare invocation prints usage
  ok   H:'ratchet:show' is reachable (not a blanket refusal)
  ok   H:'console:--help' is reachable (not a blanket refusal)
  ok   H:'oracle' explains its refusal (67 bytes)
  ok   H:'adjudicate' explains its refusal (78 bytes)
  ok   H:'ratchet' explains its refusal (27 bytes)
  ok   H:'graph' explains its refusal (63 bytes)
  ok   H:'mcp' explains its refusal (78 bytes)
  ok   I1 missing FLEET_STATE exits 3
  ok   I2 env fault explains itself (263 bytes)
  ok   I3 doctor still reports with FLEET_STATE unset (444 bytes)
== 34 passed, 0 failed ==
EXIT_CODE=0
```

The `Permission denied` line is the expected negative write probe; B3 and B4 pass around it.

## Independent full verifier — exact console result

```text
$ FLEET_MUTANTS=0 bash verify.sh
== fleet verify ==
  ok   fmt
  ok   clippy -D warn
  ok   unit tests
  ok   acceptance builds
  ok   cargo-deny
  ok   cargo-audit
  ok   secrets
  ok   acceptance
  ok   readme
  FAIL swarm                      (see var/verify.log)
  ok   policy
  ok   recur
  ok   semgrep
  ok   trivy
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 16 passed, 2 failed, 1 skipped (denominator: 19 stages) --
EXIT_CODE=6
```

The red stages were not opaque:

- `swarm`: **44 passed, 9 failed**. Failures include free-text dispatch (`rc=3`), zero-command
  multi-step plans, no plan denominator, ordinary implementation prompts being refused, and one of six
  cold-store concurrent dispatches failing.
- `corpus`: `checked=34 total=34 excluded=69 caught=28`. Numerous detector processes exceeded their
  own 30-second timeout and were correctly treated as failed.

## Arithmetic and hard-case audit

- Full verifier: `16 + 2 + 1 = 19`; denominator closes.
- Swarm: `44 + 9 = 53`; denominator closes.
- Corpus file populations: 105 hashed shell files = 103 case scripts + `run.sh` + `_selftest.sh`.
  The 103 cases split into `34 checked + 69 excluded`; no cases disappear from this arithmetic.
- Corpus result: 28 red/caught + 6 clean = 34 checked. The 69 excluded cases are printed individually
  as not mechanisable, so they are classified rather than silently dropped.
- Research evidence: 5 numbered findings but **0/5 fully qualified source URLs** in the deliverable.
  Line 38 explicitly says the source bundle was not reproduced and asks the reader to find the prior
  session or rerun the work. That is not a durable citation trail.
- Blueprint cap: `W_cpu = 16 - 2 = 14`; `W_mem = 32768 / 102 ≈ 321`; therefore
  `W_build = min(14,321) = 14`. Calling Anthropic’s 16-agent maximum a match is arithmetically false.

## Known-cheat hunt

| Cheat | Result |
|---|---|
| Gate/detector weakened to pass | No manifest weakening found: 105/105 hashes match. However, integrity green is presented as behavioral proof while the corpus using those files is red. |
| `0` substituted for unknown/null | No fabricated null-to-zero conversion established. A different hard failure exists: fresh state reports an honest `0/0` and exits 0. |
| Vacuous empty-input pass | **Found:** real `fleet status --json` returns `checked=0,total=0,empty=true`, exit 0. The Rego fixture is not a system-wide guard. |
| Detector fires on its own documentation | Not observed. The `T9` case is explicitly classified `NOT MECHANISABLE`, not silently counted as passing. |
| Done while log says partial/blocked | Cannot be reconciled because the required backlog item is absent. The deliverable nevertheless labels findings 3 and 4 “no gap” despite runtime contradictions and a red verifier. |

## Exact changes required before acceptance

1. Add or restore the exact `ANTHROPIC-RESEARCH-2026-08-30` backlog item with measurable acceptance
   criteria, or correct the review brief/deliverable to the real contract ID. Re-review against that
   contract; absence cannot be waived.
2. Replace all bare domains with full, durable URLs and attach a per-finding evidence trail containing
   source, retrieval date, quoted/paraphrased claim, local command, output and exit code. Remove “ask the
   session” as the recovery mechanism.
3. Correct finding 1: Dynamic Workflows concurrency is native, but subagent worktree isolation is
   opt-in. Publish the exact cap comparison: Anthropic maximum 16 with CPU-dependent reduction versus
   blueprint reference-host cap 14 and measured/proven cap 6. Keep the C1 recommendation explicitly
   partial until a real Claude-path integration is exercised.
4. Replace finding 3’s “no gap” verdict. Separate detector-set integrity (105 files), executable corpus
   coverage (34/103 cases), stage-level denominator (19), and end-to-end user proof. A “mechanical” claim
   requires the real empty-state path to refuse or fail and the full swarm/corpus gates to be green;
   show positive and negative controls.
5. Correct finding 4 to the six-tool runtime surface. Scope the supported statement to “no direct HTTP
   code in `mcp.rs`.” Any broader containment claim must inspect and exercise the full worker boundary,
   including actual adapter argv, environment, filesystem scope, provider egress and MCP injection.
6. Re-run `FLEET_MUTANTS=0 bash verify.sh` and include the unedited final summary and typed exit. Do not
   call the fleet-rs-side checks “verified/no gap” while required stages remain red.
