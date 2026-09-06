# Status reporting convention — read this before touching a feature

One file per feature: `status/F##-slug.status` (plain `KEY=value` lines, one per line, no quoting).
You own only your own feature's file — never edit another feature's file, never hand-edit `STATUS.md`
(it's generated). This avoids the concurrent-shared-file-write problem the rest of this build has
already hit once before (see `FLEET-LEARNINGS.md`, "Concurrent agents on shared files destroy
measurement").

## Fields

```
ID=F03
FEATURE=lld-ready gate (depth-bar enforcement)
REPO=fleet
STATUS=building
AGENT=mid-engineer (lane: F03-lld-ready-gate)
SINCE=2026-09-07T02:14:00+05:30
NOTE=writing the calibration corpus fixtures
```

`STATUS` is one of: `pending` · `contract` · `building` · `verifying` · `verified` · `blocked` ·
`done`. `SINCE` is an ISO-8601 local timestamp — update it every time `STATUS` changes, not just
once. `NOTE` is optional, one line, plain text (no `|` or newlines — it renders in a markdown table
cell).

## When to write

- **Lead agent**, the moment you start a feature's contract: write the file with `STATUS=contract`.
- **Builder agent**, the moment you pick up the brief: `STATUS=building`.
- **Builder**, the moment its own verify gate is green and it hands off: `STATUS=verifying`,
  `AGENT=` the verifier's name once known.
- **Verifier agent**, on a real independent PASS (re-derived, driven end-to-end, not trusting the
  builder's report): `STATUS=verified`.
- **Verifier**, on a real FAIL or a stub/fake/dummy it caught: `STATUS=blocked`, `NOTE=` why, in one
  line.
- **Orchestrator**, once a feature is manually tested and FEATURES.md is updated: `STATUS=done`.

Then run `bash status/render.sh` (or just let the orchestrator do it — it re-renders on every check-in,
at minimum hourly).
