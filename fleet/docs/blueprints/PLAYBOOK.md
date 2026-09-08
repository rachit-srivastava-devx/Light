# How each agent works here — AI-native SDLC playbook (Fleet adaptation)

Every Sonnet/opencode agent on this migration follows Anthropic's AI-native SDLC playbook
(https://claude.com/blog/the-ai-native-sdlc-playbook), adapted to this repo. Read this before you
write anything. The playbook's six stages map onto our work as follows.

## The artifact chain (each stage's output is the next stage's input)

`MIGRATION-PLAN.md` (intent) → `<crate>/BLUEPRINT.md` (spec + plan) → crate code + tests →
Opus review → merged crate. The commit chain IS the audit trail: who asked, what was produced, who
approved. Never skip a link; never edit code without its blueprint accepted first.

## The rules that bind every agent

1. **Spec before code (plan mode discipline).** The `BLUEPRINT.md` is the accepted plan. If your
   implementation must depart from the blueprint, update the blueprint in the SAME commit and note it
   in MIGRATION-PLAN §7 — never let code and spec drift silently.
2. **Interrogate the plan before building.** Before writing code from a blueprint, ask: what breaks?
   what's the riskiest line? why not another approach? If the blueprint can't answer, it's not done —
   fix the blueprint first.
3. **Give yourself a feedback loop; verify your own work before a human sees it.** Wrap verification
   in one command (the crate's §10 recipe). Run it, iterate until green, and **paste the real output
   with the denominator** (`47/47, 0 skipped`) — never the word "passing" alone.
4. **Bug fix = failing test first.** Reproduce the bug as a failing test, commit the test, THEN fix.
   Do not rewrite the test to make it pass. A hook may block test edits during a fix task — that's
   the point: prove the bug is fixed, not that the test was weakened.
5. **You may not grade your own code (separation of duties).** An agent that wrote a crate does not
   certify it. Opus reviews at the contract level, distrusting the tests — re-deriving behavior and
   reproducing a mutation by hand (MIGRATION-PLAN §6 step 4). Tests are the floor, never the bar.
6. **Every mistake caught twice becomes a rule.** When review catches a defect, write the lesson into
   MIGRATION-PLAN §7 teach-back (and, where mechanizable, a gate/skill) so fleet stops repeating it.
   This is the "maintain" loop closing back to "plan".
7. **Evals as regression.** Each caught defect class should leave behind a test that stays in the
   suite — the crate's mutation floor and named unit tests are that suite.
8. **Institutional knowledge is code.** Conventions, gotchas, and "things we get wrong" live in
   version-controlled docs (this file, MIGRATION-PLAN, the blueprints), not in one session's memory.

## Human ↔ AI labor division (who holds judgment)

- **Sonnet/opencode (AI):** draft blueprints from the plan, write crate code + unit/integration/
  mutation tests from the blueprint, verify own work in the loop, self-report with real output.
- **Opus (lead):** owns crate boundaries/DAG, reviews every blueprint and every crate diff, reproduces
  one mutation by hand, drives one real behavior end-to-end, and is the ONLY role that flips a crate to
  DONE. Acts as the "code owner / branch protection" gate.
- **The owner (human):** sets direction, accepts the overall plan, resolves genuine product ambiguity.

## What "done" evidence looks like (never a proxy)

A crate is not done because `cargo test` exited 0. It's done when the §10 recipe passes with a
published denominator, clippy is clean, mutation kill-rate meets the floor, AND Opus has independently
re-derived the contract and reproduced a mutation. `exit 0` after a pipe is not a pass; a green unit
suite is not a working feature. Prove the property, not a proxy for it.
