# ADR 0014 — `hdrhistogram` is the single percentile authority

**Status:** accepted. **Date:** 2026-08-28. **Registry decision:** install.

## Context

`HopHistogram.percentile()` used Python `round()` over `N-1`; the eval gate used
`ceil(p*N)-1`. For `[10, 20, 30, 40]`, p25 was respectively 20 and 10. The first implementation
also retained every sample forever. These values gate voice p50 <=1100ms and p99 <=2000ms, so an
ambiguous ruler is a release defect.

The acceptance contract explicitly names HDR Histogram instead of hand-rolled list percentiles.
The PyPI distribution is named `hdrhistogram`; its Python import package is `hdrh`.

## Decision

Pin `hdrhistogram==0.10.7` and route both call sites through `BoundedHdrHistogram`.

- Convention: nearest-rank cumulative count, no linear interpolation. The result is the smallest
  HDR bucket reaching `ceil(p/100*N)`; p0 returns the minimum. HDR reports the bucket's highest
  equivalent value, making quantisation conservative for upper-bound SLOs.
- Resolution/range: inputs round upward to one whole unit; latency callers use milliseconds and
  cost callers use paise. The histogram tracks 1 through 86,400,000 with three significant figures.
  Values near the <=2000ms voice threshold retain 1ms resolution.
- Memory: the counts array has 18,432 fixed buckets (147,456 bytes with 64-bit counters) instead of
  an unbounded `samples_ms` list.
- Coordinated omission: every supplied turn is recorded and no fast-path sampling exists. This
  primitive does not invent missing turns. Full correction requires an open-loop load harness with
  a real expected-arrival interval, then HDR's `record_corrected_value`; the local deterministic
  corpus has no defensible interval to supply.
- Thresholds remain unchanged.

## Package vetting

- License: Apache-2.0 (upstream LICENSE and PyPI classifier).
- Freshness: 0.10.7 released 2026-06-09, within the 18-month gate, with signed PyPI provenance and
  CPython 3.10-3.14 wheels including macOS ARM64.
- Adoption/ownership: maintained by the HdrHistogram organisation; the Python repository has
  non-trivial public adoption and interoperates with the Java/C HDR format.
- Security: no unresolved critical advisory was found for the Python package during adoption. A
  2026 low-severity, unreviewed Java decoder advisory does not name the Python distribution; this
  product neither decodes untrusted histogram blobs nor uses the Java package.

## Verification

The regression matrix covers N={1,2,3,4} and p={0,1,25,50,75,95,99,100} across both call sites,
plus fixed-allocation behavior after 10,004 observations. SLO before/after values are recorded in
the Track L handoff; any red gate remains red rather than moving its threshold.
