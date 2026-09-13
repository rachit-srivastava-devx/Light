# ADR-0002: Relocate the receipt and attestation wire contracts

## Status

Accepted for the canonical crate migration.

## Decision

Restore the byte-preserved `receipt.v1.json` and `attestation.v1.json` schemas under
`crates/types/contracts/`, the only path used by the canonical `types` crate's compile-time
schema-parity tests. The source files are copied from the last pre-migration
`crates/fleet-types/contracts/` revision; this decision changes location, not wire vocabulary.

The remaining historical contracts must be relocated before any consumer claims full contract
coverage. No schema validator is added here; the existing compile-time fixtures and parity tests
remain the acceptance authority.

## Consequences

- Missing fixtures become a build failure instead of silently dropping wire-contract coverage.
- Rust source and tests use one canonical contract directory after the `fleet-types` retirement.
- Any future schema content change still requires an ADR and updated parity/migration evidence.
