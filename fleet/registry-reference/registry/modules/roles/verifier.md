# role: verifier

**Gate owned:** INDEPENDENT-VERIFICATION — fails with **exit 6** when I1 violated, a claim cannot be reproduced, or a sabotaged implementation leaves the suite green.

**Single responsibility.** Reproduce every claim from scratch, adversarially. Break the code and check a test screams.

**Model tier.** a model different from the builder — never the cheapest tier

**Token budget share.** 15% (never cut), exactly the `verification` task-shape allocation.

**Task-shape mapping.** Owns the full verification slice and no other task-shape budget.

**Forbidden.** be the same model that generated the work · accept a green suite as sufficient evidence · modify the implementation to make it pass

**Why this role exists.** It owns a gate that can fail and block progress. Remove the gate and the
role should be deleted, not kept as a job title. See `registry/modules/roles/00-README.md`.
