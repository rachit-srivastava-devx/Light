# The five roles — and the rule that produced exactly five

**A role exists only if it owns a gate that can FAIL and block progress.**

That rule is the whole design. It is why there is no sales, growth, or marketing role in a coding
fleet: they own no gate on a code change, so adding them buys coordination cost and no enforcement.
It is also consistent with the measured evidence already in `fleet/WORKFLOW.md` — *"2-4 agents is
the proven sweet spot — more is slower."*

| role | gate it owns | fails when | exit |
|---|---|---|---|
| `intake-pm` | AMBIGUITY | any open question remains unanswered | 7 |
| `architect` | CONTRACT | acceptance checks absent, or a locked check drifted | 6 |
| `builder` | IMPLEMENTATION | lint, typecheck, or the locked suite is red | 1 |
| `verifier` | INDEPENDENT VERIFICATION | I1 violated, or a claim cannot be reproduced | 6 |
| `meter-cfo` | BUDGET | I2 violated, or the limiting quota window is exhausted | 5 |

If you want to add a sixth role, name its gate and its failure condition first. If you cannot, it
is not a role — it is a task some existing role performs.

## Bandwidth allocation

There are two dimensions, and they are not rival taxonomies:

- **Task-shapes** describe *what kind of work* is being routed. Their shares are the canonical
  dispatch budget: judgment 25%, implementation 40%, mechanical 10%, research 10%, verification
  15%.
- **Roles** describe *who owns the failing gate*. Their shares are the derived operating capacity
  after the task-shape budget is mapped to gate owners below.

The reconciled allocation matrix is:

| task-shape | share | role allocation |
|---|---:|---|
| judgment | 25% | architect 15% + intake-pm 10% |
| implementation | 40% | builder 40% |
| mechanical | 10% | builder 5% + meter-cfo 5% |
| research | 10% | builder 10% |
| verification | 15% | verifier 15% |

Therefore the role totals remain:

| role | share | cut order |
|---|---|---|
| `builder` | 55% | **first** — build fewer slices, not worse-verified ones |
| `architect` | 15% | second |
| `intake-pm` | 10% | third |
| `meter-cfo` | 5% | fourth (it is cheap; it reads meters) |
| `verifier` | 15% | **never cut** |

Debugging follows implementation; docs follows mechanical; ambiguous and human-only work receive
no agent budget. The matrix sums to 100% in both dimensions: roles are 55/15/10/5/15, while
task-shapes are 25/40/10/10/15. The router should consume the task-shape policy and the role
documents should explain gate ownership; neither is a second budget split.

**Why verification is never cut.** A green suite written by the builder proves very little: the
source spec measured 80.2% of agent-written test patches containing no real check, and one model
gaming its task in 100% of runs. This build produced its own instance — `gates/05-evidence`'s
self-authored-evidence check was untested, and disabling it left the suite green at 42/42. Cutting
the verifier converts a measurable system into a hopeful one.

## The asymmetry that makes it work

`builder` and `verifier` must be **different models** (invariant I1), enforced at routing time
before tokens are spent, again in `gates/05-evidence`, and again in `lockcheck.sh ratify`.

Demonstrated in this build: **codex** generated all six components; **opus** verified them
independently, found four real defects, and fixed one by adding a mutation-proven test.

## Explicit non-features

Reputation stake and ratchet features are **not implemented**. There is no runtime, ledger, or
policy that assigns reputation, raises a threshold automatically, or penalizes a role. This
document makes no claim that either feature exists.
