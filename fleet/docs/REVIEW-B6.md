# REVIEW — B6

## What was claimed

`docs/delta.d/B6.md` claims that both the default and five-attempt lane probes
completed with `checked=19 total=19 usable=3`, that three usable lanes were
observed, that the five-attempt results were `api.llm7.io 2/5`, BlockRun
`5/5`, DevToolBox `5/5`, and `ch.at 5/5 UNRESOLVED`, and that Requirement 5
should remain at 45%. It also records `FLEET_MUTANTS=0 bash verify.sh` as exit
6 with `caught=18`.

The B6 contract additionally requires: if no lane sustains calls every 8s
without rate limiting, record the probe numbers in `docs/DELTA.md` and mark
the backlog item `[!]`.

## What I ran

All commands ran from the quoted repository path
`/Users/rachitsrivastava/youtube/Principal Engineering/fleet-rs`.

1. `bash bin/lane-probe.sh` — exit `0`.

   Relevant live output:

   ```text
   api.llm7.io      1/2 yes, 1/2 limited, codestral-latest
   BlockRun         1/2 yes, 1/2 limited, nvidia/nemotron-3-super-120b
   DevToolBox       2/2 yes, llama-3.2-3b-instruct
   ch.at            2/2 yes, UNRESOLVED
   checked=19 total=19 usable=3
   ```

2. `LANE_PROBE_ATTEMPTS=5 bash bin/lane-probe.sh` — exit `0`.

   Relevant live output:

   ```text
   api.llm7.io      3/5 yes, 2/5 limited, codestral-latest
   BlockRun         5/5 yes, resolved model varied across calls
   DevToolBox       5/5 yes, llama-3.2-3b-instruct
   ch.at            5/5 yes, UNRESOLVED
   checked=19 total=19 usable=3
   ```

3. `FLEET_MUTANTS=0 bash verify.sh` — exit `6`.

   ```text
     FAIL corpus                     (see var/verify.log)
   -- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
   ```

   At log inspection, `var/verify.log` contained
   `DENOMINATOR checked=34 total=34 excluded=69 caught=16` and multiple named
   detector timeouts. The shared checkout had concurrent verifier processes,
   so the captured command summary is authoritative; the deliverable's
   `caught=18` is not reproduced by this run.

## Findings

1. **The recorded probe figures are stale/non-reproducible.** The default run
   did not show all three claimed lanes at `2/2`; it showed `1/2` for API
   LLM7 and BlockRun. The five-attempt run showed `3/5` for API LLM7, not
   `2/5`. The document has no timestamp or captured command output to bound
   these volatile measurements.

2. **The denominator is incomplete for reliability.** `bin/lane-probe.sh`
   defines 19 endpoints and increments `TOTAL` once per endpoint, while each
   endpoint performs `ATTEMPTS` HTTP calls. Therefore the default run made
   19 x 2 = 38 calls and the five-attempt run made 19 x 5 = 95 calls. The
   published `checked=19 total=19` is an endpoint denominator, not the call
   denominator needed to assess rate limiting. The B6 prose also reports only
   four endpoint rows and omits the 15 rejected/unusable candidates.

3. **The required ledger/status updates are absent.** `docs/DELTA.md` has no
   B6-specific probe record; its relevant historical entry is D57. In
   `handover/BACKLOG.md`, B6 is still `[~]`, although the contract says the
   no-reliable-lane outcome must be marked `[!]`.

4. **The verification detail does not match.** The deliverable records
   `caught=18`, while the verifier command exited red and the inspected log
   reported `caught=16` with timeouts. A red gate may be unrelated to B6's
   lane conclusion, but its exact result cannot be replaced by an unsupported
   count.

## Verdict

**REJECT**

## Required changes

1. Re-record the default and five-attempt probe with command, timestamp,
   complete endpoint classification, and both endpoint and HTTP-call
   denominators. Do not present a burst as 8-second evidence.
2. Add the B6 probe record to `docs/DELTA.md`, including the rejected lanes and
   the observed nondeterminism/rate limits.
3. Mark B6 `[!]` in `handover/BACKLOG.md` while leaving Requirement 5 at 45%.
4. Replace the unsupported `caught=18` claim with the exact captured verifier
   result, including exit `6` and the red corpus stage; resolve the shared-log
   attribution before publishing a corpus count.
