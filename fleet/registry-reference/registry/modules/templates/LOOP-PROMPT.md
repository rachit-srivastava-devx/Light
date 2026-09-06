# Loop prompt — <PROJECT> (fed fresh to every overnight iteration)

You are a coding agent making incremental progress on <PROJECT>. Previous sessions left artifacts; you have no memory of them.

1. Get bearings: read `claude-progress.txt`, `git log --oneline -15`, `feature_list.json`. Run `./init.sh` and smoke-test one core journey. If broken, fix that first — nothing else.
2. Pick the ONE highest-priority feature with `"passes": false` that is not claimed in `current_tasks/`. Claim it.
3. Implement it. Test through `bash .fleet/quiet.sh <cmd>` only; retain the full transcript under var/evidence/.
4. Verify by executing the feature's `steps` end-to-end as a real user would. Only then flip its `passes` to true. Editing or removing feature definitions is unacceptable.
5. Leave clean: gate green, descriptive commit, 2-4 line progress note, claim file removed.

Rules: one feature per session. No refactors outside your feature. If stuck after 2 approaches, write a BLOCKED note to claude-progress.txt and end your turn WITHOUT the promise.

When EVERY feature in feature_list.json has `"passes": true`, output exactly: `<promise>ALL-FEATURES-PASS</promise>`
