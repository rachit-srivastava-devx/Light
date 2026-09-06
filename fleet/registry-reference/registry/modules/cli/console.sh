#!/usr/bin/env bash
# console.sh — start the fleet console (the loopback evidence plane).
#
# WHY THIS WRAPPER EXISTS: the documented command used to be `node console/server/index.mjs`, and
# on a machine whose ambient `node` is old that dies on `import { createServer }` at line 14 —
# the same ESM trap that independently broke codex, ccusage, and the terminal UI. Reached through
# `fleet`, the entrypoint has already repaired PATH, so this just works. It also refuses to start
# on an unbuilt web/dist rather than serving 404s, and prints the URL.
#
# Usage: fleet console [--port N] [--host H] [--build] [--help]
# Exit:  0 ok | 2 usage | 3 no usable node | 4 web/dist not built
set -u

D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PORT="${FLEET_CONSOLE_PORT:-7317}"
HOST="${FLEET_CONSOLE_HOST:-127.0.0.1}"
BUILD=0

usage() {
  printf '%s\n' \
    'fleet console [--port N] [--host H] [--build]' \
    '  Serves the evidence plane on 127.0.0.1 only; the server refuses any other host.' \
    '  --port N   default 7317 (or $FLEET_CONSOLE_PORT)' \
    '  --host H   default 127.0.0.1; the server itself rejects anything non-loopback' \
    '  --build    rebuild console/web/dist first'
}
while [ $# -gt 0 ]; do
  case "$1" in
    --port) [ -n "${2-}" ] || { echo "console: --port needs a value" >&2; exit 2; }; PORT="$2"; shift 2 ;;
    --host) [ -n "${2-}" ] || { echo "console: --host needs a value" >&2; exit 2; }; HOST="$2"; shift 2 ;;
    --build) BUILD=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'console: unknown argument %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

# The entrypoint normally repairs this; resolve again so a direct invocation is not left broken.
node_ok() { [ -x "$1" ] && "$1" --input-type=module -e 'import "node:http"' >/dev/null 2>&1; }
NODE=""
for c in "${FLEET_NODE:-}" "$(command -v node 2>/dev/null)" /opt/homebrew/bin/node /usr/local/bin/node; do
  [ -n "$c" ] && node_ok "$c" && { NODE="$c"; break; }
done
[ -n "$NODE" ] || { printf 'console: no ESM-capable node found — brew install node\n' >&2; exit 3; }

if [ "$BUILD" -eq 1 ]; then
  printf 'building console/web…\n'
  ( cd "$D/console/web" && PATH="$(dirname "$NODE"):$PATH" npm run build --silent ) \
    || { printf 'console: npm run build failed\n' >&2; exit 4; }
fi

# Serving an unbuilt dist means every route 404s while the process looks healthy — a running
# server is not a working console.
[ -f "$D/console/web/dist/index.html" ] || {
  printf 'console: console/web/dist is not built — every route would 404.\n  fix: fleet console --build\n' >&2
  exit 4
}

printf 'fleet console -> http://%s:%s   (ctrl+c to stop)\n' "$HOST" "$PORT"
exec env FLEET_CONSOLE_PORT="$PORT" FLEET_CONSOLE_HOST="$HOST" PATH="$(dirname "$NODE"):$PATH" \
  "$NODE" "$D/console/server/index.mjs"
