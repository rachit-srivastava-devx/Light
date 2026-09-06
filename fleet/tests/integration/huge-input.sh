#!/usr/bin/env bash
set -u
source "$(dirname "$0")/common.sh"
require_fleet || exit 3
TMP="$(new_tmp)"
trap 'rm -rf "$TMP"' EXIT
export FLEET_STATE="$TMP/state"
mkdir -p "$FLEET_STATE"
make_repo "$TMP/repo" huge-input
echo '== integration: huge input =='
for i in $(seq 1 5000); do
  printf 'file-%s\n' "$i" > "$TMP/repo/file-$i.txt"
done
( cd "$TMP/repo" && git add -A && git -c user.email=fleet@test -c user.name=fleet commit -qm huge )
START_NS="$(python3 -c 'import time; print(time.monotonic_ns())')"
run_capture "$TMP/good.out" "$FLEET" run --task small-control --repo "$TMP/repo" --agent stub
END_NS="$(python3 -c 'import time; print(time.monotonic_ns())')"
assert_rc 0 "$RUN_RC" '5k-file repository completes'
ELAPSED_MS=$(( (END_NS - START_NS) / 1000000 ))
if [ "$ELAPSED_MS" -le 30000 ]; then
  ok "5k-file run is bounded (${ELAPSED_MS}ms)"
else
  no '5k-file run is bounded' "${ELAPSED_MS}ms"
fi
# BAD fixture: E2BIG means no fleet process ran and is therefore a failure.
python3 - "$FLEET" "$TMP/repo" "$TMP/huge.out" <<'PY'
import errno, subprocess, sys
fleet, repo, output = sys.argv[1:]
task = "x" * (10 * 1024 * 1024)
try:
    result = subprocess.run([fleet, "run", "--task", task, "--repo", repo, "--agent", "stub"],
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=10)
except OSError as error:
    open(output, "w").write(f"exec-error={error.errno}:{error.strerror}\n")
    raise SystemExit(3 if error.errno == errno.E2BIG else 8)
except subprocess.TimeoutExpired:
    open(output, "w").write("timeout\n")
    raise SystemExit(8)
open(output, "wb").write(result.stdout)
print(f"fleet-exit={result.returncode}")
raise SystemExit(0 if result.returncode == 7 else 8)
PY
HUGE_RC=$?
assert_rc 7 "$HUGE_RC" '10MB task has typed refusal exit 7'
assert_output_nonempty "$TMP/huge.out" '10MB task produced a definitive outcome'
assert_no_agent_processes "$TMP/ps.out"
finish
