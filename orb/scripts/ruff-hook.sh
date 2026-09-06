#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PYTHON="$ROOT/backend/relay-py/.venv/bin/python"

"$PYTHON" -m ruff check --config "$ROOT/backend/relay-py/pyproject.toml" "$@"
"$PYTHON" -m ruff format --check --config "$ROOT/backend/relay-py/pyproject.toml" "$@"
