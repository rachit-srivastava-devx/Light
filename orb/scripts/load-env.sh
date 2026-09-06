# Dot-source this ("`. scripts/load-env.sh`"), never execute it — sourcing is what lets the
# exported vars reach the calling shell (npm runs each script's whole string via `sh -c "..."`,
# so a leading `. scripts/load-env.sh;` loads .env into that same sh -c process before the rest
# of the script line runs). Silently does nothing if .env doesn't exist yet (e.g. before
# scripts/setup.sh has been run).
if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi
