#!/usr/bin/env bash
# intake.sh — service facade for the atomic intake feature.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
exec "$ROOT/registry/features/intake/intake.sh" "$@"
