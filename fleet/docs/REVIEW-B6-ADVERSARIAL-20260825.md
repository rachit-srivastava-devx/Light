# Adversarial review — B6

## Verdict

**REJECT**

The conclusion that Requirement 5 remains at 45% is directionally consistent with the
evidence, but B6 does not satisfy its acceptance contract. The blocked outcome was not
closed as `[!]`, the new probe numbers were not recorded in `docs/DELTA.md`, and the
published denominator is ambiguous and not checkable against the described five-attempt
results.

## What was claimed

`docs/delta.d/B6.md` claims:

- a default probe of `checked=19 total=19 usable=3`;
- a five-attempt probe with the same totals, with three usable lanes and `ch.at` excluded
  because its model is `UNRESOLVED`;
- no reliable 8-second lane exists, so Requirement 5 stays at 45%;
- `FLEET_MUTANTS=0 bash verify.sh` exited 6 with `14 passed, 1 failed, 1 skipped` and a
  corpus denominator of `checked=34 total=34 excluded=69 caught=18`.

The B6 contract in `handover/BACKLOG.md:72-76` requires that, when no lane sustains a
call every 8 seconds without rate limiting, the item remains at 45%, the probe numbers
are recorded in `docs/DELTA.md`, and B6 is marked `[!]`.

## Commands actually run

All commands were run from the quoted repository path. No git command was run.

### 1. Default lane probe

Command:

```text
bash bin/lane-probe.sh
```

Exit: `0`

Relevant observed output:

```text
https://api.llm7.io/v1/chat/completions | yes | 1/2 yes | codestral-latest | 611ms avg | 1/2 limited
https://blockrun.ai/api/v1/chat/completions | yes | 2/2 yes | nvidia/nemotron-3-super-120b | 2710ms avg | none in 2
https://devtoolbox-api.devtoolbox-api.workers.dev/ai/generate | yes | 2/2 yes | llama-3.2-3b-instruct | 973ms avg | none in 2
https://ch.at/v1/chat/completions | yes | 2/2 yes | UNRESOLVED | 2357ms avg | none in 2
checked=19 total=19 usable=3
LANE_PROBE_EXIT=0
```

The current run differs from B6's `api.llm7.io 2/2` claim: it returned `1/2`, with
one rate-limited call. This is live-service evidence and may drift, but it makes the
document's exact probe result non-reproducible.

### 2. Five-attempt probe implied by B6

Command:

```text
LANE_PROBE_ATTEMPTS=5 bash bin/lane-probe.sh
```

Exit: `0`

Observed summary:

```text
https://api.llm7.io/v1/chat/completions | yes | 3/5 yes | codestral-latest | 761ms avg | 2/5 limited
https://blockrun.ai/api/v1/chat/completions | yes | 5/5 yes | varies: nvidia/nemotron-3-super-120b, nvidia/nemotron-super-49b | 3732ms avg | none in 5
https://devtoolbox-api.devtoolbox-api.workers.dev/ai/generate | yes | 5/5 yes | llama-3.2-3b-instruct | 901ms avg | none in 5
https://ch.at/v1/chat/completions | yes | 5/5 yes | UNRESOLVED | 2255ms avg | none in 5
checked=19 total=19 usable=3
LANE_PROBE_5_EXIT=0
```

The command printed all 19 candidates. The non-usable cases were classified by the
runner as authentication-gated, browser-challenged, HTTP 404, unreachable, or
rate-limited. B6 itself only itemizes four lanes and omits the other 15 classifications,
so a reader cannot verify `usable=3` from the deliverable alone.

### 3. Independent verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Exit: `6`

Real terminal result, including red:

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
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
VERIFY_EXIT=6
```

The stage count and exit code match B6. The claimed `caught=18` denominator was not
independently attributable to this verifier invocation: `var/verify.log` is shared and
currently contains later/concurrent corpus summaries ending in `caught=25`. The B6
document therefore has no durable evidence binding `caught=18` to its stated verifier
run. The verifier red itself is not the reason Requirement 5 is blocked, but the
denominator citation should not be presented as independently reproduced.

## Findings

### F1 — The acceptance state was not closed

`handover/BACKLOG.md:72` is still `## [~] B6`, while the contract explicitly says to
mark B6 `[!]` when no lane sustains the required cadence. `docs/DELTA.md` contains the
older D57 evidence at lines 1166-1189, but it does not contain the B6 probe result
`checked=19 total=19 usable=3` or the current five-attempt results.

This is a direct acceptance failure, even though the prose correctly refuses to claim
parity.

### F2 — The denominator's unit is hidden

`bin/lane-probe.sh:37` increments `TOTAL` once per lane and `bin/lane-probe.sh:159`
increments `CHECKED` once per lane. With `LANE_PROBE_ATTEMPTS=5`, the run therefore
performs `19 * 5 = 95` HTTP attempts, while reporting `checked=19 total=19`. The B6
document presents that result without saying that the denominator is lanes rather than
calls. Its four named five-attempt lanes alone account for `5 + 5 + 5 + 5 = 20` calls,
not 19.

The counts are not false if interpreted as lane counts, but the interpretation is not
published and the call denominator needed to judge reliability is absent.

### F3 — The cited probe cannot test the required 8-second cadence

`bin/lane-probe.sh` contains no `sleep` or pacing control. It runs each lane's attempts
back-to-back. Its `none in 5` output therefore means “no limit observed in a short
burst,” not “sustained one call every 8 seconds.” B6 acknowledges this evidence boundary,
but does not run or publish a B6-specific paced probe; it relies on older D57 evidence
instead.

### F4 — Hard cases are present in the command output but dropped from the deliverable

The live runner classifies all 19 candidates, including auth-gated, challenge, 404,
unreachable, and rate-limited endpoints. B6 only lists three usable candidates and one
`UNRESOLVED` candidate. The omitted 15 candidates are not listed with their reasons, so
the document's `usable=3` cannot be checked by hand from the artifact. This violates the
project's explicit rule that hard cases must not be silently dropped.

## Exactly what must change

1. Mark B6 `[!]` in `handover/BACKLOG.md` because no reliable lane was established.
2. Add the B6 probe evidence to `docs/DELTA.md`, including the command, date, lane
   denominator, HTTP-attempt denominator, usable count, and every rejected lane with its
   reason.
3. Rewrite the probe arithmetic so `checked/total` is explicitly labeled as lanes and
   the 2-attempt/5-attempt HTTP denominators are published separately. Do not describe a
   burst as 8-second evidence.
4. Either publish a genuinely paced 8-second probe, or retain the blocked conclusion and
   cite the existing paced failure as prior evidence without implying the short probe
   established reliability.
5. If retaining the `caught=18` verifier detail, capture it from the same isolated run;
   otherwise report only the independently observed verifier result (`14/1/1`, 16
   stages, exit 6).

