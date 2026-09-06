#!/usr/bin/env bash
set -u
source "$(dirname "$0")/common.sh"
require_fleet || exit 3
TMP="$(new_tmp)"
trap 'chmod -R u+rwX "$TMP" 2>/dev/null || true; rm -rf "$TMP"' EXIT
make_repo "$TMP/repo" permissions
echo '== integration: permissions =='
export FLEET_STATE="$TMP/good-state"
mkdir -p "$FLEET_STATE"
run_capture "$TMP/good.out" "$FLEET" run --task writable --repo "$TMP/repo" --agent stub
assert_rc 0 "$RUN_RC" 'writable state control completes'
export FLEET_STATE="$TMP/readonly-state"
mkdir -p "$FLEET_STATE"
chmod 0555 "$FLEET_STATE"
run_capture "$TMP/readonly.out" "$FLEET" run --task readonly --repo "$TMP/repo" --agent stub
assert_rc 3 "$RUN_RC" 'read-only FLEET_STATE exits 3'
if python3 - "$TMP/readonly.out" <<'PY'
import sys
text = open(sys.argv[1], encoding="utf-8", errors="replace").read().lower()
raise SystemExit(1 if "panic" in text or "artifact=" in text else 0)
PY
then
  ok 'read-only state is neither panic nor silent success'
else
  no 'read-only state is neither panic nor silent success'
fi
finish
