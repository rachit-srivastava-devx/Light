#!/usr/bin/env bash
# Install the fleet harness (hooks + gate + ledger) into an EXISTING repo without
# scaffolding a blueprint or touching source. Idempotent. Use on company/ monorepo repos
# (products/<p> or platform/registry/modules/<m>) so the autopilot's gates actually bind there.
# Usage: harden-repo.sh <repo-dir>
set -euo pipefail
REPO="${1:?usage: harden-repo.sh <repo-dir>}"
FLEET_DIR="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$REPO"; REPO="$(pwd)"

mkdir -p .fleet .claude tests/contract tests/large/acceptance var/evidence current_tasks tasks/inbox tasks/active tasks/done tasks/failed

cp "$FLEET_DIR/hooks/protect-files.sh" .fleet/protect-files.sh
cp "$FLEET_DIR/hooks/stop-gate.sh"     .fleet/stop-gate.sh
cp "$FLEET_DIR/hooks/quiet-test.sh"    .fleet/quiet.sh
chmod +x .fleet/*.sh
[ -f feature_list.schema.json ] || cp "$FLEET_DIR/registry/modules/templates/feature_list.schema.json" feature_list.schema.json
[ -f feature_list.json ] || printf '{\n  "project": "%s",\n  "features": []\n}\n' "$(basename "$REPO")" > feature_list.json

# settings.json — only write if absent (never clobber an existing hook config; warn instead).
if [ ! -f .claude/settings.json ]; then
  cp "$FLEET_DIR/hooks/settings-hooks.json" .claude/settings.json
else
  grep -q 'protect-files.sh' .claude/settings.json || \
    echo "WARN: .claude/settings.json exists but has no fleet hooks — merge $FLEET_DIR/hooks/settings-hooks.json by hand." >&2
fi

# Detect the real verify gate (C17). Prefer an explicit 'verify' script; else vitest; else red.
if [ -f package.json ] && grep -q '"verify"' package.json; then
  GATE='npm run -s verify'
elif [ -f vitest.config.ts ] || [ -f vitest.config.js ]; then
  GATE='npx vitest run'
elif [ -f Makefile ] && grep -q '^verify:' Makefile; then
  GATE='make verify'
elif [ -f pyproject.toml ]; then
  GATE='ruff check . && pytest -q'
else
  GATE='echo "GATE NOT CONFIGURED — set .fleet/gate.cmd to this repo'\''s real verify command (C17)"; exit 1'
fi
[ -f .fleet/gate.cmd ] || printf '%s\n' "$GATE" > .fleet/gate.cmd

# keep .fleet logs & runtime out of the repo history
if [ -f .gitignore ]; then grep -q '^\.fleet/logs' .gitignore || printf '\n.fleet/logs/\n.fleet/.stop-gate-count\n' >> .gitignore; fi

echo "hardened: $REPO"
echo "  gate.cmd -> $(cat .fleet/gate.cmd)"
