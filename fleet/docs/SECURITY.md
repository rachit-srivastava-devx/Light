# Fleet security and provenance

## Threat model

Fleet launches an untrusted worker and records the worker's only response on fd 3. The
launcher owns the ledger path, state directory, socket setup, timestamps, actor, resolved
model, exit code, artifact hash, and attestation. A worker receives none of the ledger,
state, or socket paths in its environment. Receipt bodies are recursively redacted for
secret-shaped keys and common token prefixes before hashing and append.

The ledger is an append-only, hash-chained JSONL record. Verification checks every row,
the sequence, the predecessor hash, and the published `{checked,total}` denominator.
This is tamper-evident, not tamper-proof: a process with operator access can delete the
ledger, replace the binary, or change the machine.

## Provenance boundary

Against the worker, fleet achieves the SLSA L3 non-forgeability property: a build cannot
forge its own provenance because the fd-3 launcher-stamping design keeps provenance fields
and the ledger outside the worker's authority. The worker can still produce a bad result;
it cannot author a valid launcher receipt or rewrite the frozen artifact through that
channel.

Against the operator, fleet is only L1. The operator owns the machine and can delete the
ledger or state directory, so the evidence can disappear. Fleet's guarantee is therefore
tamper-evident, not tamper-proof.

**NOT IMPLEMENTED — corrected 2026-08-24.** This paragraph previously described "fleet attest
self" (an in-toto Statement over the running binary, verified with `--verify`, exit 8 on mismatch)
and "fleet sbom" (a CycloneDX inventory from Cargo's locked graph). **Neither command exists.**
A security document that describes capabilities the tool does not have is worse than one that
claims less: it is exactly the surface a reviewer trusts without re-checking, and it would have
survived indefinitely because no test read the docs.

Found by sweeping every backticked `fleet …` command in every document against the real binary —
20 of 23 existed. Detector `M6` now fails when a documented command does not, so this class cannot
recur silently.

What IS true today: `fleet gate` runs the required gate set (`unit tests`, `mutants`, `semgrep`,
`trivy`, `recur`, `detectors`, `policy`, `corpus` — see `crates/fleet-verify/src/registry.rs` for
the current list). Supply-chain attestation of the fleet binary itself remains unbuilt.

## Stronger property

A separate merge authority that owns the release decision, or a witnessed transparency
log that retains append evidence outside the operator's machine, would buy the stronger
operator-side property. That introduces external authority, availability and key
management costs, and breaks the current keyless operating model.

## Residual risks

Redaction is a defense-in-depth boundary, not a guarantee that arbitrary binary secrets
are recognizable. Secrets should not be placed in tasks, repository paths, or worker
output. The operator must preserve state independently when audit durability matters.
