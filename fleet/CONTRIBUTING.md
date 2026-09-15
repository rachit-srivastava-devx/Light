# Contributing

## Required release note for `main`

Every push to `main` must include a versioned release note. This is required for code, build,
documentation, configuration, and operational changes.

For each push:

1. Select the next semantic version (`MAJOR.MINOR.PATCH`).
2. Add `docs/releases/v<MAJOR>.<MINOR>.<PATCH>.md`.
3. Add the same version and date to `CHANGELOG.md`.
4. Document:
   - what changed;
   - user-visible behavior;
   - files or commands affected;
   - verification commands and real results;
   - known failures, blockers, and unverified claims.
5. Keep the release note and changelog entry in the same commit as the change.

Before pushing, verify the release artifacts:

```bash
git diff --check
test -f docs/releases/v<MAJOR>.<MINOR>.<PATCH>.md
rg -n "\[<MAJOR>\.<MINOR>\.<PATCH>\]" CHANGELOG.md
```

Do not describe a local build as CI-green or a passing unit test as production proof. Release
notes must separate locally verified evidence from remote, provider, and production gates that
were not run.
