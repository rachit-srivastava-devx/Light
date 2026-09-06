# Fixture authorship (contract §3.3)

Per `docs/SPEED-OF-THOUGHT-P0-CONTRACT.md` §3.3: "the bad fixture must be written by a different
unit's builder than the one who writes `LldReadyGate.ts` (U1 authors fixtures; U3 authors the
gate). A self-authored bad fixture is *unverified*, and unverified is reported, not assumed."

| fixture | authored by | status |
|---|---|---|
| `complete_module.json` | U1 (`sot-p0/lld-v1-schema`) | independent — U1 does not write `LldReadyGate.ts` |
| `one_line_freeze.json` | U1 (`sot-p0/lld-v1-schema`) | independent — U1 does not write `LldReadyGate.ts` |

Both fixtures were authored against the contract's §2 (`ModuleBrief`) and §3.1 (gate check table)
directly, before `LldReadyGate.ts` existed (U3 had not started at the time U1 wrote these). U1's own
acceptance test (`lld-v1.test.ts`, U1-T1/U1-T2) exercises only shape validation
(`validateModuleBrief`), not the §3 business-rule gate — the gate verdict on both fixtures still
needed independent confirmation once U3 landed `LldReadyGate.ts` and its own frozen test
(`LldReadyGate.test.ts`, U3-T1/U3-T2).

**Reconciliation (this branch, post-U1-merge):** U3 was originally built against a local placeholder
type mirror (`lld-v1-local.ts`) and a self-authored pair of fixtures, both flagged unverified below
this line until U1 merged. U1 has since merged to `main` at `79652d9` with the real `lld-v1.ts` and
its own `complete_module.json` / `one_line_freeze.json`. On rebase, this branch took **U1's fixtures**
(the canonical, independently-authored pair per the table above) over U3's self-authored copies, and
re-ran them against the real, merged `LldReadyGate.ts`:

- `complete_module.json` (U1): passes all 14 checks (`C1-OPEN`, `C2-OWNER`, `C3-ACC-PARSE`,
  `C3-ACC-GROUND`, `C3-ACC-NONTAUT`, `R17-DERIV`, `R19-ABSOLUTE`, `R21-ALTS`, `R21-FAIL`,
  `C12-STORE`, `C12-DEPS`, `REG-VERDICT`, `IFACE`, `SHAPE`) against the frozen test's `REFS`
  (`owners: ['rachit@devxlabs.ai']`, `registry_paths: ['registry/services/llm-gateway',
  'registry/features/cost-control-plane']`, `known_node_ids: ['orb-freeze-ledger']`) —
  `lldReady` returns `READY` with `checked === GATE_CHECK_IDS.length` (U3-T2). Verified by running
  `LldReadyGate.test.ts` against these fixtures post-rebase: 6/6 pass.
- `one_line_freeze.json` (U1): trips 10 distinct check ids against the merged gate (`SHAPE`,
  `C1-OPEN`, `C2-OWNER`, `C3-ACC-GROUND`, `C3-ACC-NONTAUT`, `R17-DERIV`, `R21-ALTS`, `R21-FAIL`,
  `REG-VERDICT`, `IFACE`) — well over the contract's ≥5 floor (U3-T1).

U3's own self-authored fixtures (superseded by this reconciliation, not deleted from history — see
this branch's git log for `ff42d43`) are no longer in the tree; U1's pair is now the single source of
truth for both fixtures, matching the contract's stated authorship split.
