#!/usr/bin/env bash
# Scaffold an implementation repo from a blueprint, wired with the full harness:
# CLAUDE.md ritual, feature-ledger contract, hooks (protect/stop-gate/quiet), loop prompts.
# Usage: init-project.sh "<blueprint-dir>" "<target-dir>"
set -euo pipefail
BLUEPRINT="${1:?usage: init-project.sh <blueprint-dir> <target-dir>}"
TARGET="${2:?usage: init-project.sh <blueprint-dir> <target-dir>}"
FLEET_DIR="$(cd "$(dirname "$0")/../../.." && pwd)"
NAME="$(basename "$TARGET")"

mkdir -p "$TARGET"/{.fleet,.claude,tests/large/acceptance,tests/contract,var/evidence,current_tasks}
BP_ABS="$(cd "$BLUEPRINT" && pwd)"

subst() { sed -e "s|<PROJECT>|$NAME|g" -e "s|<BLUEPRINT_PATH>|$BP_ABS|g" "$1" > "$2"; }
subst "$FLEET_DIR/registry/modules/templates/PROJECT-CLAUDE.md"    "$TARGET/CLAUDE.md"
subst "$FLEET_DIR/registry/modules/templates/INITIALIZER-BRIEF.md" "$TARGET/INITIALIZER-BRIEF.md"
subst "$FLEET_DIR/registry/modules/templates/LOOP-PROMPT.md"       "$TARGET/LOOP-PROMPT.md"
ln -sf CLAUDE.md "$TARGET/AGENTS.md"

cp "$FLEET_DIR/hooks/protect-files.sh" "$FLEET_DIR/hooks/stop-gate.sh" "$FLEET_DIR/hooks/injection-guard.sh" "$TARGET/.fleet/"
cp "$FLEET_DIR/hooks/quiet-test.sh" "$TARGET/.fleet/quiet.sh"
chmod +x "$TARGET/.fleet/"*.sh
cp "$FLEET_DIR/hooks/settings-hooks.json" "$TARGET/.claude/settings.json"
cp "$FLEET_DIR/registry/modules/templates/feature_list.schema.json" "$TARGET/feature_list.schema.json"

# Deterministic spine, seeded RED so the Stop-hook is never a silent no-op before the
# initializer runs. The initializer replaces gate.cmd with the real verify command (C17)
# and writes the real feature_list.json. Until then: gate is red, ledger is empty-but-valid.
[ -f "$TARGET/.fleet/gate.cmd" ] || \
  printf 'echo "GATE NOT CONFIGURED — initializer must set .fleet/gate.cmd to the real verify command (C17)"; exit 1\n' \
  > "$TARGET/.fleet/gate.cmd"
[ -f "$TARGET/feature_list.json" ] || \
  printf '{\n  "project": "%s",\n  "features": []\n}\n' "$NAME" > "$TARGET/feature_list.json"

cd "$TARGET"
TARGET="$(pwd)"
git init -q 2>/dev/null || true
git add -A && git commit -qm "scaffold $NAME harness from fleet templates" 2>/dev/null || true

cat <<EOF
Scaffolded $TARGET  (blueprint: $BP_ABS)
Next:
  cd "$TARGET" && claude
  > Read INITIALIZER-BRIEF.md and execute it.        # once
Then either hand it to firstmate as a project, or run the loop:
  tmux new -d -s loop-$NAME "$FLEET_DIR/registry/modules/cli/overnight.sh '$TARGET' 30"
EOF
