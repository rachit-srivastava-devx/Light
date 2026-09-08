# docs/

Index of what lives here after the `keel/`→`crates/` migration and the 2026-09-08 doc cleanup
(155 → 133 non-blueprint files reviewed; 76 point-in-time reviews plus 9 dated audits/briefs moved
to `archive/`, everything else fixed in place or left as active governance).

## Current guidance — read these first

| File | What it's for |
|---|---|
| [`QUICKSTART.md`](QUICKSTART.md) | Shortest verified path from a fresh clone to real output, executed top to bottom. Start here. |
| [`USING-FLEET.md`](USING-FLEET.md) | Every `fleet` command, a real walkthrough, the six exit codes. Linked from the root `README.md`. |
| [`DEPENDENCIES.md`](DEPENDENCIES.md) | Every crate, package, external binary and lane fleet depends on, and whether it's actually adopted. Linked from the root `README.md`. |
| [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md) | Symptom-first recovery commands. Linked from the root `README.md`. |
| [`SECURITY.md`](SECURITY.md) | Threat model, provenance boundary, residual risks. |
| [`ROUTING.md`](ROUTING.md) | The model-routing architecture decision (static role table, not a learned router) and the evaluated alternatives. |
| [`DELTA.md`](DELTA.md) | **Active, append-only.** Lead-owned log of every place implementation disagrees with the blueprint. Agents must not edit it directly — see `objections/`. |

## Active governance directories

- **`objections/`** — one file per agent, written when the agent disagrees with something (per
  the no-shared-write rule `DELTA.md` itself documents). Small and current.
- **`adr/`** — Architecture Decision Records. `ADR-0001` is live design guidance for the
  `lane-status.v1` contract projection.
- **`runbook/`** — on-call procedures (`disk-full.md`, `ledger-corrupt.md`, `restore.md`,
  `state-dir-missing.md`). Kept as current operational guidance; note the runbooks reference a
  `fleet state verify|restore|gc` command family that does not exist anywhere in `src/` or
  `crates/` today — flagged for the owner, not rewritten (see report from the 2026-09-08 cleanup).

## Historical record — `archive/`

Point-in-time reports, kept for history but not current guidance:

- **`archive/reviews/`** — 76 dated adversarial-review / walkthrough transcripts
  (`REVIEW-OPUS-WALKTHROUGH-*`, `REVIEW-B*`, `REVIEW-S*`, etc.), each a snapshot of one review run
  against a commit that has since moved on.
- **`archive/delta.d/`** — per-lane delta records (`B1.md`…`B22.md`, `S0.md`…`S3.md`, etc.), the
  detailed backing evidence behind rows the top-level review docs summarize.
- **`archive/ADOPTION.md`, `CONFORMANCE.md`, `BLUEPRINT-COHERENCE.md`, `LANES.md`, `LANE-SWEEP.md`,
  `STITCH.md`, `VERIFY-P0.md`** — dated (2026-08-24/25) audits measured against the pre-migration
  tree.
- **`archive/BRIEF-P0.md`, `BRIEF-P0-VERIFY.md`** — the original P0 builder/verifier task briefs,
  superseded once P0 shipped.

None of the archived files were rewritten to use current paths — they are historical records of
what was true when written, and correcting their paths would misrepresent that history.

## Not classified by this cleanup

- **`blueprints/`** — owned by a concurrent workstream; not read-modified by this pass beyond
  confirming which files it does and doesn't contain, for link-checking.
- **`pdfs/`** — see the report from the 2026-09-08 cleanup for what this 34MB directory actually
  contains (no PDFs) and the disposal recommendation; left untouched pending the owner's call.
