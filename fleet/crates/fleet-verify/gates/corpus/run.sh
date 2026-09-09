#!/usr/bin/env bash
set -u
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
# S1 fix: each detector used to derive its scan root from ITS OWN script path (dirname/../..),
# which only ever pointed at a real checkout when the gates-root itself was overridden to be one.
# Capture the cwd this script was invoked with -- fleet's runner sets it to the `--repo` target
# (see verify_runner_bounded.rs) -- BEFORE anything below can change it, and hand it to every
# detector explicitly so "scan root" never again depends on where this script happens to live.
export FLEET_TARGET_REPO="$(pwd)"
checked=0
total=0
excluded=0
caught=0
timeout_contention=0
timeout_confirmed=0
timeout_persistent=0
timed_out_files=()

# run_one <file> <timeout-seconds> -- sets globals `out` and `drc`.
run_one() {
  out=$(timeout "$2" bash "$1" 2>&1); drc=$?
}

for f in "$DIR"/*.sh; do
  [ "$f" = "$DIR/run.sh" ] && continue
  [ "$f" = "$DIR/_selftest.sh" ] && continue
  [ -f "$f" ] || continue
  total=$((total + 1))
  # D30: a per-detector timeout. A detector that HANGS is worse than one that fails -- the whole
  # gate stalls and reads as "still running" rather than "broken". This happened when a 4.9GB
  # CARGO_TARGET_DIR was created outside the pruned paths and 24 detectors began walking it.
  # A timeout is a FAILURE, named, never a skip.
  #
  # B13: under this project's normal operating mode (multiple agents/verify.sh runs concurrently),
  # a 30s wall-clock timeout fires from CPU starvation as often as from an actually-broken
  # detector, and the two used to be indistinguishable in the output. A first-pass timeout is now
  # PROVISIONAL, not final: it is queued and retried once, serially, after every other detector has
  # finished this same run (so it is no longer competing with the rest of this run's own detectors
  # for CPU -- it does nothing about *other processes* on the machine, but it is a real, free signal
  # about *this run*). See the retry loop below for how the retry's outcome is classified. This is a
  # heuristic, not proof of contention -- a genuinely flaky-but-real bug that happens to also be slow
  # could in principle show the same "fails once, passes once" pattern. That is why a
  # timeout-cleared-on-retry still fails the gate (never a silent pass) rather than being waved
  # through as "probably fine".
  run_one "$f" "${FLEET_DETECTOR_TIMEOUT:-30}"
  if [ "$drc" -eq 124 ]; then
    echo "  TIMEOUT $(basename "$f") exceeded ${FLEET_DETECTOR_TIMEOUT:-30}s on the main pass -- provisional, queued for one serial retry"
    timed_out_files+=("$f")
    continue
  fi
  # NOT `rc=$?` -- that reads the `if` block's status (always 0), so exit 77 (not-mechanisable)
  # would never fire and every detector would count as checked. The denominator went 27 -> 97 and
  # excluded 69 -> 0 the moment the wrapper landed. Same $?-after-a-compound-statement bug this
  # build has now hit four times. Carry the detector's own code forward explicitly.
  rc=$drc
  printf "%s\n" "$out"
  case "$rc" in
    0|1) checked=$((checked + 1)); [ "$rc" -eq 1 ] && caught=$((caught + 1));;
    77) excluded=$((excluded + 1));;
    *) printf "ENVIRONMENT FAULT: %s exit %s\n" "$f" "$rc"; exit 3;;
  esac
done

if [ "${#timed_out_files[@]}" -gt 0 ]; then
  echo "-- retrying ${#timed_out_files[@]} timed-out detector(s) serially, one at a time, once each --"
  for f in "${timed_out_files[@]}"; do
    run_one "$f" "${FLEET_DETECTOR_TIMEOUT:-30}"
    rc=$drc
    printf "%s\n" "$out"
    case "$rc" in
      0)
        # Passed clean once no longer racing the rest of this run for CPU: consistent with
        # contention, not breakage. Still counted as checked, still FAILS the gate (see exit logic
        # below) -- distinguishable from a caught regression in both the label and the exit code,
        # never silently converted to a pass.
        echo "  TIMEOUT-CONTENTION $(basename "$f") -- timed out on the main pass, PASSED CLEAN on serial retry (consistent with CPU contention, not a broken detector). Still fails the gate; rerun uncontended to confirm before trusting this label."
        checked=$((checked + 1)); timeout_contention=$((timeout_contention + 1));;
      1)
        # Failed for real on retry too, with no contention to blame this time: a genuine caught
        # regression, not an environment artifact.
        echo "  TIMEOUT-CONFIRMED-CAUGHT $(basename "$f") -- timed out on the main pass, and FAILED (not merely timed out) on serial retry: counted as a caught regression."
        checked=$((checked + 1)); caught=$((caught + 1)); timeout_confirmed=$((timeout_confirmed + 1));;
      124)
        # Timed out again with nothing else running concurrently in this script: this is a
        # genuinely hanging detector, not contention. Counted as a caught regression, same as any
        # other broken detector -- never let a persistent hang hide behind "the machine was busy".
        echo "  TIMEOUT-PERSISTENT $(basename "$f") -- timed out again on an uncontended serial retry: a genuinely hanging detector. Counted as a caught regression."
        checked=$((checked + 1)); caught=$((caught + 1)); timeout_persistent=$((timeout_persistent + 1));;
      77) excluded=$((excluded + 1));;
      *) printf "ENVIRONMENT FAULT: %s exit %s (on serial retry)\n" "$f" "$rc"; exit 3;;
    esac
  done
fi

printf "DENOMINATOR checked=%d total=%d excluded=%d caught=%d timeout_contention=%d timeout_confirmed=%d timeout_persistent=%d\n" \
  "$checked" "$((total-excluded))" "$excluded" "$caught" "$timeout_contention" "$timeout_confirmed" "$timeout_persistent"
[ "$checked" -gt 0 ] || exit 6
if [ "$caught" -gt 0 ]; then
  exit 1
elif [ "$timeout_contention" -gt 0 ]; then
  # Distinct exit code from the plain "caught a regression" path (1): the gate still fails -- a
  # timeout is never silently converted to a pass -- but a human/agent reading `$?` alone, not just
  # the log, can tell "every retry cleared, likely just busy" from "something is actually broken".
  exit 5
else
  exit 0
fi
