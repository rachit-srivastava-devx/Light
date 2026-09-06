# Adversarial review: S1-semgrep-trivy

**Verdict: REJECT**

Review date: 2026-08-29. Contract reviewed: `handover/BACKLOG.md` S1 item 3 and its
acceptance at lines 79-96. Background rules reviewed: `handover/KT-CODEX.md`, especially
zero-input refusal, published denominators, both-direction controls, and mutation proof.

## What was claimed

`docs/delta.d/S1-semgrep-trivy.md` claims that Semgrep and Trivy are real blocking stages in
`verify.sh`; that an unexcluded Semgrep run found 31 findings in four classes and all 31 were
false positives; that the tuned Semgrep gate has zero findings; that Trivy secret scanning checks
the tree and has zero findings; and that the full verifier was 17 passed, 1 failed, 1 skipped of
19 stages. It marks the Semgrep/Trivy S1 piece done while describing Trivy vulnerability and
misconfiguration scanning as blocked/TODO.

## Commands actually run

| Command | Exit | Observation |
|---|---:|---|
| `semgrep --config=auto --quiet keel/fleet/src` | 0 | 6s; zero stdout and zero stderr. Silence did not prove that no rules ran. |
| `semgrep --metrics=off --config=auto keel/fleet/src` | 2 | `Cannot create auto config when metrics are off.` |
| `semgrep --config=auto --json keel/fleet/src` | 0 | 51 rules ran on 26 targets; 0 findings and 0 errors. This falsifies the current claim that auto itself silently no-ops; `--quiet` hid a real scan denominator. |
| `semgrep --config=p/rust --config=p/secrets --config=p/security-audit --json keel/fleet/src` | 0 | Reproduced 31 findings, 49 rules, 26 targets, 0 scanner errors. Default Semgrep exit remains 0 on findings without `--error`. |
| `bash bin/semgrep-gate.sh` | 0 | 5s; zero stdout and zero stderr. The required stage publishes neither checked targets nor rules. |
| `bash bin/trivy-gate.sh` | 0 | 18s; Trivy logged `num=5`, but the report rows showed `Secrets: -` (`Not scanned`) for five lockfiles and published no eligible-file `{checked,total}` denominator. |
| `trivy fs --scanners vuln .` | 130 | Still downloading `mirror.gcr.io/aquasec/trivy-db:2` after 4m39s; interrupted after reproducing the documented stall. No scan ran. |
| `trivy fs --scanners misconfig .` | 130 | Still downloading the checks bundle after 4m42s; interrupt caused embedded-check fallback followed by `context canceled`. No completed scan denominator. |
| Tuned Semgrep command against an empty `mktemp -d` target | 0 | Explicitly reported `Targets scanned: 0`, `Nothing to scan`, and success. |
| `trivy fs --scanners secret --exit-code 6 "$EMPTY"` against an empty `mktemp -d` target | 0 | Explicitly reported `num=0`, target `-`, `Not scanned`, and success. |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | 17 passed, corpus failed, mutants skipped; denominator 19. Semgrep and Trivy displayed green. |

The claimed Trivy positive-control attempts were not reproducible: the deliverable publishes
neither an exact command nor the planted literal, and it admits that neither attempt proved the
gate rejects a detectable secret. No Semgrep positive or mutation control is published either.

## Arithmetic and hard-case audit

- Semgrep class counts add up: `17 + 10 + 3 + 1 = 31`; `31/31 = 100%` claimed false positives.
- Verifier counts add up: `17 + 1 + 1 = 19` stages.
- The missing denominator is scan coverage. Neither wrapper asserts or publishes nonzero targets;
  both independently returned 0 after examining an empty directory.
- The 31 decisions are only grouped prose. There is no raw finding artifact or per-instance
  file/line disposition. All four rule IDs are globally excluded, so hard cases and future uses of
  those APIs are quietly dropped together.
- No detector self-fire was observed: Semgrep scans only Rust source, and the current Trivy repo
  scan reported no secret finding in its own documentation. This does not replace a positive control.

## Findings

1. **Vacuous gates, confidence 10/10.** Both scanner commands return success on zero inputs, and
   neither wrapper checks `{checked,total}`. This directly violates AGENTS.md hard rules 6 and 10
   and KT-CODEX laws 2 and 5.
2. **Semgrep was weakened to green, confidence 10/10.** `bin/semgrep-gate.sh` excludes every rule
   class responsible for all 31 findings. A new `unsafe`, `temp_dir`, `current_exe`, or `args` use is
   invisible. The comment promising a periodic manual rerun is not enforcement.
3. **New guards have no positive or mutation proof, confidence 10/10.** The deliverable admits the
   Trivy positive control never fired and provides no successful Semgrep control. KT-CODEX line 37
   says a detector without this proof is not accepted.
4. **DONE contradicts PARTIAL/BLOCKED, confidence 10/10.** Trivy vulnerability and
   misconfiguration scanning are explicit TODOs, while the implemented stage is secret-only.
   Environment blockage explains the gap; it does not implement it. The named `p/...` Semgrep packs
   also have no version, digest, or local lock, so “pinned” is unsupported.
5. **Typed scanner failures are lost, confidence 9/10.** Each wrapper maps scanner failure to exit
   3, but `verify.sh` counts any installed-stage nonzero as `FAIL` and exits 6. An environment fault
   is therefore reported as an invariant failure, contrary to AGENTS.md hard rule 7.

## Independent verifier output

```text
== fleet verify ==
  ok   fmt
  ok   clippy -D warn
  ok   unit tests
  ok   acceptance builds
  ok   cargo-deny
  ok   cargo-audit
  ok   secrets
  ok   acceptance
  ok   readme
  ok   swarm
  ok   policy
  ok   recur
  ok   semgrep
  ok   trivy
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 17 passed, 1 failed, 1 skipped (denominator: 19 stages) --
VERIFY_EXIT_CODE=6
```

The corpus log ended with `DENOMINATOR checked=34 total=34 excluded=69 caught=28` and included
25 detector timeouts. The old aggregate reproduced, but the present red cause was not independently
shown to be only B13. A red full verifier cannot satisfy B11's full acceptance bar.

## Exactly what must change

1. Make both wrappers parse machine-readable results, fail when targets/files checked is zero, and
   publish nonzero `{checked,total}` plus finding counts in `verify.sh` evidence.
2. Replace Semgrep's four global rule exclusions with an audited per-instance baseline; fail on any
   new/baseline-drift finding, and lock rule content by local file or immutable version/digest.
3. Add automated negative and positive controls for both gates using non-sensitive fixtures, then
   mutation-test each wrapper so a broken detector goes red and names the failure.
4. Preserve typed exits through `verify.sh`: scanner/environment rc 3 must remain environment fault;
   findings/invariant failures must remain rc 6.
5. Either wire the promised Trivy vulnerability/misconfiguration scans using a reliable cached or
   local bundle, or revise the contract before marking this slice done; then rerun the exact full
   verifier and publish a green terminal summary plus raw per-finding review evidence.

