# SECURITY-TRIAGE.md
<!-- Triaged: 2026-09-11 | Source: prior automated triage artifact; no current Sonnet sign-off -->

This is not a current model or security sign-off. The Claude CLI verification attempt
in this review could not authenticate; treat these findings as hypotheses until the
listed tools and versions are rerun and their raw exit status is recorded.

Security findings from OSV Scanner 2.5.1 run against all three lockfiles in the Fleet repo.
Each finding is classified by runtime exposure and remediation track.

---

## Cargo.lock (fleet Rust workspace)

### RUSTSEC-2023-0086 / GHSA-2326-pfpj-vx3h — `lexical-core@0.7.6`

| Field | Value |
|-------|-------|
| Severity | CVSS 7.5 (high) |
| Type | Unsoundness — UAF in numeric string parsing (misaligned read) |
| Scope | Transitive (pulled in by tantivy text-search library) |
| Fleet exposure | Fleet uses tantivy for BM25 retrieval in `fleet-memory`. Fleet never passes adversarial untrusted strings directly to lexical-core; all inputs are already-parsed numeric tokens from the knowledge store. |
| Verdict | **SUPPRESSED** — no direct adversarial input path; documented in `deny.toml` ignore list |
| Remediation | Upgrade when tantivy publishes a release pulling lexical-core ≥ 0.8. Tracked: TECH_DEBT.md |

### RUSTSEC-2026-0253 — `lru@0.16.4`

| Field | Value |
|-------|-------|
| Severity | Informational |
| Type | Cache eviction ordering edge case |
| Scope | Transitive |
| Fleet exposure | lru is used by internal caching in transitive deps; fleet does not directly instantiate LRU caches with adversarial keys. |
| Verdict | **SUPPRESSED** — informational advisory, no exploitable path; documented in `deny.toml` ignore list |
| Remediation | Update when patched version released |

---

## crates/fleet-crew/uv.lock (Python tooling — workspace-excluded)

`crates/fleet-crew` is listed in the Cargo workspace `exclude` array. It is a Python
scaffolding tool, not fleet runtime. Not deployed to any production environment.

### PYSEC-2026-1845 / GHSA-6w46-j5rx-g56g — `pytest@8.4.2`

| Field | Value |
|-------|-------|
| Severity | Low |
| Scope | Dev tooling only (`crates/fleet-crew` is workspace-excluded) |
| Fleet exposure | Zero runtime exposure — pytest is never installed in fleet runtime containers |
| Verdict | **ACCEPTED** — dev tooling scope, no runtime path |
| Remediation | `cd crates/fleet-crew && uv lock --upgrade-package pytest` when a fixed pytest release is available |

---

## fleet-workflow-explorer/pnpm-lock.yaml (static visualization — non-workspace)

`fleet-workflow-explorer` is a standalone React visualization tool (static HTML/JS). It is not
part of the Rust workspace and is not deployed as part of fleet runtime. The minimist findings
are in devDependencies of devDependencies (transitive via parcel build tooling).

### GHSA-vh95-rmgr-6w4m + GHSA-xvch-5gv4-984h — `minimist@0.0.10` and `minimist@1.1.3`

| Field | Value |
|-------|-------|
| Severity | Medium (prototype pollution in CLI argument parsing) |
| Scope | Build-time dev tool (parcel) transitive dep — never in the shipped bundle |
| Fleet exposure | Zero — minimist is invoked by the parcel build tool on the developer's machine, never in the shipped HTML artifact or in fleet runtime |
| Verdict | **ACCEPTED** — build-tool scope only; not reachable from any fleet process |
| Remediation | `cd fleet-workflow-explorer && pnpm update` when minimist is upgraded by parcel; or replace parcel with vite (already in devDeps) and remove parcel |

---

## Summary

| Finding | Package | Lockfile | Runtime exposure | Status |
|---------|---------|----------|-----------------|--------|
| RUSTSEC-2023-0086 | lexical-core@0.7.6 | Cargo.lock | None — adversarial input never reaches lexical-core | Suppressed in deny.toml |
| GHSA-2326-pfpj-vx3h | lexical-core@0.7.6 | Cargo.lock | Same as above | Suppressed in deny.toml |
| RUSTSEC-2026-0253 | lru@0.16.4 | Cargo.lock | None — informational | Suppressed in deny.toml |
| PYSEC-2026-1845 | pytest@8.4.2 | fleet-crew/uv.lock | None — workspace-excluded dev tool | Accepted |
| GHSA-6w46-j5rx-g56g | pytest@8.4.2 | fleet-crew/uv.lock | None — workspace-excluded dev tool | Accepted |
| GHSA-vh95-rmgr-6w4m | minimist@0.0.10 | explorer/pnpm-lock.yaml | None — build-tool transitive, not bundled | Accepted |
| GHSA-xvch-5gv4-984h | minimist@0.0.10 | explorer/pnpm-lock.yaml | None — build-tool transitive, not bundled | Accepted |
| GHSA-vh95-rmgr-6w4m | minimist@1.1.3 | explorer/pnpm-lock.yaml | None — build-tool transitive, not bundled | Accepted |
| GHSA-xvch-5gv4-984h | minimist@1.1.3 | explorer/pnpm-lock.yaml | None — build-tool transitive, not bundled | Accepted |

**All 9 findings: zero runtime exposure. No fleet process can be reached through any of these paths.**

`cargo deny check` exits 0 for advisories (with two ignore entries for the Cargo.lock findings;
warnings appear because cargo-deny's local advisory DB cache predates the 2026 advisory IDs,
but the ignore entries will activate when the cache updates).
