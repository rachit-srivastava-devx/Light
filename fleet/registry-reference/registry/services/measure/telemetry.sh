#!/usr/bin/env bash
# Service seam for modules that need telemetry; the feature owns the implementation.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
exec "$ROOT/registry/features/telemetry/telemetry.sh" "$@"
