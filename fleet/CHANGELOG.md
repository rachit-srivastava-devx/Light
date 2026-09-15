# Changelog

Release convention: every push to `main` requires a matching versioned note under
`docs/releases/` and an entry in this file. See [CONTRIBUTING.md](CONTRIBUTING.md).

## [0.1.1] - 2026-09-15

### Added

- Human-readable LLD execution tracing for `fleet run`.
- Numbered LLD path logs for every stage and node, including crash-resumed stages.
- Interactive-answer tracing that identifies the direct adapter path and states when the full
  pipeline is not traversed.
- Native Apple Silicon development artifacts under `target/aarch64-apple-darwin`.

### Fixed

- `cargo-watch` now invokes `scripts/fleet-build.sh` through shell mode instead of incorrectly
  dispatching `cargo bash`.
- Adopted legacy watchers now use their existing `.dev-watch/watch.log` for diagnostics.
- `./dev.sh watch` stays in watcher mode instead of forwarding `watch` to Fleet.
- ARM64 development no longer selects the x86_64 Rust toolchain when the native toolchain is
  installed.
- The optional `sccache` wrapper is opt-in via `FLEET_USE_SCCACHE=1`; the default avoids a
  stalled ARM64 Cargo/x86_64 toolchain combination.

### Verification

- Native binary confirmed as `Mach-O 64-bit executable arm64`.
- `cargo test -p print`: 19 passed.
- Repeat native build: 0.436 seconds after the initial 63-second build.
- Real `fleet run` printed the LLD path trace through Verify.

### Known limitation

- The real pipeline run reached Verify but the repository secret scanner timed out, so that run
  is not recorded as a full pipeline pass.
