#!/usr/bin/env bash
# Additive keyless SQLite/tree-sitter/PyDriller entrypoint; kmap.sh is untouched.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
D="$(cd "$HERE/../../.." && pwd)"
PYTHON="${KMAP_PYTHON:-$D/var/kmap-venv/bin/python}"
[ -x "$PYTHON" ] || { printf 'error: kmap Python runtime missing: %s\n' "$PYTHON" >&2; exit 3; }
exec "$PYTHON" "$HERE/mapdb.py" "$@"

