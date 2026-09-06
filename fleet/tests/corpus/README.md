# tests/corpus — every catalogued failure, as an executable detector

**Rule: an issue faced once becomes a test here, forever.** The blueprint's failure corpus
(`../../blueprints/Fleet-L8-Deep-Dive/00-EVIDENCE-BASE.md`) plus this build's own findings
(`docs/DELTA.md`) are the input.

Each `<ID>.sh` exits **0** if the tree does NOT exhibit the failure, **1** if it does,
**77** if the failure is NOT MECHANISABLE (judgement — reported, never counted as a pass).

Two rules, inherited:
1. **detectable-by must name a mechanism**, not a hope. "Be more careful" is not a detector.
2. **caught? is measured by running the detector**, never by assuming it would fire.
Every detector needs a BAD fixture it flags and a GOOD control it accepts.
