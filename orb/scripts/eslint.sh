#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
NPM_CACHE_DIR=${ORB_GATE_NPM_CACHE_DIR:-"$ROOT/.gate-tools/npm-cache"}
mkdir -p "$NPM_CACHE_DIR"

export npm_config_cache="$NPM_CACHE_DIR"
npx --yes \
  --package=eslint@10.9.1 \
  --package=typescript-eslint@8.68.0 \
  sh -c '
    eslint_bin=$(command -v eslint)
    eslint_modules=$(cd "$(dirname "$eslint_bin")/.." && pwd)
    ORB_ESLINT_NODE_MODULES="$eslint_modules" eslint --config eslint.config.mjs "$@"
  ' sh "$@"
