#!/usr/bin/env bash
# Cleanly remove token-optimizer. Primary path: restore the pre-install settings.json snapshot
# (drops all 25 hooks + the cache hook + the ContextQ status line in one shot), then delete the
# install dir + skill symlink. Fully reverses `fleet/bin/wakeup-install`-era token-optimizer setup.
#
#   fleet/registry/modules/cli/token-optimizer-uninstall.sh          # DRY RUN
#   fleet/registry/modules/cli/token-optimizer-uninstall.sh --yes     # apply
set -u
FLEET_DIR="$(cd "$(dirname "$0")/../../.." && pwd)"
SNAP="$FLEET_DIR/backups/$(cat "$FLEET_DIR/backups/.latest-token-optimizer" 2>/dev/null || true)"
DO=0; [ "${1:-}" = "--yes" ] && DO=1
run(){ if [ "$DO" = 1 ]; then eval "$1"; else echo "  would: $1"; fi; }

echo "== token-optimizer uninstall $([ "$DO" = 1 ] && echo '(APPLYING)' || echo '(dry run — pass --yes)') =="
if [ -f "$SNAP/settings.json" ]; then
  echo "-- restore settings.json from $SNAP (removes hooks + cache hook + status line)"
  run "cp '$SNAP/settings.json' \"\$HOME/.claude/settings.json\""
else
  echo "-- no snapshot found; using the tool's own uninstall (best-effort)"
  run "python3 \"\$HOME/.claude/token-optimizer/skills/token-optimizer/scripts/measure.py\" setup-quality-bar --uninstall"
fi
echo "-- remove the install + skill symlink"
run "rm -f \"\$HOME/.claude/skills/token-optimizer\""
# rm-rf-ok: uninstaller removing exactly the directory it installed
run "rm -rf \"\$HOME/.claude/token-optimizer\""
[ "$DO" = 1 ] && echo "== removed. Restart Claude Code to reload settings. ==" || echo "== dry run only. Re-run with --yes. =="
