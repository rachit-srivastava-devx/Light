#!/usr/bin/env bash
set -u
source "$(dirname "$0")/common.sh"
require_fleet || exit 3
if ! command -v hdiutil >/dev/null 2>&1 || ! command -v diskutil >/dev/null 2>&1; then
  printf 'FATAL: hdiutil and diskutil are required for the macOS quota fixture; exit 3\n'
  exit 3
fi
TMP="$(new_tmp)"
trap 'hdiutil detach "$TMP/mount" >/dev/null 2>&1 || true; rm -rf "$TMP"' EXIT
export FLEET_STATE="$TMP/good-state"
mkdir -p "$FLEET_STATE"
make_repo "$TMP/good-repo" disk-good
echo '== integration: disk full =='
run_capture "$TMP/good.out" "$FLEET" run --task disk-control --repo "$TMP/good-repo" --agent stub
assert_rc 0 "$RUN_RC" 'good normal-disk control completes'
hdiutil create -quiet -size 2m -fs APFS -volname FleetInteg "$TMP/fleet.dmg"
mkdir -p "$TMP/mount"
hdiutil attach "$TMP/fleet.dmg" -nobrowse -mountpoint "$TMP/mount" >/dev/null
FLEET_STATE="$TMP/mount/state"
export FLEET_STATE
mkdir -p "$FLEET_STATE"
run_capture "$TMP/seed.out" "$FLEET" ledger append --event note --body '{"fixture":"seed"}'
assert_rc 0 "$RUN_RC" 'quota fixture has a writable good control'
fill_index=0
while [ "$fill_index" -lt 100 ]; do
  dd if=/dev/urandom of="$TMP/mount/fill-$fill_index" bs=1024 count=64 >/dev/null 2>&1
  fill_rc=$?
  fill_index=$((fill_index + 1))
  if [ "$fill_rc" -ne 0 ]; then
    break
  fi
done
run_capture "$TMP/full.out" "$FLEET" run --task disk-full --repo "$TMP/good-repo" --agent stub
assert_rc 3 "$RUN_RC" 'full quota is typed as environment fault'
assert_output_captured "$TMP/full.out" 'disk-full command produced a definitive outcome'
ledger_verify "$TMP/full-verify.out"
assert_rc 0 "$RUN_RC" 'disk-full refusal leaves the ledger valid'
assert_no_agent_processes "$TMP/ps.out"
finish
