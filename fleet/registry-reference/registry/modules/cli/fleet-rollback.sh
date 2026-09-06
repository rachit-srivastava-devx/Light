#!/usr/bin/env bash
# shellcheck disable=SC2010
# fleet-rollback.sh — undo the wakeup upgrade, in whole or in part (goal #6).
# Restores the pre-change file snapshot and removes anything the installer added.
#
#   fleet/registry/modules/cli/fleet-rollback.sh              # DRY RUN — prints exactly what it would undo
#   fleet/registry/modules/cli/fleet-rollback.sh --yes        # actually roll back the latest change set
#   fleet/registry/modules/cli/fleet-rollback.sh --list       # list available snapshots
set -u
FLEET_DIR="$(cd "$(dirname "$0")/../../.." && pwd)"
BK="$FLEET_DIR/var/backups"
RC="$HOME/.zshrc"
DO=0; [ "${1:-}" = "--yes" ] && DO=1

if [ "${1:-}" = "--list" ]; then
  echo "snapshots in $BK:"; ls -1 "$BK" | grep -v '^\.' | sed 's/^/  /'; exit 0
fi

LATEST="$(cat "$BK/.latest" 2>/dev/null || true)"
SNAP="$BK/${LATEST}-pre-wakeup"
run(){ if [ "$DO" = 1 ]; then eval "$1"; else echo "  would: $1"; fi; }

echo "== fleet rollback ${DO:+}$([ "$DO" = 1 ] && echo '(APPLYING)' || echo '(dry run — pass --yes to apply)') =="

# 1) restore modified fleet files from the pre-wakeup snapshot
if [ -d "$SNAP" ]; then
  echo "-- restore files from $SNAP"
  [ -f "$SNAP/registry/modules/cli/autopilot.sh" ] && run "cp '$SNAP/registry/modules/cli/autopilot.sh' '$FLEET_DIR/registry/modules/cli/autopilot.sh'"
  [ -f "$SNAP/docs/guides/WORKFLOW.md" ]      && run "cp '$SNAP/docs/guides/WORKFLOW.md' '$FLEET_DIR/docs/guides/WORKFLOW.md'"
  for f in "$SNAP/registry/modules/agents/"*.md; do [ -e "$f" ] && run "cp '$f' '$FLEET_DIR/registry/modules/agents/$(basename "$f")'"; done
else
  echo "-- no pre-wakeup snapshot found (nothing to restore for edited files)"
fi

# 2) remove the wakeup alias block from ~/.zshrc
if grep -q '# >>> fleet wakeup' "$RC" 2>/dev/null; then
  echo "-- remove wakeup alias block from ~/.zshrc"
  run "sed -i '' '/# >>> fleet wakeup/,/# <<< fleet wakeup/d' '$RC'"
else
  echo "-- no wakeup alias block in ~/.zshrc"
fi

# 3) undo installer-recorded mutations (agents copied, brew/npm noted for manual removal)
LATEST_MAN="$(ls -1t "$BK"/*-install-manifest.txt 2>/dev/null | head -1 || true)"
if [ -n "$LATEST_MAN" ]; then
  echo "-- undo install manifest: $LATEST_MAN"
  while IFS= read -r line; do
    case "$line" in
      agent:*)  run "rm -f '${line#agent:}'";;
      brew:*)   echo "  (manual, if you want:) brew uninstall ${line#brew:}";;
      npm:*)    echo "  (manual, if you want:) npm rm -g ${line#npm:}";;
    esac
  done < "$LATEST_MAN"
fi

# 4) new files the upgrade added (kept by default; remove explicitly if you want them gone)
echo "-- upgrade-added files (kept — delete manually to fully revert):"
for f in registry/modules/cli/wakeup.sh registry/modules/cli/wakeup-install.sh registry/modules/cli/model-for.sh registry/modules/cli/px.sh registry/modules/cli/fleet-rollback.sh \
         docs/reports/AUDIT.md docs/guides/WAKEUP.md docs/design/TOKEN-POLICY.md docs/guides/ROLLBACK.md; do
  [ -e "$FLEET_DIR/$f" ] && echo "     fleet/$f"
done

[ "$DO" = 1 ] && echo "== rolled back. Run: source ~/.zshrc ==" || echo "== dry run only. Re-run with --yes to apply. =="
