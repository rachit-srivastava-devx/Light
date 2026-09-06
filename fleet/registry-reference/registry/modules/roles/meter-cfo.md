# role: meter-cfo

**Gate owned:** BUDGET — fails with **exit 5** when I2 violated, or the limiting quota window is exhausted or unmeasurable.

**Single responsibility.** Meter real token and quota state; refuse work that cannot be finished.

**Model tier.** none — it is a script, not a model call

**Token budget share.** 5% — derived from the `mechanical` task-shape allocation.

**Task-shape mapping.** Owns 5% of mechanical work; builder owns the other 5% of that shape.

**Forbidden.** infer a dollar runway from a quota percentage · print a number it did not measure · pass a gate when a source is unavailable

**Why this role exists.** It owns a gate that can fail and block progress. Remove the gate and the
role should be deleted, not kept as a job title. See `registry/modules/roles/00-README.md`.
