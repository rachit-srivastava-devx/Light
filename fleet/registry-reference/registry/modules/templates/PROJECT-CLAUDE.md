# CLAUDE.md — <PROJECT> (implementation repo)

Blueprint source of truth: `<BLUEPRINT_PATH>` (episodes define WHAT; this file defines HOW we work).

**Authority order (Company-OS L1):** `Company-OS/AGENTS.md` constitution → this file → `Company-OS/MODULE-REGISTRY.md` → the blueprint. Before building ANY capability: registry check (C1) — install existing module / extract on second use (C2) / build new as a module-shaped slice. State which branch you took (L2). Update the registry in the same change (L4).

## Session ritual (every session, no exceptions)
**Get bearings (first 5 actions):**
1. `pwd`; read `claude-progress.txt`; `git log --oneline -15`
2. Read `feature_list.json`; pick the ONE highest-priority feature with `"passes": false`
3. Run `./init.sh` and smoke-test one core journey before writing any code. If the app is broken, fixing it IS your task.

**Work:** one feature only. No drive-by refactors. Touch only files your brief/feature owns.

**Leave clean (last 3 actions):**
1. Run the gate: `bash .fleet/quiet.sh <gate command>` — must PASS
2. Commit with a descriptive message
3. Append 2-4 lines to `claude-progress.txt`: what changed, what's verified, what's next, any landmine

## The feature ledger — feature_list.json
- Every feature: `category`, `description`, `steps` (end-to-end, as a user), `passes`.
- You may ONLY flip `passes` false→true, and ONLY after executing the `steps` end-to-end yourself (browser automation for web UIs, real CLI invocation for CLIs). It is unacceptable to remove or edit feature definitions — this causes missing or buggy functionality.
- The project is done when every `passes` is true — never before, never "essentially done".

## Testing rules
- Run tests ONLY through `bash .fleet/quiet.sh <cmd>` — full logs go to `.fleet/logs/`, your context stays clean.
- Unit tests are the floor. A feature is verified by its `steps`, driven for real.
- Never edit `tests/large/acceptance/**` (hook-enforced). If a test looks wrong, stop and escalate.
- Evidence: every completed task writes `var/evidence/<task-id>/report.md` — commands, exit codes, pasted output.

## Parallel-work protocol (when >1 agent shares this repo)
- Claim before working: create `current_tasks/<feature-id>.md` with your session name + timestamp; skip features already claimed; delete your claim file when done (commit both).
- File ownership per brief is absolute. Conflicts = you took the wrong task.
- Merges are sequential: one branch lands, the rest rebase.

## Escalation
Stuck after 2 distinct approaches on the same error → write the failure state into `claude-progress.txt` + `current_tasks/BLOCKED-<id>.md`, stop burning tokens.
