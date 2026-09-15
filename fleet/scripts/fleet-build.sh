#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"
CARGO_BIN="${FLEET_CARGO_BIN:-$(command -v cargo)}"
if [[ -z "${CARGO_TARGET_DIR:-}" && "$(uname -m)" == "arm64" ]]; then
    export CARGO_TARGET_DIR="$ROOT_DIR/target/aarch64-apple-darwin"
fi
if [[ "$(uname -m)" == "arm64" ]] && command -v rustup >/dev/null 2>&1 \
    && rustup toolchain list 2>/dev/null | grep -q '^stable-aarch64-apple-darwin'; then
    export RUSTUP_TOOLCHAIN="${FLEET_RUST_TOOLCHAIN:-stable-aarch64-apple-darwin}"
fi
if [[ "${FLEET_USE_SCCACHE:-0}" == "1" ]] && command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER="$(command -v sccache)"
    export SCCACHE_IGNORE_SERVER_IO_ERROR=1
fi
exec "$CARGO_BIN" build --bin fleet
