## What changed and why

<!-- One or two sentences. The "why" matters more than the "what" -- the diff already shows what. -->

## Evidence

<!-- Paste real command output, red included. A claim with no reproducing command is not a
     measurement (AGENTS.md). -->

```
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
find src crates -name '*.rs' | xargs wc -l | awk '$1>80 && $2!="total"'   # must print nothing
```

## Scope check

- [ ] Did not edit a pre-authored acceptance test to make it pass (hard rule 1)
- [ ] Did not edit `crates/types/contracts/*.json` without an ADR, if touched
- [ ] No new file over ~80 lines (or a documented, deliberate exception)
- [ ] Registry updated if this adds/extracts a service or feature (C1/L2)

## Out of scope

<!-- What you deliberately did not touch, and why -- an honest gap is information. -->
