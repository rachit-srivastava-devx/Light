# Brief: <task-id> — <one-line goal>

**Project:** <repo> · **Blueprint source:** <OS folder>/episodes/Episode-XX §<sections>
**Builder:** mid-engineer | junior-engineer · **Layers required:** L0 L1 [L2] [L3]

## Goal
<2-4 sentences. What exists when this is done.>

## Files you own (touch nothing else)
- src/...
- tests/unit/...

## Acceptance tests (already written — DO NOT EDIT)
- tests/large/acceptance/<file> — run with: `<command>`

## Done means
1. `<acceptance command>` exits 0 in a clean checkout.
2. `var/evidence/<task-id>/report.md` exists: commands, exit codes, pasted output, fresh-clone repro steps.
3. For L3 tasks: the app was started and the journey `<journey>` driven end-to-end; output/screenshots in var/evidence/.

## Out of scope
<explicitly excluded work — prevents drive-by refactors>
